use bytes::Bytes;
use simple_network::{Packet, StreamServer};

#[tokio::main]
async fn main() {
    let server_name = "localhost:8080";
    let mut server = StreamServer::new(server_name);

    loop {

    }
}