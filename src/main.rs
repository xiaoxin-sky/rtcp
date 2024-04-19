use nom::{
    bytes::complete::{tag, take_until, take_while1}, error::Error, sequence::{preceded, Tuple}, AsChar
};

fn main() {}

#[test]
fn test() {
    // combine http and version to extract the version string
    // preceded will return the result of the second parser
    // if both succeed

    // combine all previous parsers in one function
    fn request_line(i: &[u8]) {
        let method = take_while1(AsChar::is_alpha);
        let space = take_while1(|c| c == b' ');
        let space1 = take_while1(|c| c == b' ');
        let url = take_while1(|c| c != b' ');
        let is_version = |c| c >= b'0' && c <= b'9' || c == b'.';
        let http = take_until::<&str, &[u8], Error<&[u8]>>("HTTP/");
        let version = take_while1(is_version);
        let line_ending = tag("\r\n");
        let http_version = preceded(http, version);
        // Tuples of parsers are a parser themselves,
        // parsing with each of them sequentially and returning a tuple of their results.
        // Unlike most other parsers, parser tuples are not `FnMut`, they must be wrapped
        // in the `parse` function to be able to be used in the same way as the others.
        let a = (method, space, url, space1, http_version, line_ending).parse(i);
        println!("{a:?}");


        // Ok((input, (method, url, version)))
    }

    request_line(b"Get /index.html Http/1.1\r\n");
}
