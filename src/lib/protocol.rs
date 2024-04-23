use std::{fmt::Display, io};

use bytes::{Buf, BytesMut};
use nom::{
    bytes::streaming::{tag, take_until},
    sequence::{preceded, terminated, tuple},
    Parser,
};
use uuid::Uuid;

/// 传输唯一id
pub type ConnectId = Option<String>;

/// 传输数据长度
pub type TransformationDataLen = usize;

/// Represents the different types of RTCP messages.
#[derive(Debug)]
pub enum RTCPType {
    /// 初始化
    Initialize,
    /// 创建新链接，携带唯一id
    NewConnection,
    /// 互传数据，携带唯一id
    // Transformation(TransformationDataLen),
    /// 关闭
    CloseConnection,
}

impl RTCPType {
    /// Create a new RTCPType from the given string.
    pub fn new_from_str(s: &str) -> io::Result<RTCPType> {
        // if s.starts_with("transformation") {
        //     let size_str = &s["transformation:".len()..];
        //     if let Ok(size) = size_str.parse::<usize>() {
        //         return Ok(RTCPType::Transformation(size));
        //     }
        // }
        match s {
            "initialize" => Ok(RTCPType::Initialize),
            "new_connection" => Ok(RTCPType::NewConnection),
            "close_connection" => Ok(RTCPType::CloseConnection),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid rtcp message type",
            )),
        }
    }
}

impl Display for RTCPType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RTCPType::Initialize => write!(f, "initialize"),
            RTCPType::NewConnection => write!(f, "new_connection"),
            // RTCPType::Transformation(size) => write!(f, "transformation:{size}"),
            RTCPType::CloseConnection => write!(f, "close_connection"),
        }
    }
}

/// Represents a single RTCP message, which is used for communication between rtcp client and server.
pub struct RTCPMessage {
    /// message type
    pub message_type: RTCPType,
    /// 连接id
    pub connect_id: ConnectId,
}

impl RTCPMessage {
    /// Create a new RTCPMessage with the specified type and data.
    pub fn new(message_type: RTCPType) -> Self {
        let connect_id = match message_type {
            RTCPType::Initialize => None,
            RTCPType::NewConnection => Some(Uuid::new_v4().to_string()),
            // other types of message,need return  None， if use other types of message, need use fromExactMessage fn
            _ => None,
        };
        Self {
            message_type,
            connect_id,
        }
    }

    /// Serialize the RTCPMessage into a byte array.
    /// the protocol formate type:
    /// ```
    /// rtcp:message_type connect_id\r\n
    /// bytes
    /// ```
    pub fn serialize(&self) -> BytesMut {
        // Serialize the message type and data into a byte array.

        let mut frame = BytesMut::from(
            format!(
                "rtcp:{} {}\r\n",
                self.message_type,
                self.connect_id.clone().unwrap_or_default()
            )
            .as_bytes(),
        );

        frame
    }

    /// Deserialize the byte array into an RTCPMessage.
    pub fn deserialize(mut input: BytesMut) -> io::Result<Self> {
        // Deserialize the byte array into an RTCPMessage.

        let parse_res = tuple((
            preceded(
                tag::<&str, &[u8], nom::error::Error<&[u8]>>("rtcp:"),
                take_until(" "),
            ),
            tag(" "),
            terminated(take_until("\r\n"), tag("\r\n")),
        ))
        .parse(&input[..]);

        if parse_res.is_err() {
            println!("❌{:?}", parse_res);
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid rtcp message",
            ));
        }

        let (output, (message_type, _, connect_id)) = parse_res.unwrap();

        let message_type =
            RTCPType::new_from_str(String::from_utf8(message_type.to_vec()).unwrap().as_str())?;

        let connect_id = if connect_id.is_empty() {
            None
        } else {
            Some(String::from_utf8(connect_id.to_vec()).unwrap())
        };

        let a = input.len() - output.len();

        input.advance(a);

        Ok(Self {
            message_type,
            connect_id,
        })
    }
}

#[cfg(test)]
mod tests_protocol {
    use super::*;

    #[test]
    fn test_serialize() {
        let message = RTCPMessage::new(RTCPType::Initialize);
        let serialized = message.serialize();
        let b = BytesMut::from("rtcp:initialize \r\n");
        assert_eq!(
            serialized, b,
            "检查序列化失败 a：{:?} b: {:?}",
            serialized, b
        );
    }

    #[test]
    fn test_deserialize() {
        let message = RTCPMessage::new(RTCPType::Initialize);
        let serialized = message.serialize();
        let deserialized = RTCPMessage::deserialize(serialized.clone()).unwrap();
        assert_eq!(
            deserialized.connect_id, message.connect_id,
            "反检查序列化 connect_id 失败 {:?} - {:?}",
            deserialized.connect_id, message.connect_id
        );
        assert_eq!(
            deserialized.message_type.to_string(),
            message.message_type.to_string(),
            "反检查序列化 message_type 失败",
        );
    }
}
