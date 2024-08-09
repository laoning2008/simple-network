
use tokio::net::tcp;
use tokio::net::unix;
use std::io;
use async_trait::async_trait;
use bytes::BytesMut;
use tokio::io::AsyncReadExt;

#[async_trait]
pub trait StreamReader: Send + Sync {
    async fn read_some(&mut self, buf: &mut BytesMut) -> io::Result<usize>;
}

pub struct UnixReader {
    reader: unix::OwnedReadHalf,
}

pub struct TcpReader {
    reader: tcp::OwnedReadHalf,
}

impl UnixReader {
    pub fn new(reader: unix::OwnedReadHalf) -> Self {
        Self {
            reader
        }
    }
}



impl TcpReader {
    pub fn new(reader: tcp::OwnedReadHalf) -> Self {
        Self {
            reader
        }
    }
}



#[async_trait]
impl StreamReader for UnixReader {
    async fn read_some(&mut self, buf: &mut BytesMut) -> io::Result<usize> {
        self.reader.read_buf(buf).await
    }
}



#[async_trait]
impl StreamReader for TcpReader {
    async fn read_some(&mut self, buf: &mut BytesMut) -> io::Result<usize> {
        self.reader.read_buf(buf).await
    }
}
