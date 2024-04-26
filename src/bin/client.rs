use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use bytes::{Buf, BytesMut};
use rtcp::{
    protocol::{RTCPMessage, RTCPType},
    tcp_pool::{Pool, TcpPoolManager},
};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{tcp, TcpSocket, TcpStream},
    time::sleep,
};

pub struct Client {
    back_end_pool: Pool,
}

impl Client {
    pub fn new() -> Self {
        let mgr = TcpPoolManager::new("nestjs".to_string(), "127.0.0.1".to_string(), 8083);
        let back_end_pool = Pool::builder(mgr).build().unwrap();
        Client { back_end_pool }
    }

    /// 启动代理
    pub async fn start(&self) {
        let addr = "127.0.0.1:5541".parse().unwrap();
        let tcp = TcpSocket::new_v4().unwrap();
        let mut client_stream = tcp.connect(addr).await.unwrap();

        let init_msg = RTCPMessage::new(RTCPType::Initialize(3361));
        client_stream
            .write_all(&init_msg.serialize())
            .await
            .unwrap();
        loop {
            println!("等待读取消息");
            match self.read_msg(&mut client_stream).await {
                Ok(msg) => {
                    println!("{:?}", msg.message_type);
                    self.msg_handel(msg).await;
                }
                Err(e) => {
                    println!("❌读取消息失败,{:?}", e);
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };
        }
    }

    async fn read_msg(&self, tcp: &mut TcpStream) -> io::Result<RTCPMessage> {
        let mut buf = BytesMut::with_capacity(4 * 1024);
        loop {
            tcp.read_buf(&mut buf).await?;
            if buf.is_empty() {
                return Err(io::Error::new(io::ErrorKind::Other, "读取数据失败"));
            }

            let res = RTCPMessage::deserialize(&buf);

            if res.is_err() {
                println!("序列化失败,继续读取");
                continue;
            }

            let (rtcp_message, _size) = res.unwrap();

            return Ok(rtcp_message);
        }
    }

    async fn msg_handel(&self, msg: RTCPMessage) {
        match msg.message_type {
            RTCPType::Initialize(_) => println!("🔥客户端不需要实现"),
            RTCPType::NewConnection => {
                println!("🚀创建back_end新链接");
                self.create_proxy_connection().await;
            }
            RTCPType::CloseConnection => println!("🔥客户端不需要实现"),
        }
    }

    /// 创建后端连接池
    async fn create_proxy_connection(&self) {
        let back_end_pool = self.back_end_pool.clone();
        tokio::spawn(async move {
            let addr = "127.0.0.1:5533".parse().unwrap();
            let tcp = TcpSocket::new_v4().unwrap();
            let mut client_stream = tcp.connect(addr).await.unwrap();

            // let mut data = BytesMut::with_capacity(4 * 1024);
            loop {
                // let res = client_stream.read_buf(&mut data).await.unwrap();
                // println!("读取结果:{:?}", res);
                // if res == 0 {
                //     break;
                // }
                let peek_addr = client_stream.peer_addr();
                println!("{:?}等待写入", peek_addr);
                let mut b_tcp = back_end_pool.get().await.unwrap();
                let (mut r, mut w) = b_tcp.stream.split();
                let (mut r1, mut w1) = client_stream.split();

                let res = tokio::select! {
                    res = io::copy(&mut r, &mut w1) => res,
                    res = io::copy(&mut r1, &mut w) => res,
                }
                .unwrap();

                println!("{:?}写入完成:{:?}", peek_addr, res);
                if res == 0 {
                    break;
                }
            }

            // loop {
            //     match back_end_pool.get().await {
            //         Ok(mut back_tcp) => {
            //             let (mut s1_read, mut s1_write) = client_stream.split();

            //             let (mut s2_read, mut s2_write) = back_tcp.stream.split();

            //             loop {
            //                 println!(" aaa转发请求");
            //                 let res = io::copy(&mut s1_read, &mut s2_write).await;

            //                 if res.is_err() {
            //                     break;
            //                 }
            //                 let res = res.unwrap();
            //                 println!("代理传输完成,{:?}", res);
            //             }

            //             loop {
            //                 let res = io::copy(&mut s2_read, &mut s1_write).await;

            //                 if res.is_err() {
            //                     break;
            //                 }
            //                 let res = res.unwrap();
            //                 println!("代理响应完成,{:?}", res);
            //             }

            //             println!("结束");
            //         }
            //         Err(e) => {
            //             println!(" ❌获取后端 tcp链接失败: {:?}", e);
            //             break;
            //         }
            //     }
            // }
        });
    }
}

#[tokio::main]
async fn main() {
    let client = Client::new();
    client.start().await;
}
