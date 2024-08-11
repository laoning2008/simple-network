use anyhow::{Result};
use bytes::Bytes;
use prost::Message;
use crate::{Packet, StreamClient};

pub struct RpcClient {
    client: StreamClient,
}

impl RpcClient {
    pub fn new(server_name: &str) -> Self {
        RpcClient {
            client : StreamClient::new(server_name),
        }
    }

    pub async fn send_req<REQ: Message, RSP: Message + Default>(&self, method_id: u32, req: REQ, timeout_seconds: u64) -> Result<RSP> {
        let body = req.encode_to_vec();
        let pack = Packet::new_req(method_id, Bytes::from(body));
        let rsp_pack = self.client.send_request(&pack, timeout_seconds).await?;
        Ok(RSP::decode(rsp_pack.body())?)
    }

    pub async fn wait_push<M: Message + Default>(&self, method_id: u32) -> M {
        loop {
            let pack = self.client.wait_push(method_id).await;
            let result = M::decode(pack.body());
            if result.is_err() {
                continue;
            }

            return result.unwrap();
        }
    }

    pub async fn wait_closed_event(&self) {
        self.client.wait_closed_event().await
    }
}