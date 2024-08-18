mod packet;
mod connector;
mod reader;
mod writer;
mod channel;
mod client;
mod server;
mod acceptor;
mod factory;
mod rpc_server;
mod rpc_client;

use futures::future::BoxFuture;
pub use packet::Packet;
pub use client::StreamClient;
pub use server::StreamServer;
pub use rpc_client::RpcClient;
pub use rpc_server::RpcServer;


pub type ChannelClosedCallback = Box<dyn Fn(u64) -> BoxFuture<'static, ()> + Send + Sync + 'static>;
pub type RequestReceivedCallback = Box<dyn Fn(u64, Packet, u64) -> BoxFuture<'static, Packet> + Send + Sync + 'static>;
pub type PushReceivedCallback = Box<dyn Fn(u64, Packet, u64) -> BoxFuture<'static, ()> + Send + Sync + 'static>;