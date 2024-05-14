use std::{collections::HashMap, marker::PhantomPinned, net::SocketAddr};

use bytes::{Buf, BufMut, BytesMut};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::parser::{parser_request_head_all, RequestLine};

pub struct HttpTransformer {
    user_addr: SocketAddr,
    /// 请求首部
    request_head: Option<RequestHead>,
    _marker: PhantomPinned,
}

/// 请求首部
#[derive(Debug)]
struct RequestHead {
    request_line: RequestLine,
    headers: HashMap<String, String>,
}

impl RequestHead {
    /// 修改请求头
    pub fn change_head(&mut self, k: String, v: String) {
        self.headers.insert(k, v);
    }

    /// 构造请求头
    pub fn build_request_head(&self) -> BytesMut {
        let request_line_byte = self.request_line.to_byte();

        let mut request_head = BytesMut::from_iter(request_line_byte);

        for (key, value) in self.headers.iter() {
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
    pub fn new(user_addr: SocketAddr) -> Self {
        Self {
            user_addr,
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

    /// 解析一条 http 请求
    async fn parse_http_request<'a, R>(&mut self, reader: &'a mut R) -> BytesMut
    where
        R: AsyncReadExt + Unpin,
    {
        let mut buf = BytesMut::with_capacity(4 * 1024);
        loop {
            match reader.read_buf(&mut buf).await {
                Ok(size) => {
                    if size == 0 {
                        println!("✅用户退出");
                        return buf;
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
                    };
                    break;
                }
                Err(e) => {
                    eprintln!("parse_http_request 传输读取失败 {e:?}");
                    return buf;
                }
            };
        }

        let request_head = self.request_head.as_mut().unwrap();
        let header_bytes = Self::transformer(request_head, self.user_addr);

        let mut res = BytesMut::new();
        res.extend_from_slice(&header_bytes);
        res.extend_from_slice(&buf);

        self.request_head = None;
        res
    }

    /// 修改请求头
    fn transformer(request_head: &mut RequestHead, user_addr: SocketAddr) -> BytesMut {
        request_head.change_head("Host".to_string(), "8.0.0.1".to_string());
        request_head.change_head("X-Forwarded-For".to_string(), user_addr.ip().to_string());
        // request_head.change_head("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".to_string());
        request_head.build_request_head()
    }

    /// 拷贝数据
    pub async fn copy<'a, R, W>(&mut self, reader: &'a mut R, writer: &'a mut W) -> io::Result<u64>
    where
        R: AsyncReadExt + Unpin,
        W: AsyncWriteExt + Unpin,
    {
        let parsed_byte = self.parse_http_request(reader).await;
        if parsed_byte.is_empty() {
            return Ok(0);
        }

        let write_res = writer.write_all(&parsed_byte).await;
        if write_res.is_err() {
            println!("❌写入出错{write_res:?}");
            return Ok(0);
        }
        let _ = writer.flush().await;

        Ok(parsed_byte.len().try_into().unwrap())
    }
}
