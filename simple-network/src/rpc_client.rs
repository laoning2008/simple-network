use anyhow::{Result};
use bytes::Bytes;
use crate::base::Packet;
use crate::ipc::IpcClient;
use prost::Message;

pub struct RpcClient {
    ipc_client: IpcClient,
}

impl RpcClient {
    pub fn new(server_name: &str) -> Self {
        RpcClient {
            ipc_client : IpcClient::new(server_name),
        }
    }

    pub async fn send_req<REQ: Message, RSP: Message + Default>(&self, method_id: u32, req: REQ, timeout_seconds: u64) -> Result<RSP> {
        let body = req.encode_to_vec();
        let pack = Packet::new_req(method_id, Bytes::from(body));
        let rsp_pack = self.ipc_client.send_req(pack, timeout_seconds).await?;
        let rsp_msg = RSP::decode(rsp_pack.body())?;

        return Ok(rsp_msg);
    }

    pub async fn wait_push<M: Message + Default>(&self, method_id: u32) -> M {
        loop {
            let pack = self.ipc_client.wait_push(method_id).await;
            let result = M::decode(pack.body());
            if result.is_err() {
                continue;
            }

            return result.unwrap();
        }
    }
}