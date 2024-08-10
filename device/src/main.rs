use std::time::Duration;
use bytes::Bytes;
use tokio::time;
use simple_network::{Packet, StreamClient};

#[tokio::main]
async fn main() {
    let server_name = "localhost:8080";
    let mut client = StreamClient::new(server_name);
    let req = Packet::new_req(1, Bytes::from("hello"));
    loop {
        if let Ok(rsp) = client.send_request_wait_response(&req, 1000).await {
            println!("receive rsp,  cmd = {}, body = {}", rsp.cmd(), String::from_utf8_lossy(rsp.body()));
        } else {
            println!("failed to receive rsp");
        }

        time::sleep(Duration::from_millis(1000)).await;
    }
}