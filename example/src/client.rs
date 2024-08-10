use std::sync::Arc;
use std::time::Duration;
use bytes::Bytes;
use tokio::time;
use simple_network::{Packet, StreamClient};

#[tokio::main]
async fn main() {
    let server_name = "/tmp/simple";//"localhost:8080";
    let client = Arc::new(StreamClient::new(server_name));

    let client_reconnect = client.clone();
    tokio::spawn(async move {
        loop {
            client_reconnect.wait_closed_event().await;
            if client_reconnect.connect().await.is_err() {
                println!("connect failed");
            }
        }
    });

    if client.connect().await.is_err() {
        println!("connect failed");
    }

    while let Err(_) = client.connect().await {
        println!("connect failed");
        time::sleep(Duration::from_millis(1000)).await;
    }

    let req = Packet::new_req(1, Bytes::from("hello"));
    loop {
         if let Ok(rsp) = client.send_request(&req, 1000).await {
             println!("receive rsp,  cmd = {}, body = {}", rsp.cmd(), String::from_utf8_lossy(rsp.body()));
         } else {
             println!("failed to receive rsp");
         }

        time::sleep(Duration::from_millis(1000)).await;
    }
}