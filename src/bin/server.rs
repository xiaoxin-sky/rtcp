use std::{
    collections::HashMap, io::Read, marker::PhantomPinned, sync::Arc, time::Duration
};

use bytes::{Buf, BufMut, BytesMut};
use deadpool::unmanaged::{self, Object};
use rtcp::{
    parser::{parser_request_head_all, RequestLine},
    protocol::{RTCPMessage, RTCPType},
    tcp_pool::TcpStreamData,
};
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, error::TryRecvError},
    time::timeout,
};

pub struct RTcpServer {
    pub tcp_pool: Arc<unmanaged::Pool<TcpStreamData>>,
}

impl RTcpServer {
    pub async fn new() -> Self {
        let tcp_pool = unmanaged::Pool::new(1000);
        Self {
            tcp_pool: Arc::new(tcp_pool),
        }
    }

    /// 创建通道服务器
    pub async fn create_connect_channel(self) -> io::Result<()> {
        let tcp_listener = TcpListener::bind("0.0.0.0:5541").await?;
        let this = Arc::new(self);

        loop {
            let this = this.clone();
            
            match tcp_listener.accept().await {
                Ok(stream) => {
                    tokio::spawn(async move {
                        this.client_handle(stream.0).await;
                    });
                }
                Err(e) => {
                    println!("❌通道接收失败{:?}", e);
                    continue;
                }
            };
        }
    }

    async fn client_handle(self: Arc<Self>, mut tcp: TcpStream) {
        tokio::spawn(async move {
            loop {
                let msg = self.read_msg(&mut tcp).await;

                if msg.is_err() {
                    println!("❌读取消息失败,关闭当前client 连接{:?}", msg);
                    return;
                }

                let msg = msg.unwrap();

                println!("读取消息: {}", msg.message_type);
                match msg.message_type {
                    RTCPType::Initialize(port) => {
                       let proxy_server_handle =  self.create_proxy_server().await;
                        self.create_user_server(port, &mut tcp).await;
                        proxy_server_handle.abort();
                        println!("监测到client 断开，销毁全部✅");
                    }
                    RTCPType::NewConnection => {
                        println!("🔥不需要实现")
                    }
                    RTCPType::CloseConnection => println!("🔥不需要实现"),
                }
            }
        });
    }

    async fn read_msg<T>(&self, tcp: &mut T) -> io::Result<RTCPMessage>
    where
        T: AsyncRead + Unpin,
    {
        let mut buf = BytesMut::with_capacity(4 * 1024);
        loop {
            tcp.read_buf(&mut buf).await?;
            if buf.is_empty() {
                return Err(io::Error::new(io::ErrorKind::Other, "tcp连接已关闭"));
            }

            let res = RTCPMessage::deserialize(&buf);

            if res.is_err() {
                println!("序列化失败,继续读取 {:?}", res);
                continue;
            }

            let (rtcp_message, _size) = res.unwrap();

            return Ok(rtcp_message);
        }
    }

    /// 创建用户服务器
    /// 用于接收用户请求，并把请求转发给代理服务器
    async fn create_user_server(&self, port: usize, tcp: &mut TcpStream) {
        let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await;
        if listener.is_err() {
            return;
        }
        let listener = listener.unwrap();
        println!("✅[{port}]用户服务器端口启动成功");

        let (tx, mut rx) = mpsc::channel::<()>(100);
        loop {

            let tx = tx.clone();
            const TIMEOUT: Duration = Duration::from_millis(500);
            if let Ok(Ok(res)) = timeout(TIMEOUT, listener.accept()).await {
                let (mut user_tcp, _user_addr) = res;
                let tcp_pool = self.tcp_pool.clone();

                let pool_status = tcp_pool.status();
                println!("🚀收到请求:{_user_addr}  {pool_status:?}");

                if pool_status.available == 0 {
                    let msg = RTCPMessage::new(RTCPType::NewConnection);
                    let res = tcp.write_all(&msg.serialize()).await;
                    println!("🚀写入创建新消息结果{:?}", res);
                    if res.is_err() {
                        break;
                    }
                    let res = tcp.flush().await;
                    println!("🚀发送创建新链接消息成功,{:?}", res);
                }
                tokio::spawn(async move {
                    let mut client_tcp = tcp_pool.get().await.unwrap();
                    let mut is_client_disconnect = false;
                    loop {
                        let (mut r, mut w) = client_tcp.stream.split();
                        let (mut r1, mut w1) = user_tcp.split();

                        let tran = async move ||{
                            
                        };
                        
                        let res = tokio::select! {
                            res = io::copy(&mut r, &mut w1) => {
                                println!("🌈代理池中tcp断开");
                                is_client_disconnect = true;
                                res
                            },
                            res = io::copy(&mut r1, &mut w) => {
                                println!("🌈用户tcp断开");
                                res
                            },
                        }
                        .unwrap();
                        println!("{_user_addr} 传输结束{:?}", res);
                        if res == 0 {
                            break;
                        }
                    }

                    // 如果是代理客户端主动断开，则销毁当前连接
                    if is_client_disconnect {
                        tx.send(()).await;
                        let _ = Object::take(client_tcp);
                    }
                });
            };

            match rx.try_recv() {
                Ok(_) => {
                    println!("用户连接关闭，需检查代理连接是否关闭");
                    let mut buf = BytesMut::with_capacity(1);
                    if let Ok(Ok(size)) =
                        timeout(Duration::from_millis(300), tcp.peek(&mut buf)).await
                    {
                        if size == 0 {
                            println!("代理端也关闭了，需销毁用户服务器和代理服务器");
                            break;
                        }
                    };
                }
                Err(TryRecvError::Empty) => {
                    println!("正常监听中");
                    continue;
                }
                Err(TryRecvError::Disconnected) => {
                    println!("onshort 丢失，用户端断开");
                }
            };

            println!(" ❌用户连接超时");
        }
    }

    /// 创建代理服务器
    /// 用于接收 client 端的 tcp 连接，并把该连接加入到连接池中
    async fn create_proxy_server(&self) -> tokio::task::JoinHandle<()> {
        let tcp_pool: Arc<unmanaged::Pool<TcpStreamData>> = self.tcp_pool.clone();
        tokio::spawn(async move {
            let listener = TcpListener::bind("0.0.0.0:5533").await;
            if listener.is_err() {
                return;
            }
            let listener = listener.unwrap();
            println!("✅代理服务器池监听启动成功");

            loop {
                let res = listener.accept().await;
                if res.is_err() {
                    println!("❌获取代理连接失败{:?}", res);
                    break;
                }
                let (proxy_client, _) = res.unwrap();
                match tcp_pool.add(TcpStreamData::new(proxy_client)).await {
                    Ok(_) => println!("✅ 收到1个代理客户端连接成功"),
                    Err(e) => {
                        println!("❌代理连接添加失败{:?}", e.1);
                        break;
                    }
                };
            }
        })
    }
}

// async fn create_proxy_server()
#[tokio::main]
async fn main() -> io::Result<()> {
    let r_tcp_server = RTcpServer::new().await;
    let _ = r_tcp_server.create_connect_channel().await;

    Ok(())
}
