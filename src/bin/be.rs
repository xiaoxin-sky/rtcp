use bytes::BytesMut;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};

#[tokio::main]
async fn main() {
    let a = TcpListener::bind("0.0.0.0:8083").await.unwrap();
    loop {
        let (mut stream, addr) = a.accept().await.unwrap();
        println!("addr 连接");
        tokio::spawn(async move {
            let mut buf = BytesMut::with_capacity(4 * 1024);
            loop {
                let len = stream.read_buf(&mut buf).await.unwrap();
                let data = buf.clone();
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length:{}\r\nContent-Type:text/plain; Charset=utf-8\r\n\r\n", data.len());
                stream.write_all(head.as_bytes()).await.unwrap();
                stream.write_all(&data).await.unwrap();
                let _ =stream.flush().await;

            }
        });
    }
}
