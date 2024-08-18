use std::mem;
use std::os::raw::c_void;
use anyhow::{Result};
use bytes::Bytes;
use futures::future::BoxFuture;
use prost::Message;
use crate::{Packet, StreamServer};


const PARSE_REQ_ERROR : u32 = 1;

pub struct RpcServer {
    server: StreamServer,
}

impl RpcServer {
    pub fn new(server: StreamServer) -> Self {
        RpcServer {
            server,
        }
    }

    pub async fn send_push<M: Message>(&self, channel_id : u64, cmd: u32, seq: u32, ec: u32, msg: M) -> Result<()> {
        let body = msg.encode_to_vec();
        let pack = Packet::new_rsp(cmd, seq, ec, Bytes::from(body));
        self.server.send_push(channel_id, &pack).await
    }

    pub async fn register_request_callback<REQ: Message + Default+ 'static, RSP: Message + Default+ 'static>(&self, cmd: u32, callback: impl Fn(u64, REQ) -> BoxFuture<'static, Result<RSP, u32>> + Send + Sync + 'static) {
        let cb: Box<Box<dyn Fn(u64, REQ) -> BoxFuture<'static,  Result<RSP, u32>> + Send + Sync + 'static>>  = Box::new(Box::new(callback));
        let cookie = Box::into_raw(cb) as *mut c_void as u64;
        self.server.register_request_callback(cmd,  move|channel_id, pack, cookie|  {
            Box::pin(async move  {
                unsafe {
                    let closure: &Box<dyn Fn(u64, REQ) -> BoxFuture<'static,  Result<RSP, u32>> + Send + Sync + 'static> = mem::transmute(cookie as * mut  c_void);
                    let request = REQ::decode(pack.body());
                    if let Ok(request) = request {
                        let rsp_result = closure(channel_id, request).await;
                        if let Ok(rsp_msg) = rsp_result {
                            let body = rsp_msg.encode_to_vec();
                            Packet::new_rsp(cmd, pack.seq(), 0, Bytes::from(body))
                        } else {
                            Packet::new_rsp(cmd, pack.seq(), rsp_result.err().unwrap(), Bytes::new())
                        }
                    } else {
                        Packet::new_rsp(cmd, pack.seq(), PARSE_REQ_ERROR, Bytes::new())
                    }
                }
            })
        }, cookie).await;
    }

    pub async  fn unregister_request_callback<REQ: Message + Default+ 'static, RSP: Message + Default+ 'static>(&self, cmd: u32) {
        if let Some((_, cookie)) = self.server.unregister_request_callback(cmd).await {
            unsafe {
                let _ = Box::from_raw(cookie as *mut c_void as *mut Box<Box<dyn Fn(u64, REQ) -> BoxFuture<'static,  Result<RSP, u32>> + Send + Sync + 'static>>);
            }
        }
    }

    pub async fn set_channel_closed_callback(&self, cb: impl Fn(u64) -> BoxFuture<'static, ()> + Send + Sync + 'static) {
        self.server.set_channel_closed_callback(cb).await;
    }
}