use anyhow::{Result};
use bytes::Bytes;
use chrono::Local;
use crate::base::Packet;
use crate::ipc::{IpcServer};
use prost::Message;

pub struct RpcServer {
    ipc_server: IpcServer,
}

impl RpcServer {
    pub fn new(server_name: &str) -> Self {
        RpcServer {
            ipc_server : IpcServer::new(server_name),
        }
    }


    pub async fn send_push_to_all<M: Message>(&self, method_id: u32, msg: M) {
        let body = msg.encode_to_vec();
        let pack = Packet::new_push(method_id, Bytes::from(body));
        return self.ipc_server.send_push_to_all(pack).await;
    }

    pub async fn send_push<M: Message>(&self, conn_id : u32, method_id: u32, msg: M) -> Result<()> {
        let body = msg.encode_to_vec();
        let pack = Packet::new_push(method_id, Bytes::from(body));
        return self.ipc_server.send_rsp_or_push(conn_id, pack).await;
    }

    pub async fn send_rsp<M: Message>(&self, conn_id : u32, method_id: u32, seq: u32, ec: u32, msg: M) -> Result<()> {
        let body = msg.encode_to_vec();
        let pack = Packet::new_rsp(method_id, seq, ec, Bytes::from(body));
        return self.ipc_server.send_rsp_or_push(conn_id, pack).await;
    }

    pub async fn wait_request<M: Message + Default>(&self, method_id: u32) -> (u32, u32, M) {
        loop {
            let (conn_id, pack) = self.ipc_server.wait_request(method_id).await;
            let result = M::decode(pack.body());
            if result.is_err() {
                continue;
            }

            return (conn_id, pack.seq(), result.unwrap());
        }
    }
}