use std::{borrow::BorrowMut, time::Duration};

use bytes::BytesMut;
use clap::Parser;
use deadpool::managed::Object;
use rtcp::{
    protocol::{RTCPMessage, RTCPType},
    tcp_pool::{Pool, TcpPoolManager},
};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpSocket, TcpStream},
    time::sleep,
};
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// 被代理服务器 ip
    #[arg(short, long)]
    ip: String,

    /// 被代理服务器端口
    #[arg(short, long)]
    port: u16,

    /// 访问端口
    #[arg(short, long)]
    access_port: u16,

    /// rtcp 服务器ip
    #[arg(short, long)]
    server: String,
}
pub struct Client {
    back_end_pool: Pool,
    /// rtcp 服务器ip
    server_ip: String,
}

// impl Default for Client {
//     fn default() -> Self {
//         Self::new()
//     }
// }

impl Client {
    pub fn new(backend_ip: String, backend_port: u16, server_ip: String) -> Self {
        let mgr = TcpPoolManager::new("nestjs".to_string(), backend_ip, backend_port);
        let back_end_pool = Pool::builder(mgr).build().unwrap();
        Client {
            back_end_pool,
            server_ip,
        }
    }

    /// 启动代理
    pub async fn start(&self, access_port: u16) {
        loop {
            let addr = format!("{}:5541", self.server_ip).parse().unwrap();
            let tcp = TcpSocket::new_v4().unwrap();

            let connect_res = tcp.connect(addr).await;
            if connect_res.is_err() {
                println!("❌连接失败，开始重试,{:?}", connect_res);
                sleep(Duration::from_secs(1)).await;
                continue;
            }

            let mut client_stream = connect_res.unwrap();

            self.send_init_msg(&mut client_stream, access_port).await;
            self.server_msg_handel(client_stream).await;
        }
    }

    async fn send_init_msg(&self, client_stream: &mut TcpStream, access_port: u16) {
        let init_msg = RTCPMessage::new(RTCPType::Initialize(access_port));

        client_stream
            .write_all(&init_msg.serialize())
            .await
            .unwrap();
        client_stream.flush().await.unwrap();
    }

    async fn server_msg_handel(&self, mut client_stream: TcpStream) {
        while let Ok(msg) = self.parse_msg(&mut client_stream).await {
            match msg.message_type {
                RTCPType::Initialize(_) => println!("🔥客户端不需要实现"),
                RTCPType::NewConnection => {
                    println!("🚀创建 back_end 新链接");
                    self.create_proxy_connection().await;
                }
                RTCPType::CloseConnection => println!("🔥客户端不需要实现"),
            }
        }
    }

    /// parse rtcp protocol
    async fn parse_msg(&self, tcp: &mut TcpStream) -> io::Result<RTCPMessage> {
        let mut buf = BytesMut::with_capacity(4 * 1024);

        loop {
            tcp.read_buf(&mut buf).await?;
            if buf.is_empty() {
                return Err(io::Error::new(io::ErrorKind::Other, "server closed"));
            }

            let res = RTCPMessage::deserialize(&buf);

            if res.is_err() {
                continue;
            }

            let (rtcp_message, _size) = res.unwrap();

            return Ok(rtcp_message);
        }
    }

    /// 创建后端连接池
    async fn create_proxy_connection(&self) {
        let back_end_pool = self.back_end_pool.clone();
        let addr = format!("{}:5533", self.server_ip).parse().unwrap();

        tokio::spawn(async move {
            let tcp = TcpSocket::new_v4().unwrap();
            let mut client_stream = tcp.connect(addr).await.unwrap();

            loop {
                let mut b_tcp = back_end_pool.get().await.unwrap();
                let (mut r, mut w) = b_tcp.stream.split();
                let (mut r1, mut w1) = client_stream.split();

                let (is_back_end_close, size) = tokio::select! {
                    res = io::copy(&mut r, &mut w1) => {
                        (true,res.unwrap_or_default())
                    },
                    res = io::copy(&mut r1, &mut w) => {
                        (false,res.unwrap_or_default())
                    },
                };

                if size == 0 {
                    if is_back_end_close {
                        // drop back_end_tcp from pool
                        let _ = Object::take(b_tcp);
                    }
                    break;
                }
            }
        });
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let client = Client::new(args.ip, args.port, args.server);
    client.start(args.access_port).await;
}
