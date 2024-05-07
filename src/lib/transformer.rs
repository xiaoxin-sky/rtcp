use std::{collections::HashMap, marker::PhantomPinned};

use bytes::{Buf, BufMut, BytesMut};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::parser::{parser_request_head_all, RequestLine};

pub struct HttpTransformer {
    /// 请求首部
    request_head: Option<RequestHead>,
    _marker: PhantomPinned,
}

/// 请求首部
struct RequestHead {
    request_line: RequestLine,
    headers: HashMap<String, String>,
}

impl RequestHead {
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

    /// 解析一条 http 请求
    async fn parse_http_request<'a, R>(&mut self, reader: &'a mut R) -> Option<(BytesMut, bool)>
    where
        R: AsyncReadExt + Unpin,
    {
        let mut buf = BytesMut::with_capacity(4 * 1024);
        let mut is_end = false;
        loop {
            match reader.read_buf(&mut buf).await {
                Ok(size) => {
                    if size == 0 {
                        println!("✅退出");
                        is_end = true;
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
                    };
                    break;
                }
                Err(e) => {
                    eprintln!("{e:?}");
                    break;
                }
            };
        }

        let header_bytes = self.request_head.as_ref()?.build_request_head();

        let mut res = BytesMut::new();
        res.extend_from_slice(&header_bytes);
        res.extend_from_slice(&buf);

        Some((res, is_end))
    }

    /// 拷贝数据
    pub async fn copy<'a, R, W>(&mut self, reader: &'a mut R, writer: &'a mut W) -> io::Result<u64>
    where
        R: AsyncReadExt + Unpin,
        W: AsyncWriteExt + Unpin,
    {
        loop {
            let parsed_byte = self.parse_http_request(reader).await;
            if parsed_byte.is_none() {
                return Ok(0);
            }
            let (parsed_byte, is_end) = parsed_byte.unwrap();

            let _ = writer.write_all(&parsed_byte).await;
            let _ = writer.flush().await;

            if is_end {
                return Ok(parsed_byte.len().try_into().unwrap());
            }
        }
    }
}
