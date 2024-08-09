use tokio::net::tcp;
use tokio::net::unix;
use std::io;
use async_trait::async_trait;
use tokio::io::AsyncWriteExt;

#[async_trait]
pub trait StreamWriter: Send + Sync {
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;
}

pub struct UnixWriter {
    writer: unix::OwnedWriteHalf,
}


pub struct TcpWriter {
    writer: tcp::OwnedWriteHalf,
}

impl UnixWriter {
    pub fn new(writer: unix::OwnedWriteHalf) -> Self {
        Self {
            writer
        }
    }
}

impl TcpWriter {
    pub fn new(writer: tcp::OwnedWriteHalf) -> Self {
        Self {
            writer
        }
    }
}

#[async_trait]
impl StreamWriter for UnixWriter {
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.writer.write_all(buf).await
    }
}

#[async_trait]
impl StreamWriter for TcpWriter {
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.writer.write_all(buf).await
    }
}
