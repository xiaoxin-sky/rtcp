use std::{collections::HashMap, marker::PhantomPinned};

use bytes::{Buf, BufMut, BytesMut};
use tokio::{
    io::{self, AsyncReadExt},
    net::TcpStream,
};

use crate::parser::{parser_request_head_all, RequestLine};

pub struct HttpTransformer {
    // pub tcp: TcpStream,

    /// 请求首部
    pub request_head: Option<RequestHead>,

    _marker: PhantomPinned,
}

/// 请求首部
#[derive(Debug)]
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
        }

        request_head.put_slice(b"\r\n");

        request_head
    }

    /// 获取请求头长度
    pub fn get_content_length(&self) -> Option<String> {
        self.headers.get("Content-Length").cloned()
    }
}

impl HttpTransformer {
    pub fn new() -> Self {
        Self {
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


    /// 从 stream 中读取请求头
    async fn read_request_header<'a>(&self, r: &mut tokio::net::tcp::ReadHalf<'a>) {
        let mut buf = BytesMut::with_capacity(4 * 1024);

        loop {
            match r.read_buf(&mut buf).await {
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

    }

    pub async fn run<'a>(&mut self, r: &mut tokio::net::tcp::ReadHalf<'a>){
        self.read_request_header(r).await;
        println!("请求头解析完成{:?}",self.request_head);
        return self.;
    }
}
