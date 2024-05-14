use std::{
    sync::Arc,
    time::{self, SystemTime, UNIX_EPOCH},
};

use bytes::BytesMut;
use deadpool::unmanaged::{self, Object};
use rtcp::{
    protocol::{RTCPMessage, RTCPType},
    tcp_pool::TcpStreamData,
    transformer::HttpTransformer,
};
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Sender},
    task::JoinHandle,
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
                    println!("收到rtcp client新连接");
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

    async fn client_handle(self: Arc<Self>, tcp: TcpStream) {
        let (mut read_half, mut write_half) = tcp.into_split();
        let mut new_poll_connect_handle: Option<JoinHandle<()>> = None;
        let mut proxy_server_handle: Option<JoinHandle<()>> = None;
        let mut user_server_handle: Option<JoinHandle<()>> = None;

        // client 连接池不够用时候，发送创建新连接的消息
        let (tx, mut rx) = mpsc::channel::<()>(1000);

        new_poll_connect_handle = Some(tokio::spawn(async move {
            loop {
                if rx.recv().await.is_some() {
                    let msg = RTCPMessage::new(RTCPType::NewConnection);
                    write_half.write_all(&msg.serialize()).await.unwrap();
                    write_half.flush().await.unwrap();
                }
            }
        }));

        loop {
            let msg = self.read_msg(&mut read_half).await;

            if msg.is_err() {
                println!("❌读取消息失败,关闭当前client 连接{:?}", msg);
                if let Some(handle) = new_poll_connect_handle.take() {
                    handle.abort();
                }
                if let Some(handle) = user_server_handle.take() {
                    handle.abort();
                }
                if let Some(handle) = proxy_server_handle.take() {
                    handle.abort();
                }
                return;
            }

            let msg = msg.unwrap();

            println!("读取消息: {}", msg.message_type);
            match msg.message_type {
                RTCPType::Initialize(port) => {
                    proxy_server_handle = Some(self.create_proxy_server().await);
                    user_server_handle = Some(self.create_user_server(port, tx.clone()).await);
                }
                RTCPType::NewConnection => {
                    println!("🔥不需要实现")
                }
                RTCPType::CloseConnection => println!("🔥不需要实现"),
            }
        }
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
    async fn create_user_server(
        &self,
        port: u16,
        sender: Sender<()>,
    ) -> tokio::task::JoinHandle<()> {
        let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
        println!("✅[{port}]用户服务器端口启动成功");
        let tcp_pool = self.tcp_pool.clone();

        tokio::spawn(async move {
            loop {
                if let Ok((mut user_tcp, user_addr)) = listener.accept().await {
                    if tcp_pool.status().available == 0 {
                        sender.send(()).await.unwrap();
                    }
                    
                    let mut client_tcp = tcp_pool.get().await.unwrap();

                    tokio::spawn(async move {
                        let (mut client_reader, mut client_writer) = client_tcp.stream.split();
                        let (mut user_reader, mut user_writer) = user_tcp.split();

                        let mut http_transformer = HttpTransformer::new(user_addr);

                        let is_client_disconnect = loop {
                            let (res, is_client_disconnect) = tokio::select! {
                                res = io::copy(&mut user_reader, &mut client_writer) => {
                                    // println!("🔐 用户发送到代理池 {res:?}");
                                    (res.unwrap_or_default(),false)
                                },
                                res = http_transformer.copy(&mut client_reader, &mut user_writer) => {
                                    // println!("🌈 代理池服务器响应到用户 {res:?} {:?}",id);
                                    (res.unwrap_or_default(),true)
                                },
                            };

                            if res == 0 {
                                // println!(
                                //     "传输断开  是否为代理客户端断开{:?}",
                                //     is_client_disconnect
                                // );
                                break is_client_disconnect;
                            }
                        };

                        let mut client_tcp = Object::take(client_tcp);
                        client_tcp.stream.shutdown().await;
                        // if is_client_disconnect {
                        //     let mut client_tcp = Object::take(client_tcp);
                        //     client_tcp.stream.shutdown().await;
                        // } else {
                        // client_tcp.latest_time = Some(std::time::Instant::now());
                        // }
                    });
                };
            }
        })
    }

    /// 创建代理服务器
    /// 用于接收 client 端的 tcp 连接，并把该连接加入到连接池中
    async fn create_proxy_server(&self) -> tokio::task::JoinHandle<()> {
        let tcp_pool = self.tcp_pool.clone();
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
                    Ok(_) => {
                        println!("✅ 收到1个代理客户端连接成功");
                    }
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
