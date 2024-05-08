use std::{fmt::Display, io};

use bytes::{Buf, Bytes, BytesMut};
use nom::{
    bytes::streaming::{tag, take_until},
    error::Error,
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
    Initialize(u16),
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
        if s.starts_with("initialize") {
            let size_str = &s["initialize:".len()..];
            if let Ok(size) = size_str.parse::<u16>() {
                return Ok(RTCPType::Initialize(size));
            }
        }
        match s {
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
            RTCPType::Initialize(port) => write!(f, "initialize:{port}"),
            RTCPType::NewConnection => write!(f, "new_connection"),
            RTCPType::CloseConnection => write!(f, "close_connection"),
        }
    }
}

/// Represents a single RTCP message, which is used for communication between rtcp client and server.
#[derive(Debug)]
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
            RTCPType::Initialize(_) => None,
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
    /// message_type connect_id\r\n
    /// ```
    pub fn serialize(&self) -> Bytes {
        // Serialize the message type and data into a byte array.

        return Bytes::copy_from_slice(
            format!(
                "{} {}\r\n",
                self.message_type,
                self.connect_id.clone().unwrap_or_default()
            )
            .as_bytes(),
        );
    }

    /// Deserialize the byte array into an RTCPMessage.
    pub fn deserialize(input: &[u8]) -> io::Result<(Self, usize)> {
        // Deserialize the byte array into an RTCPMessage.

        let parse_res = tuple((
            take_until::<&str, &[u8], Error<&[u8]>>(" "),
            tag(" "),
            terminated(take_until("\r\n"), tag("\r\n")),
        ))
        .parse(input);

        if parse_res.is_err() {
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

        let msg_size = input.len() - output.len();
        Ok((
            Self {
                message_type,
                connect_id,
            },
            msg_size,
        ))
    }

    pub fn get_size(&self) -> usize {
        return self.message_type.to_string().as_bytes().len()
            + self.connect_id.clone().unwrap_or_default().as_bytes().len();
    }
}

#[cfg(test)]
mod tests_protocol {
    use super::*;

    #[test]
    fn test_serialize() {
        let message = RTCPMessage::new(RTCPType::Initialize(8830));
        let serialized = message.serialize();
        let b = BytesMut::from("initialize:8830 \r\n");
        assert_eq!(
            serialized, b,
            "检查序列化失败 a：{:?} b: {:?}",
            serialized, b
        );
    }

    #[test]
    fn test_deserialize() {
        let message = RTCPMessage::new(RTCPType::Initialize(8830));
        let serialized = message.serialize();
        let (deserialized, size) = RTCPMessage::deserialize(&serialized).unwrap();
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
        assert_eq!(size, b"initialize:8830 \r\n".len(), "反检查序列化 size 失败");
    }
}
