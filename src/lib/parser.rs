use std::collections::HashMap;

use nom::{
    bytes::streaming::{tag, take_until},
    sequence::{terminated, tuple},
    IResult, Parser,
};

/**
 * 解析请求首部的请求行
 */
#[derive(Debug)]
pub struct RequestLine {
    pub method: String,
    pub path: String,
    pub protocol: String,
}

impl RequestLine {
    /// 转换成字节
    pub fn to_byte(&self) -> Vec<u8> {
        format!("{} {} {}\r\n", self.method, self.path, self.protocol).into_bytes()
    }
}

pub type Headers = HashMap<String, String>;

pub type RequestHeader = (String, String);

/// 解析请求首部的请求行
pub fn parser_request_line(input: &[u8]) -> IResult<&[u8], RequestLine> {
    let (input, (method, _sp, path, _sp2, protocol)) = tuple((
        take_until(" "),
        tag(" "),
        take_until(" "),
        tag(" "),
        terminated(take_until("\r\n"), tag("\r\n")),
    ))
    .parse(input)?;

    Ok((
        input,
        RequestLine {
            method: String::from_utf8(method.to_vec()).unwrap_or_default(),
            path: String::from_utf8(path.to_vec()).unwrap_or_default(),
            protocol: String::from_utf8(protocol.to_vec()).unwrap_or_default(),
        },
    ))
}

/// 解析请求首部的请求头
pub fn parser_request_header(input: &[u8]) -> IResult<&[u8], RequestHeader> {
    let (input, (key, _colon, value)) = tuple((
        take_until(":"),
        tag(": "),
        terminated(take_until("\r\n"), tag("\r\n")),
    ))
    .parse(input)?;

    Ok((
        input,
        (
            String::from_utf8(key.to_vec()).unwrap_or_default(),
            String::from_utf8(value.to_vec()).unwrap_or_default(),
        ),
    ))
}

/// 解析请求首部，一次性解析完全头部
pub fn parser_request_head_all(input: &[u8]) -> IResult<&[u8], (RequestLine, Headers)> {
    let (input, (request_line, row_headers, _end_lines)) =
        tuple((parser_request_line, take_until("\r\n\r\n"), tag("\r\n\r\n"))).parse(input)?;

    // 解析请求头
    let mut headers = HashMap::<String, String>::new();
    let mut row_headers = row_headers.to_owned();
    row_headers.extend_from_slice(b"\r\n");
    loop {
        // 请求头解析完毕
        if row_headers.is_empty() {
            break;
        }
        match parser_request_header(&row_headers) {
            Ok((rest, request_header)) => {
                headers.insert(request_header.0, request_header.1);

                row_headers = rest.to_owned();
            }
            Err(_) => {
                break;
            }
        }
    }

    Ok((input, (request_line, headers)))
}

#[cfg(test)]
mod test {
    use nom::{
        bytes::streaming::{tag, take_until},
        error,
        sequence::{terminated, tuple},
        IResult, Parser,
    };

    use super::*;

    #[test]
    fn request_line_by_str() {
        let row = "Get /index.html Http/1.1\r\n";

        fn parser(input: &str) -> IResult<&str, RequestLine> {
            let (input, (method, _sp, path, _sp2, protocol)) = tuple((
                take_until::<&str, &str, error::Error<&str>>(" "),
                tag(" "),
                take_until(" "),
                tag(" "),
                terminated(take_until("\r\n"), tag("\r\n")),
            ))
            .parse(input)
            .unwrap();

            Ok((
                input,
                RequestLine {
                    method: method.to_string(),
                    path: path.to_string(),
                    protocol: protocol.to_string(),
                },
            ))
        }
        let a = parser(row).unwrap();
        println!("{:?}", a.1);
    }

    #[test]
    fn parse_request_line_by_steam() {
        let row = b"Get /index.html Http/1.1\r\n";
        let res = parser_request_line(row);
        println!("{:?}", res);
    }

    #[test]
    fn test_parse_request_header() {
        let row = b"Host: www.baidu.com\r\n";
        let (_input, (key, value)) = parser_request_header(row).unwrap();
        let co = key == "Host" && value == "www.baidu.com";
        assert!(
            key == "Host" && value == "www.baidu.com",
            "{key}--{value}--{co}"
        );
    }

    #[test]
    fn test_parse_request_head() {
        let row = b"Get /index.html?a=1 Http/1.1\r\nHost: www.baidu.com\r\nContent-Type: text/html;charset=utf-8\r\nContent-Length: 100\r\n\r\n";
        let (input, request_line) = parser_request_line(row).unwrap();
        assert_eq!(request_line.method, "Get");
        assert_eq!(request_line.path, "/index.html?a=1");
        assert_eq!(request_line.protocol, "Http/1.1");

        let (input, header) = parser_request_header(input).unwrap();
        assert_eq!(header, ("Host".to_owned(), "www.baidu.com".to_owned()));
        let (input, header) = parser_request_header(input).unwrap();
        assert_eq!(
            header,
            (
                "Content-Type".to_owned(),
                "text/html;charset=utf-8".to_owned()
            ),
            "测试出错：{header:?}"
        );
        let (input, header) = parser_request_header(input).unwrap();
        assert_eq!(
            header,
            ("Content-Length".to_owned(), "100".to_owned()),
            "Content-Length测试出错：{header:?}"
        );
        assert_eq!(input, b"\r\n", "结尾测试出错：{input:?}");
    }

    #[test]
    /// 测试解析请求头
    fn parse_head() {
        let row = b"Get /index.html?a=1 Http/1.1\r\nHost: www.baidu.com\r\nContent-Type: text/html;charset=utf-8\r\nContent-Length: 100\r\n\r\n";

        let (input, (request_line, headers)) = parser_request_head_all(row).unwrap();

        assert!(input.is_empty(), "剩余内容：{input:?}");
    }

    #[test]
    /// 测试解析请求头，不完整
    fn parse_head_no_complete() {
        let row = b"Get /index.html?a=1 Http/1.1\r\nHost: www.baidu.com\r\nContent-Type: text/html;charset=utf-8\r\nContent-Length: ";

        let res = parser_request_head_all(row);

        eprintln!("✅{res:?}");
        assert!(res.is_err());
        // println!("{:?}", request_line);
        // println!("{:?}", headers);
    }
}
