use nom::{
    bytes::streaming::{tag, take_until},
    sequence::{terminated, tuple},
    IResult, Parser,
};

fn main() {}

#[derive(Debug)]
struct RequestLine {
    method: String,
    path: String,
    protocol: String,
}

/// 解析请求首部的请求行
fn parser_request_line(input: &[u8]) -> IResult<&[u8], RequestLine> {
    let (input, (method, sp, path, sp2, protocol)) = tuple((
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
fn parser_request_header(input: &[u8]) -> IResult<&[u8], (String, String)> {
    let (input, (key, a, value)) =
        tuple((take_until(":"), tag(": "), terminated(take_until("\r\n"), tag("\r\n")))).parse(input)?;

    Ok((
        input,
        (
            String::from_utf8(key.to_vec()).unwrap_or_default(),
            String::from_utf8(value.to_vec()).unwrap_or_default(),
        ),
    ))
}

#[cfg(test)]
mod test {
    use nom::{
        bytes::streaming::{tag, take_until},
        error::{self, Error},
        sequence::{terminated, tuple},
        IResult, Parser,
    };

    use super::*;

    #[test]
    fn request_line_by_str() {
        let row = "Get /index.html Http/1.1\r\n";

        fn parser(input: &str) -> IResult<&str, RequestLine> {
            let (input, (method, sp, path, sp2, protocol)) = tuple((
                take_until::<&str, &str, error::Error<&str>>(" "),
                tag(" "),
                take_until(" "),
                tag(" "),
                terminated(take_until("\r\n"), tag("\r\n")),
            ))
            .parse(input)
            .unwrap();

            return Ok((
                input,
                RequestLine {
                    method: method.to_string(),
                    path: path.to_string(),
                    protocol: protocol.to_string(),
                },
            ));
        }
        let a = parser(&row).unwrap();
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
        assert_eq!(header, ("Content-Type".to_owned(), "text/html;charset=utf-8".to_owned()),"测试出错：{header:?}");
        let (input, header) = parser_request_header(input).unwrap();
        assert_eq!(header, ("Content-Length".to_owned(), "100".to_owned()),"Content-Length测试出错：{header:?}");
        assert_eq!(input, b"\r\n","结尾测试出错：{input:?}");
    }
}
