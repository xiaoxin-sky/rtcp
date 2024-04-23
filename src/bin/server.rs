use std::{collections::HashMap, io::Error, marker::PhantomPinned, sync::Arc};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use headers::{Header, HeaderMapExt};
use rtcp::{
    parser::{parser_request_head_all, RequestLine},
    protocol::{RTCPMessage, RTCPType},
    manage::RTCPManager,
};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    join,
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
};

/// 创建通道服务器
async fn create_connect_channel() -> io::Result<TcpStream> {
    let tcp_listener = TcpListener::bind("0.0.0.0:5541").await?;

    loop {
        match tcp_listener.accept().await {
            Ok(mut stream) => {
                println!("通道服务器({:?})连接成功", stream.1);

                let msg = RTCPMessage::new(RTCPType::Initialize, BytesMut::new());
                stream.0.write_all(msg.serialize().as_ref()).await?;

                return Ok(stream.0);
            }
            Err(e) => {
                println!("❌通道服务器连接失败{:?}", e);
                continue;
            }
        };
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let rtcp_manage = Arc::new(Mutex::new(RTCPManager::new()));

    let client_server = tokio::spawn(async move {
        let client_tcp = create_connect_channel().await?;
        let client_tcp = Arc::new(Mutex::new(client_tcp));
        let client_tcp_clone = client_tcp.clone();
        let rtcp_message = Mutex::<Option<RTCPMessage>>::new(None);
        let buf = Arc::new(Mutex::new(BytesMut::new()));

        loop {
            let buf_mut = buf.lock().await;
            let mut buf = (*buf_mut).clone();
            drop(buf_mut);

            let read_res = client_tcp_clone.lock().await.read_buf(&mut buf).await;

            if read_res.is_err() {
                println!("❌通道服务器读取失败{:?}", read_res);
                break;
            }

            let read_res = read_res.unwrap();

            if rtcp_message.lock().await.is_none() {
                match RTCPMessage::deserialize(buf) {
                    Ok(res) => {
                        let mut rtcp_message_mut = rtcp_message.lock().await;
                        *rtcp_message_mut = Some(res);
                    }
                    Err(e) => {
                        println!("序列化失败,继续读取{:?}", e);
                        continue;
                    }
                };
            }

            let mut rtcp_message_mutex = rtcp_message.lock().await;

            /// 这里其实可以不需要判断
            if rtcp_message_mutex.is_none() {
                continue;
            }

            let rtcp_message = rtcp_message_mutex.as_mut().unwrap();
            match rtcp_message.message_type {
                // client 、 server  都需要实现
                RTCPType::Transformation(data_len) => {
                    if rtcp_message.data.len() < data_len {
                        continue;
                    }
                    // let mut buf = BytesMut::with_capacity(data_len);
                    // 获取 date_len 的数据给 用户 client tcp
                    //
                    println!("TODO:aaa");
                    // 发送完毕之后吃掉发送的长度，然后再继续读取
                    rtcp_message.data.advance(data_len);
                    *rtcp_message_mutex = None;
                    // 读取下一条数据
                    continue;
                }
                RTCPType::CloseConnection => {
                    break;
                }
                // 其他 arm 需要 client 实现
                _ => {
                    println!("收到事件{}", rtcp_message.message_type);
                    // 读取下一条数据
                    continue;
                }
            }
        }

        Ok::<_, io::Error>(())
    });


    let user_server = tokio::spawn(async move {
        let addr = "0.0.0.0:9931";
        let tcp_listener = TcpListener::bind(addr).await?;

        loop {
            // let client_tcp = client_tcp_clone1.clone();
            let (tcp_stream, socket_addr) = tcp_listener.accept().await?;
            tokio::spawn(async move {
                println!("{socket_addr}");

                // 解析并转换用户请求
                let mut http_transformer = HttpTransformer::new(tcp_stream);

                let body_buf = http_transformer.run().await?;

                let head_buf = http_transformer.request_head.unwrap().build_request_head();

                // 收到新连接事件，构造消息
                let msg = RTCPMessage::new(RTCPType::NewConnection, head_buf);

                // 发送创建连接事件
                // client_tcp.lock().await.write_all(&msg.serialize()).await?;

                // // 转发数据
                // let msg = RTCPMessage {
                //     message_type: RTCPType::Transformation(body_buf.len()),
                //     connect_id: msg.connect_id.clone(),
                //     data: body_buf,
                // };

                // // 发送data
                // client_tcp.lock().await.write_all(&msg.serialize()).await?;

                // // 刷新缓冲
                // client_tcp.lock().await.flush().await?;

                Ok::<_, io::Error>(())
            });
        }

        Ok::<_, io::Error>(())
    });

    let _res = join!(client_server, user_server);

    Ok(())
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

    pub async fn run(&mut self) -> io::Result<BytesMut> {
        let mut buf = BytesMut::with_capacity(4 * 1024);

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
        Ok(buf)
    }
}
