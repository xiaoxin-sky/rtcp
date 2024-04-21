use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{tcp, TcpSocket},
};

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:8080".parse().unwrap();
    let tcp = TcpSocket::new_v4().unwrap();
    let mut client_stream = tcp.connect(addr).await.unwrap();

    client_stream.write_all(b"Hello, world!").await.unwrap();

    let mut buf = BytesMut::with_capacity(4 * 1024);
    loop {
        let a = client_stream.read_buf(&mut buf).await;
    }
}
