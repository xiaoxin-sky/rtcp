use std::sync::{Arc, Mutex};

use bytes::BytesMut;
use rtcp::protocol::RTCPMessage;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{tcp, TcpSocket},
};

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:5541".parse().unwrap();
    let tcp = TcpSocket::new_v4().unwrap();
    let mut client_stream = tcp.connect(addr).await.unwrap();

    let init_msg = RTCPMessage::new(rtcp::protocol::RTCPType::Initialize, BytesMut::from(""));

    client_stream
        .write_all(&init_msg.serialize())
        .await
        .unwrap();

    let mut buf = Arc::new(Mutex::new(BytesMut::with_capacity(4 * 1024)));

    loop {
        let mut buf = buf.lock().unwrap();
        let mut a = (*buf).clone();
        drop(buf);
        match client_stream.read_buf(&mut a).await {
            Ok(_) => {
                let rtcp_msg = RTCPMessage::deserialize(a);
                if rtcp_msg.is_err() {
                    continue;
                }
                let rtcp_msg = rtcp_msg.unwrap();
                match rtcp_msg.message_type {
                    rtcp::protocol::RTCPType::NewConnection => {
                        println!("收到新连接: {:?}", rtcp_msg.connect_id);
                    }
                    rtcp::protocol::RTCPType::Transformation(byte_size) => {
                        println!("收到数据，长度: {}", byte_size);
                    }
                    rtcp::protocol::RTCPType::CloseConnection => {
                        println!("关闭连接")
                    }
                    rtcp::protocol::RTCPType::Initialize => {
                        println!("client 不需要实现")
                    }
                }
            }
            Err(e) => {
                println!("❌Error: {}", e);
                break;
            }
        };
    }
}
