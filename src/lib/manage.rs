use std::{collections::HashMap, io, net::TcpStream};

use crate::protocol::ConnectId;

/// 连接对象
#[derive(Debug)]
pub struct RTCPConnection {
    pub income_tcp: TcpStream,
    pub outcome_tcp: TcpStream,
    pub connect_id: ConnectId,
}

/// 连接储存
pub type RTCPConnectionMap = HashMap<ConnectId, RTCPConnection>;

/// 连接管理
#[derive(Debug)]
pub struct RTCPManager {
    inner: RTCPConnectionMap,
}

impl RTCPManager {
    pub fn new() -> Self {
        RTCPManager {
            inner: HashMap::new(),
        }
    }

    /// 检查connect_id 是否有效
    fn check_connect_id(&self, connect_id: &ConnectId) -> io::Result<bool> {
        if connect_id.is_none() {
            return Err(io::Error::new(io::ErrorKind::Other, "connect_id is none"));
        }
        Ok(true)
    }

    /// 添加连接
    pub fn add_connection(&mut self, connection: RTCPConnection) -> io::Result<()> {
        self.check_connect_id(&connection.connect_id)?;
        self.inner.insert(connection.connect_id.clone(), connection);
        Ok(())
    }

    /// 移除连接
    pub fn remove_connection(&mut self, connect_id: ConnectId) -> io::Result<()> {
        self.check_connect_id(&connect_id)?;

        self.inner.remove(&connect_id);
        Ok(())
    }

    /// 获取连接
    pub fn get_connection(&self, connect_id: &ConnectId) -> io::Result<Option<&RTCPConnection>> {
        self.check_connect_id(connect_id)?;

        Ok(self.inner.get(connect_id))
    }
}
