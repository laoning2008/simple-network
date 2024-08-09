use bytes::Bytes;
use simple_network::{Packet, StreamClient, StreamServer};

#[tokio::main]
async fn main() {
    let server_name = "/tmp/simple";//"localhost:8080";
    let mut server = StreamServer::new(server_name);

    loop {
        let (chan_id, req) = server.wait_request(1).await;
        println!("receive req,  cmd = {}, body = {}", req.cmd(), String::from_utf8_lossy(req.body()));

        let rsp = Packet::new_rsp(req.cmd(), req.seq(), 0, Bytes::from("world"));
        if server.send_rsp_or_push(chan_id, &rsp).await.is_ok() {
            println!("send rsp success ");
        } else {
            println!("send rsp failed ");
        }
    }
}