use anyhow::{Result};
use bytes::Bytes;
use prost::Message;
use crate::{Packet, StreamServer};

pub struct RpcServer {
    server: StreamServer,
}

impl RpcServer {
    pub fn new(server: StreamServer) -> Self {
        RpcServer {
            server,
        }
    }

    pub async fn send_push<M: Message>(&self, channel_id : u64, method_id: u32, msg: M) -> Result<()> {
        let body = msg.encode_to_vec();
        let pack = Packet::new_push(method_id, Bytes::from(body));
        self.server.send_push(channel_id, &pack).await
    }

    pub async fn send_response<M: Message>(&self, channel_id : u64, method_id: u32, seq: u32, ec: u32, msg: M) -> Result<()> {
        let body = msg.encode_to_vec();
        let pack = Packet::new_rsp(method_id, seq, ec, Bytes::from(body));
        self.server.send_response(channel_id, &pack).await
    }

    pub async fn wait_request<M: Message + Default>(&self, method_id: u32) -> (u64, u32, M) {
        loop {
            let (channel_id, pack) = self.server.wait_request(method_id).await;
            let result = M::decode(pack.body());
            if result.is_err() {
                continue;
            }

            return (channel_id, pack.seq(), result.unwrap());
        }
    }

    pub async fn wait_channel_closed_event(&self) -> u64 {
        self.server.wait_channel_closed_event().await
    }
}