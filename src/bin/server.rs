use std::{
    borrow::BorrowMut, collections::HashMap, marker::PhantomPinned, ops::DerefMut, sync::Arc,
    thread::sleep, time::Duration,
};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use deadpool::unmanaged::{self, Object};
use headers::{Header, HeaderMapExt, Server};
use rtcp::{
    manage::RTCPManager,
    parser::{parser_request_head_all, RequestLine},
    protocol::{RTCPMessage, RTCPType},
    tcp_pool::{Pool, TcpPoolManager, TcpStreamData},
};
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    join,
    net::{tcp::WriteHalf, TcpListener, TcpStream},
    sync::{
        mpsc::{self, error::TryRecvError},
        oneshot::{self, Sender},
        Mutex, RwLock,
    },
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
                        println!("通讯连接成功，启用代理服务器{port}");
                        self.create_proxy_server().await;
                        self.create_user_server(port, &mut tcp).await;
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
        let tcp_pool = self.tcp_pool.clone();

        loop {
            let (mut user_tcp, _user_addr) = listener.accept().await.unwrap();
            let tcp_pool = tcp_pool.clone();

            let pool_status = tcp_pool.status();
            println!("收到请求:{_user_addr}  {pool_status:?}");

            if pool_status.available == 0 {
                println!("🚀连接池已满,发送创建新链接消息");
                let msg = RTCPMessage::new(RTCPType::NewConnection);
                let res = tcp.write_all(&msg.serialize()).await;
                println!("写入创建新消息结果{:?}", res);
                if res.is_err() {
                    break;
                }
                let _ = tcp.flush().await;
                println!(" 🚀发送创建新链接消息成功");
            }
            tokio::spawn(async move {
                let mut client_tcp = tcp_pool.get().await.unwrap();
                loop {
                    let (mut r, mut w) = client_tcp.stream.split();
                    let (mut r1, mut w1) = user_tcp.split();
                    let res = tokio::select! {
                        res = io::copy(&mut r, &mut w1) => res,
                        res = io::copy(&mut r1, &mut w) => res,
                    }
                    .unwrap();
                    println!("{_user_addr} 传输结束{:?}", res);
                    if res == 0 {
                        break;
                    }
                }
                let _ = Object::take(client_tcp);
                let pool_status = tcp_pool.status();
                println!("剩余{pool_status:?}");
            });
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

            loop {
                println!("进行");
                let res = listener.accept().await;
                if res.is_err() {
                    println!("❌获取代理连接失败{:?}", res);
                    break;
                }
                let (proxy_client, _) = res.unwrap();
                match tcp_pool.add(TcpStreamData::new(proxy_client)).await {
                    Ok(_) => println!("🚀 新1个代理客户端连接成功"),
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

struct HttpTransformer {
    tcp: TcpStream,

    /// 请求首部
    request_head: Option<RequestHead>,

    /// 请求体信息
    /// 可能没有请求体
    request_body_state: Option<RequestBodyState>,
    _marker: PhantomPinned,
}

/// 请求首部
struct RequestHead {
    request_line: RequestLine,
    headers: HashMap<String, String>,
}

impl RequestHead {
    /// 构造请求头
    pub fn build_request_head(&mut self) -> BytesMut {
        let request_line_byte = self.request_line.to_byte();

        let mut request_head = BytesMut::from_iter(request_line_byte);

        for (key, value) in self.headers.iter_mut() {
            let header = format!("{}: {}\r\n", key, value);
            request_head.put_slice(header.as_bytes());
            // request_head.extend_from_slice(key.as_bytes());
            // request_head.extend_from_slice(b": ");
            // request_head.extend_from_slice(value.as_bytes());
            // request_head.extend_from_slice(b"\r\n");
        }

        request_head.put_slice(b"\r\n");

        request_head
    }

    /// 获取请求头长度
    pub fn get_content_length(&self) -> Option<String> {
        self.headers.get("Content-Length").cloned()
    }
}

struct RequestBodyState {
    /// 请求体开始位置
    request_body_index: usize,
}

impl HttpTransformer {
    pub fn new(tcp_stream: TcpStream) -> Self {
        Self {
            tcp: tcp_stream,
            request_body_state: None,
            request_head: None,
            _marker: PhantomPinned,
        }
    }

    /// 解析请求头
    fn parse_header(&mut self, buf: &mut BytesMut) -> Result<usize, ()> {
        match parser_request_head_all(buf) {
            Ok((rest, (request_line, headers))) => {
                self.request_head = Some(RequestHead {
                    request_line,
                    headers,
                });
                let head_len = buf.len() - rest.len();
                Ok(head_len)
            }
            Err(_) => Err(()),
        }
    }

    /// 转发数据
    // fn proxy(){
    //     // 转换请求头
    //     if let Some(val) = self.headers.get_mut("Host") {
    //         *val = "127.0.0.1:9930".to_string();
    //     };

    //     // 替换原始请求头,并写入
    //     let head_byte = self.build_request_head();
    //     let _ = self.tcp.write_all(&head_byte).await;

    //     buf.advance(size);

    //     let _ = self.tcp.write_all(&buf).await;

    //     println!("读取大小:{:?},剩余长度{}", size, buf.len());
    // }

    pub async fn run(&mut self) -> io::Result<BytesMut> {
        let mut buf = BytesMut::with_capacity(4 * 1024);

        loop {
            match self.tcp.read_buf(&mut buf).await {
                Ok(size) => {
                    if size == 0 {
                        println!("✅退出");
                        break;
                    }

                    if self.request_head.is_none() {
                        // 头部没有读取完整，继续读取
                        let res = self.parse_header(&mut buf);
                        if res.is_err() {
                            continue;
                        }

                        // 头部解析完之后，丢弃掉头部的数据
                        let head_len = res.unwrap();
                        buf.advance(head_len);
                    }

                    let request_head = self.request_head.as_ref().unwrap();
                    if let Some(content_len) = request_head.get_content_length() {
                        // 如果当前读取的数据没有超过内容长度，继续读取
                        if buf.len() < content_len.parse::<usize>().unwrap() {
                            continue;
                        }
                        break;
                    };
                    break;
                }
                Err(e) => {
                    eprintln!("{e:?}");
                    break;
                }
            };
        }
        Ok(buf)
    }
}
