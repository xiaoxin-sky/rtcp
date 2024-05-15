use std::time::Duration;

use bytes::{Buf, BytesMut};
use clap::Parser;
use rtcp::{
    protocol::{RTCPMessage, RTCPType},
    tcp_pool::{Pool, TcpPoolManager},
};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{tcp::OwnedReadHalf, TcpSocket, TcpStream},
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

    proxy_pool: Pool,
}

impl Client {
    pub fn new(backend_ip: String, backend_port: u16, server_ip: String) -> Self {
        let mgr = TcpPoolManager::new("nestjs".to_string(), backend_ip, backend_port);
        let back_end_pool = Pool::builder(mgr).build().unwrap();

        let mgr_proxy = TcpPoolManager::new("mgr_proxy".to_string(), server_ip.clone(), 5533);
        let proxy_pool = Pool::builder(mgr_proxy).build().unwrap();

        Client {
            back_end_pool,
            server_ip,
            proxy_pool,
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

            let (reader_stream, mut writer_stream) = client_stream.into_split();
            tokio::spawn(async move {
                loop {
                    sleep(Duration::from_secs(10)).await;
                    let msg = RTCPMessage::new(RTCPType::Heartbeat);
                    writer_stream.write_all(msg.serialize().as_ref()).await;
                    writer_stream.flush().await;
                }
            });

            self.server_msg_handel(reader_stream).await;
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

    async fn server_msg_handel(&self, mut client_stream: OwnedReadHalf) {
        let mut buf = BytesMut::with_capacity(40 * 1024);

        loop {
            client_stream.read_buf(&mut buf).await.unwrap();

            if buf.is_empty() {
                println!("❌读取为空，5541 服务器断开连接");
                break;
            }

            // 一次读取的数据中可能包含多个 msg，需要全部解析出来
            loop {
                let res = RTCPMessage::deserialize(&buf);
                // 遇到错误读取错误, 退出当前循环，继续读取消息 TODO: 这里只应该处理解析长度不足的错误，其他错误都应该 rethrow
                if res.is_err() {
                    break;
                }

                let (rtcp_message, size) = res.unwrap();

                buf.advance(size);

                match rtcp_message.message_type {
                    RTCPType::Initialize(_) => println!("🔥客户端不需要实现"),
                    RTCPType::NewConnection => {
                        self.create_proxy_connection();
                        println!("✅创建连接成功");
                    }
                    RTCPType::CloseConnection => println!("🔥客户端不需要实现"),
                    RTCPType::Heartbeat => {}
                }
            }
        }
    }

    /// 创建后端连接池
    fn create_proxy_connection(&self) {
        // 真实后端连接池
        let back_end_pool = self.back_end_pool.clone();
        let proxy_pool = self.proxy_pool.clone();

        tokio::spawn(async move {
            let mut b_tcp = back_end_pool.get().await.unwrap();
            let mut proxy_stream = proxy_pool.get().await.unwrap();

            let (mut back_end_reader, mut back_end_writer) = b_tcp.stream.split();
            let (mut client_reader, mut client_writer) = proxy_stream.stream.split();

            let is_back_end_close = loop {
                let (size, is_back_end_close) = tokio::select! {
                    res = io::copy(&mut back_end_reader, &mut client_writer) => {
                        // println!("🚌 后端读取结束并写入到代理客户端 {:?}",res);
                        let size = res.unwrap_or_default();

                        (size,true)
                    },
                    res = io::copy(&mut client_reader, &mut back_end_writer) => {
                        // println!("🔐 用户客户端读取并写入到后端 {:?}",res);
                        let size = res.unwrap_or_default();

                        (size,false)
                    },
                };

                if size == 0 {
                    break is_back_end_close;
                }
            };

            proxy_stream.disconnect = true;
            proxy_stream.stream.shutdown().await;
            if is_back_end_close {
                b_tcp.disconnect = true;
                // proxy_stream.latest_time = Some(std::time::Instant::now());
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
