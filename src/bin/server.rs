use std::{collections::HashMap, io::Error, marker::PhantomPinned};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use headers::{Header, HeaderMapExt};
use rtcp::parser::{parser_request_head_all, RequestLine};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

/// 创建通道服务器
async fn create_connect_channel() -> io::Result<TcpStream> {
    let tcp_listener = TcpListener::bind("0.0.0.0:5541").await?;
    
    loop {
        match tcp_listener.accept().await {
            Ok(stream) => {
                println!("通道服务器连接成功");
                return Ok(stream.0);
            }
            Err(e) => {
                println!("❌通道服务器连接失败{:?}", e);
                continue
            },
        };
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let client_tcp = create_connect_channel().await?;

    let client_tcp = Arc::new(Mutex::new(client_tcp));

    let addr = "0.0.0.0:9931";
    let tcp_listener = TcpListener::bind(addr).await?;
    loop {
        let (mut tcp_stream, socket_addr) = tcp_listener.accept().await?;
        tokio::spawn(async move {
            let ip = socket_addr.ip().to_string();
            println!("{ip}");
            let mut buf = BytesMut::with_capacity(4 * 1024);

            let read_res = tcp_stream.read_buf(&mut buf).await;
            if read_res.is_err() {
                eprintln!("读取错误{:?}", read_res);
                return;
            }
            let read_res = read_res.unwrap();

            // let mut http_transformer = HttpTransformer::new(tcp_stream);
            // let _ = http_transformer.run().await;
        });
    }
}

struct HttpTransformer {
    tcp: TcpStream,

    /// 请求首部
    request_head: Option<RequestHead>,

    /// 请求体信息
    /// 可能没有请求体
    request_body_state: Option<RequestBodyState>,
    _marker: PhantomPinned,
}

/// 请求首部
struct RequestHead {
    request_line: RequestLine,
    headers: HashMap<String, String>,
}

impl RequestHead {
    /// 构造请求头
    pub fn build_request_head(&mut self) -> BytesMut {
        let request_line_byte = self.request_line.to_byte();

        let mut request_head = BytesMut::from_iter(request_line_byte);

        for (key, value) in self.headers.iter_mut() {
            let header = format!("{}: {}\r\n", key, value);
            request_head.put_slice(header.as_bytes());
            // request_head.extend_from_slice(key.as_bytes());
            // request_head.extend_from_slice(b": ");
            // request_head.extend_from_slice(value.as_bytes());
            // request_head.extend_from_slice(b"\r\n");
        }

        request_head.put_slice(b"\r\n");

        request_head
    }

    /// 获取请求头长度
    pub fn get_content_length(&self) -> Option<String> {
        self.headers.get("Content-Length").cloned()
    }
}

struct RequestBodyState {
    /// 请求体开始位置
    request_body_index: usize,
}

impl HttpTransformer {
    pub fn new(tcp_stream: TcpStream) -> Self {
        Self {
            tcp: tcp_stream,
            request_body_state: None,
            request_head: None,
            _marker: PhantomPinned,
        }
    }

    /// 解析请求头
    fn parse_header(&mut self, buf: &mut BytesMut) -> Result<usize, ()> {
        match parser_request_head_all(buf) {
            Ok((rest, (request_line, headers))) => {
                self.request_head = Some(RequestHead {
                    request_line,
                    headers,
                });
                let head_len = buf.len() - rest.len();
                Ok(head_len)
            }
            Err(_) => Err(()),
        }
    }

    /// 转发数据
    // fn proxy(){
    //     // 转换请求头
    //     if let Some(val) = self.headers.get_mut("Host") {
    //         *val = "127.0.0.1:9930".to_string();
    //     };

    //     // 替换原始请求头,并写入
    //     let head_byte = self.build_request_head();
    //     let _ = self.tcp.write_all(&head_byte).await;

    //     buf.advance(size);

    //     let _ = self.tcp.write_all(&buf).await;

    //     println!("读取大小:{:?},剩余长度{}", size, buf.len());
    // }

    pub async fn run(&mut self) -> io::Result<()> {
        let mut buf = BytesMut::with_capacity(4 * 1024);

        let start = std::time::Instant::now();
        loop {
            match self.tcp.read_buf(&mut buf).await {
                Ok(size) => {
                    if size == 0 {
                        println!("✅退出");
                        break;
                    }

                    if self.request_head.is_none() {
                        // 头部没有读取完整，继续读取
                        let res = self.parse_header(&mut buf);
                        if res.is_err() {
                            continue;
                        }

                        // 头部解析完之后，丢弃掉头部的数据
                        let head_len = res.unwrap();
                        buf.advance(head_len);
                    }

                    let request_head = self.request_head.as_ref().unwrap();
                    if let Some(content_len) = request_head.get_content_length() {
                        // 如果当前读取的数据没有超过内容长度，继续读取
                        if buf.len() < content_len.parse::<usize>().unwrap() {
                            continue;
                        }
                        break;
                    };
                    break;
                }
                Err(e) => {
                    eprintln!("{e:?}");
                    break;
                }
            };
        }
        println!("读取结束");
        let end = std::time::Instant::now();

        println!("读取耗时:{:?}", end.duration_since(start));

        let _ = self
            .tcp
            .write(b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/html;charset=utf-8\r\n\r\n")
            .await;

        let _ = self
            .tcp
            .write(format!("{}MiByte", ((buf.len() as f64) / 1024.0 / 1024.0)).as_bytes())
            .await;
        let _ = self.tcp.flush().await;

        Ok(())
    }
}
