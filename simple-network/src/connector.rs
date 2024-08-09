
use tokio::net::{TcpStream, UnixStream};
use async_trait::async_trait;
use tokio::sync::mpsc;
use crate::channel::{Channel, Event};
use crate::reader::{TcpReader, UnixReader};
use crate::writer::{TcpWriter, UnixWriter};

#[async_trait]
pub trait StreamConnector: Send + Sync {
    async fn connect(&mut self, event_sender: mpsc::Sender<Event>) -> anyhow::Result<Channel>;
}



pub struct UnixConnector {
    server_name: String,
}



pub struct TcpConnector {
    server_name: String,
}

impl UnixConnector {
    pub fn new(server_name: &str) -> Self {
        Self {
            server_name: server_name.to_string(),
        }
    }
}

impl TcpConnector {
    pub fn new(server_name: &str) -> Self {
        Self {
            server_name: server_name.to_string(),
        }
    }
}



#[async_trait]
impl StreamConnector for UnixConnector {
    async fn connect(&mut self, event_sender: mpsc::Sender<Event>) -> anyhow::Result<Channel> {
        let stream = UnixStream::connect(&self.server_name).await?;
        let (reader, writer) = stream.into_split();
        Ok(Channel::new(Box::new(UnixReader::new(reader)), Box::new(UnixWriter::new(writer)), event_sender))
    }

}



#[async_trait]
impl StreamConnector for TcpConnector {
    async fn connect(&mut self, event_sender: mpsc::Sender<Event>) -> anyhow::Result<Channel> {
        let stream = TcpStream::connect(&self.server_name).await?;
        let (reader, writer) = stream.into_split();
        Ok(Channel::new(Box::new(TcpReader::new(reader)), Box::new(TcpWriter::new(writer)), event_sender))
    }
}