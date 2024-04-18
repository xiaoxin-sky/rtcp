use std::{io::BufRead, pin::Pin};

use bytes::BytesMut;
use tokio::{
    io::{self, AsyncReadExt},
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
            http_transformer.rewrite_host("127.0.0.1".to_string()).await;
            http_transformer.run().await;
        });
    }
}

struct HttpTransformer {
    host: Option<String>,
    rd: OwnedReadHalf,
    wr: OwnedWriteHalf,
    cursor: usize,
}

impl HttpTransformer {
    pub fn new(tcp_stream: TcpStream) -> Self {
        let (rd, wr) = tcp_stream.into_split();
        Self {
            host: None,
            rd,
            wr,
            cursor: 0,
        }
    }

    pub async fn rewrite_host(&mut self, new_host: String) {
        self.host = Some(new_host);
    }

    fn parse_header(&self, buf: &mut BytesMut) {
        let _ = buf.lines().map(|item| {
            let a = item.unwrap();
            println!("{a}");
        });
    }

    pub async fn run(&mut self) {
        let mut buf = BytesMut::with_capacity(4 * 1024);

        // loop {
        let n = self.rd.read_buf(&mut buf).await.unwrap();
        println!("执行{:?}", buf);
        // self.parse_header(&mut buf);
        let lines = buf.lines();
        for a in lines.into_iter() {
            
            println!("{:?}", a);
        }

        // if n == 0 {
        //     println!("读取完毕{:?}", buf);
        //     break;
        // } else {
        //     self.cursor += n;
        //     println!("{n}");
        // }
        // }
    }
}
