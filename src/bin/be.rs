use bytes::BytesMut;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};

#[tokio::main]
async fn main() {
    let a = TcpListener::bind("0.0.0.0:8082").await.unwrap();
    loop {
        let (mut stream, addr) = a.accept().await.unwrap();
        println!("addr 连接");
        tokio::spawn(async move {
            let mut buf = BytesMut::with_capacity(4 * 1024);
            loop {
                let len = stream.read_buf(&mut buf).await.unwrap();
                let data = format!("读取长度:{}", len);
                println!("{data}");
                let data = format!("HTTP/1.1 200 OK\r\nContent-Length:{}\r\nContent-Type:text/plain; Charset=utf-8\r\n\r\n{}", data.len(), data);
                stream.write_all(data.as_bytes()).await.unwrap();
                let _ =stream.flush().await;

            }
        });
    }
}
