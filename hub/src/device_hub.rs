use tokio::select;
use simple_network::{RpcServer, StreamServer};
use crate::device::DeviceMgr;
use crate::proto::{DeviceOfflineRequest, DeviceOnlineRequest};

const CMD_DEVICE_ONLINE : u32 = 1;
const CMD_DEVICE_OFFLINE : u32 = 2;

pub struct DeviceHub {
    rpc_server: RpcServer,
    device_mgr: DeviceMgr,
}

impl DeviceHub {
    pub fn new(server_name: &str) -> Self {
        Self {
            rpc_server: RpcServer::new(StreamServer::new(server_name)),
            device_mgr: DeviceMgr::new(),
        }
    }

    pub async fn start(&self) {
        loop {
            select! {
                channel_id = self.rpc_server.wait_channel_closed_event() => {
                    self.device_mgr.on_channel_closed(channel_id).await;
                }
                (channel_id, cmd, request) = self.rpc_server.wait_request::<DeviceOnlineRequest>(CMD_DEVICE_ONLINE) => {
                    self.device_mgr.on_device_online(request).await;
                }
                (channel_id, cmd, request) = self.rpc_server.wait_request::<DeviceOfflineRequest>(CMD_DEVICE_OFFLINE) => {
                    self.device_mgr.on_device_offline(request).await;
                }
            }
        }
    }
}