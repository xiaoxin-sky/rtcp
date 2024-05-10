use std::{borrow::BorrowMut, time::Duration};

use bytes::{Buf, BufMut, BytesMut};
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
        // while let Ok(msg) = self.parse_msg(&mut client_stream).await {
        //     match msg.message_type {
        //         RTCPType::Initialize(_) => println!("🔥客户端不需要实现"),
        //         RTCPType::NewConnection => {
        //             println!("🚀创建 back_end 新链接");
        //             self.create_proxy_connection().await;
        //         }
        //         RTCPType::CloseConnection => println!("🔥客户端不需要实现"),
        //     }
        // }
        match self.parse_msg(&mut client_stream).await {
            Ok(msg) => match msg.message_type {
                RTCPType::Initialize(_) => println!("🔥客户端不需要实现"),
                RTCPType::NewConnection => {
                    println!("🚀创建 back_end 新链接");
                    // self.create_proxy_connection();
                }
                RTCPType::CloseConnection => println!("🔥客户端不需要实现"),
            },
            Err(e) => {
                println!("解析消息出错,{:?}", e);
            }
        }
    }

    /// parse rtcp protocol
    async fn parse_msg(&self, tcp: &mut TcpStream) -> io::Result<RTCPMessage> {
        let mut buf = BytesMut::with_capacity(40 * 1024);

        loop {
            tcp.read_buf(&mut buf).await?;
            println!("读取长度 : {:?}", buf.len());

            if buf.is_empty() {
                return Err(io::Error::new(io::ErrorKind::Other, "server closed"));
            }

            // 一次读取的数据中可能包含多个 msg，需要全部解析出来
            loop {
                let res = RTCPMessage::deserialize(&buf);
                // 遇到错误读取错误, 退出当前循环，继续读取消息 TODO: 这里只应该处理解析长度不足的错误，其他错误都应该 rethrow
                if res.is_err() {
                    break;
                }

                let (rtcp_message, size) = res.unwrap();

                println!("消息大小: {:?}", size);

                buf.advance(size);

                match rtcp_message.message_type {
                    RTCPType::Initialize(_) => println!("🔥客户端不需要实现"),
                    RTCPType::NewConnection => {
                        self.create_proxy_connection();
                    }
                    RTCPType::CloseConnection => println!("🔥客户端不需要实现"),
                }
            }
        }
    }

    /// 创建 rtcp 服务器代理连接
    // async fn create_rtcp_proxy_connection(&self) -> TcpStream {
    //     let addr = format!("{}:5533", self.server_ip).parse().unwrap();
    //     let tcp = TcpSocket::new_v4().unwrap();
    //     tcp.connect(addr).await.unwrap()
    // }
    /// 创建后端连接池
    fn create_proxy_connection(&self) {
        // rtcp 服务器 tcp stream
        // let mut client_stream = self.create_rtcp_proxy_connection().await;
        let addr = format!("{}:5533", self.server_ip).parse().unwrap();

        // 真实后端连接池
        let back_end_pool = self.back_end_pool.clone();

        tokio::spawn(async move {
            let tcp = TcpSocket::new_v4().unwrap();
            let mut client_stream = tcp.connect(addr).await.unwrap();
            let mut b_tcp = back_end_pool.get().await.unwrap();
            // println!(
            //     "后端 tcp id: {:?} 池信息{:?}",
            //     b_tcp.id,
            //     back_end_pool.status()
            // );

            let (mut back_end_reader, mut back_end_writer) = b_tcp.stream.split();
            let (mut client_reader, mut client_writer) = client_stream.split();

            let is_back_end_close = loop {
                let (size, is_back_end_close) = tokio::select! {
                    res = io::copy(&mut back_end_reader, &mut client_writer) => {
                        // println!("🚌 后端读取结束并写入到代理客户端 {:?}",res);
                        let size = res.unwrap_or_default();

                        (size,true)
                    },
                    res = io::copy(&mut client_reader, &mut back_end_writer) => {
                        // println!("🔐 客户端读取并写入到后端 {:?}",res);
                        let size = res.unwrap_or_default();

                        (size,false)
                    },
                };

                if size == 0 {
                    break is_back_end_close;
                }
            };

            if is_back_end_close {
                b_tcp.disconnect = true;
            } else {
                b_tcp.latest_time = Some(std::time::Instant::now());
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
