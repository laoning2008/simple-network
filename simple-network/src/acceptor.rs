use tokio::net::{UnixListener, TcpListener};
use async_trait::async_trait;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use crate::channel::{Channel, Event};
use crate::reader::{TcpReader, UnixReader};
use crate::writer::{TcpWriter, UnixWriter};


#[async_trait]
pub trait StreamAcceptor: Send + Sync {
    async fn accept(&self, event_sender: mpsc::Sender<Event>) -> anyhow::Result<Channel>;
}

pub struct UnixAcceptor {
    listener: UnixListener,
}

pub struct TcpAcceptor {
    listener: TcpListener,
}

impl UnixAcceptor {
    pub fn new(server_name: String) -> Self {
        Self {
            listener: UnixListener::bind(&server_name).unwrap(),
        }
    }
}



impl TcpAcceptor {
    pub async fn new(server_name: String) -> Self {
        Self {
            listener: TcpListener::bind(&server_name).await.unwrap(),
        }
    }
}



#[async_trait]
impl StreamAcceptor for UnixAcceptor {
    async fn accept(&self, event_sender: mpsc::Sender<Event>) -> anyhow::Result<Channel> {
        let (stream, _) = self.listener.accept().await?;
        let (reader, writer) = stream.into_split();
        Ok(Channel::new(Box::new(UnixReader::new(reader)), Box::new(UnixWriter::new(writer)), event_sender))
    }
}



#[async_trait]
impl StreamAcceptor for TcpAcceptor {
    async fn accept(&self, event_sender: mpsc::Sender<Event>) -> anyhow::Result<Channel> {
        let (stream, _) = self.listener.accept().await?;
        let (reader, writer) = stream.into_split();
        Ok(Channel::new(Box::new(TcpReader::new(reader)), Box::new(TcpWriter::new(writer)), event_sender))
    }
}