use std::{
    collections::HashMap,
    io::BufRead,
    marker::PhantomPinned,
};

use bytes::BytesMut;
use nom::{bytes::{complete::tag, streaming::take_while1}, character::is_alphabetic};
use rtcp::parser::parser_request_line;
use tokio::{
    io::{self, split, AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
};

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr = "0.0.0.0:9931";
    let tcp_listener = TcpListener::bind(addr).await?;
    loop {
        let (tcp_stream, socket_addr) = tcp_listener.accept().await?;
        tokio::spawn(async move {
            let ip = socket_addr.ip().to_string();
            println!("{ip}");

            let mut http_transformer = HttpTransformer::new(tcp_stream);
            // http_transformer.rewrite_host("127.0.0.1".to_string()).await;
            http_transformer.run().await;
        });
    }
}

struct HttpTransformer {
    headers: HashMap<String, String>,
    tcp: TcpStream,
    _marker: PhantomPinned,
}

impl HttpTransformer {
    pub fn new(tcp_stream: TcpStream) -> Self {
        Self {
            headers: HashMap::new(),
            tcp: tcp_stream,
            _marker: PhantomPinned,
        }
    }

    // pub async fn rewrite_host(&mut self, new_host: String) {
    //     self.host = Some(new_host);
    // }

    /// 解析请求头
    fn parse_header(&mut self, buf: &mut BytesMut) -> io::Result<bool> {
        let lines = buf.lines();
        for row in lines.into_iter() {
            match row {
                Ok(line) => {
                    if line.is_empty() {
                        continue;
                    }

                    // if line.starts_with(""){}
                    // 匹配第一个冒号，作为key，剩余的作为 value
                    let mut a = line.splitn(2, ':');
                    let key = a.next().unwrap_or("");
                    let value = a.next().unwrap_or("");
                    if value.eq("") || key.eq("") {
                        continue;
                    }
                    self.headers
                        .insert(key.trim().to_string(), value.trim().to_string());
                    println!("{key}:{value}");
                }
                Err(e) => {
                    eprintln!("❌{e:?}");
                    return Err(e);
                }
            }
        }
        println!("{:?}", self.headers);
        Ok(false)
    }

    pub async fn run(&mut self) -> io::Result<()> {
        // let mut buf = BytesMut::with_capacity(4 * 1024);
        let mut vec = Vec::new();
        let a = self.tcp.read_to_end(&mut vec).await.unwrap();
        println!("读取大小{a}读取内容{:?}", String::from_utf8(vec.clone()));
        let res = parser_request_line(vec.as_slice());
        println!("{:?}", res);
        // loop {
        //     match self.tcp.read_buf(&mut buf).await {
        //         Ok(size) => {
        //             // self.parse_header(&mut buf)?;

        //             if size == 0 {
        //                 println!("✅退出");
        //                 break;
        //             }

        //             println!("读取大小:{:?}", size);
        //         }
        //         Err(e) => {
        //             eprintln!("{e:?}");
        //             break;
        //         }
        //     };
        // }
        // let a = self.parse_header(&mut buf)?;
        // println!("{:?}", buf);
        self.tcp
            .write_all(b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\n\r\nhello world!")
            .await;
        self.tcp.flush().await;

        Ok(())
    }
}
