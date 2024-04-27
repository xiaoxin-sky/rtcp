use deadpool::managed::{self};
use tokio::
    net::{TcpSocket, TcpStream}
;

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

#[derive(Debug)]
pub struct TcpStreamData {
    pub stream: TcpStream,
    pub id: uuid::Uuid,
    pub disconnect: bool,
}

impl TcpStreamData {
    pub fn new(stream: TcpStream) -> Self {
        TcpStreamData {
            stream,
            id: uuid::Uuid::new_v4(),
            disconnect: false,
        }
    }
}

impl managed::Manager for TcpPoolManager {
    type Type = TcpStreamData;

    type Error = Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let tcp_socket = TcpSocket::new_v4().unwrap();
        let addr = format!("{}:{}", self.host, self.port).parse().unwrap();
        let stream = tcp_socket.connect(addr).await.unwrap();
        println!(" 🚀 创建 steam 成功");
        Ok(TcpStreamData::new(stream))
    }

    async fn recycle(
        &self,
        obj: &mut Self::Type,
        metrics: &managed::Metrics,
    ) -> managed::RecycleResult<Self::Error> {
        // println!(" 🚀 回收 steam 成功");
        // let mut buf = BytesMut::with_capacity(1);
        // match obj.stream.peek(&mut buf).await {
        //     Ok(size) => if size == 0 {
        //         // 后端断开了，需要从池中销毁掉这个无效的 obj
        //         Object::take(obj);
        //         return  Ok(());
        //         // return Err(deadpool::managed::RecycleError::Backend(Error::Fail));
        //     },
        //     Err(e) => {}
        // };

        Ok(())
    }
}

pub type Pool = managed::Pool<TcpPoolManager>;

#[cfg(test)]
mod tcp_poll_test {

    use deadpool::unmanaged;

    use super::{Pool, TcpPoolManager, TcpStreamData};

    #[tokio::test]
    async fn test_tcp_pool() {
        let mgr = TcpPoolManager::new("test".to_string(), "127.0.0.1".to_string(), 8081);
        let poll_builder = Pool::builder(mgr);
        let poll = poll_builder.build().unwrap();
        let a = poll.get().await.unwrap();
        let b = poll.get().await.unwrap();
        println!("🚀id_a:{:?}, id_b:{:?}", a.id, b.id);
        drop(a);
        let a = poll.get().await.unwrap();
        println!("🚀{:?}", a.id);
    }

    async fn test_tcp_pool_2() {
        let a: unmanaged::Pool<TcpStreamData> = unmanaged::Pool::new(1000);
        // a.add()
    }
}
