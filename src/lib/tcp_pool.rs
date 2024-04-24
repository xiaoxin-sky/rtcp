use deadpool::managed;
use tokio::{
    net::{TcpListener, TcpSocket, TcpStream},
    sync::Mutex,
};

pub struct TcpPoolManager {
    name: String,
    host: String,
    port: u16,
}

#[derive(Debug)]
pub enum Error {
    Fail,
}


impl TcpPoolManager {
    pub fn new(name: String, host: String, port: u16) -> Self {
        TcpPoolManager { name, host, port }
    }
}

impl managed::Manager for TcpPoolManager {
    type Type = TcpStream;

    type Error = Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let tcp_socket = TcpSocket::new_v4().unwrap();
        let addr = format!("{}:{}", self.host, self.port).parse().unwrap();
        let stream = tcp_socket.connect(addr).await.unwrap();
        Ok(stream)
    }

    async fn recycle(
        &self,
        obj: &mut Self::Type,
        metrics: &managed::Metrics,
    ) -> managed::RecycleResult<Self::Error> {
        todo!()
    }
}

pub type Pool = managed::Pool<TcpPoolManager>;

#[cfg(test)]
mod tcp_poll_test {
    // use deadpool::managed::Pool;

    use super::{Pool, TcpPoolManager};

    #[tokio::test]
    async fn test_tcp_pool() {
        let mgr = TcpPoolManager::new("test".to_string(), "127.0.0.1".to_string(), 8081);
        let poll_builder = Pool::builder(mgr);
        let poll = poll_builder.build().unwrap();
        let a = poll.get().await.unwrap();

        

    }
}
