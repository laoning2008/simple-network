use std::mem;
use std::os::raw::c_void;
use anyhow::{Result};
use bytes::Bytes;
use futures::future::BoxFuture;
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

    pub async fn run(&self) {
        self.client.run().await
    }

    pub async fn connect(&self) -> anyhow::Result<()> {
        self.client.connect().await
    }

    pub async fn send_req<REQ: Message, RSP: Message + Default>(&self, method_id: u32, req: REQ, timeout_seconds: u64) -> Result<RSP> {
        let body = req.encode_to_vec();
        let pack = Packet::new_req(method_id, Bytes::from(body));
        let rsp_pack = self.client.send_request(&pack, timeout_seconds).await?;
        Ok(RSP::decode(rsp_pack.body())?)
    }

    pub fn register_push_callback<M: Message + Default+ 'static>(&self, cmd: u32, callback: impl Fn(u64, M) -> BoxFuture<'static, ()> + Send + Sync + 'static) {
        let cb: Box<Box<dyn Fn(u64, M) -> BoxFuture<'static,  ()> + Send + Sync + 'static>>  = Box::new(Box::new(callback));
        let cookie = Box::into_raw(cb) as *mut c_void as u64;
        self.client.register_push_callback(cmd,  move|channel_id, pack, cookie|  {
            Box::pin(async move  {
                unsafe {
                    let closure: &Box<dyn Fn(u64, M) -> BoxFuture<'static, ()> + Send + Sync + 'static> = mem::transmute(cookie as * mut  c_void);
                    let request = M::decode(pack.body());
                    if let Ok(request) = request {
                        closure(channel_id, request).await;
                    }
                }
            })
        }, cookie);
    }

    pub  fn unregister_push_callback<M: Message + Default+ 'static>(&self, cmd: u32) {
        if let Some((_, cookie)) = self.client.unregister_push_callback(cmd) {
            unsafe {
                let _ = Box::from_raw(cookie as *mut c_void as *mut Box<Box<dyn Fn(u64, M) -> BoxFuture<'static, ()> + Send + Sync + 'static>>);
            }
        }
    }

    pub fn set_channel_closed_callback(&self, cb: impl Fn(u64) -> BoxFuture<'static, ()> + Send + Sync + 'static) {
        self.client.set_channel_closed_callback(cb);
    }
}