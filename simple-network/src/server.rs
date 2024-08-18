use std::collections::HashMap;
use anyhow::ensure;
use bytes::Bytes;
use futures::future::BoxFuture;
use tokio::select;
use tokio::sync::{mpsc, Mutex};
use crate::{ChannelClosedCallback, Packet, RequestReceivedCallback};
use crate::acceptor::StreamAcceptor;
use crate::channel::{Channel, Event};
use crate::factory::Factory;
use crate::packet::HEART_BEAT_CMD;




pub struct StreamServer {
    channels: Mutex<HashMap<u64, Channel>>,
    channel_closed_callback: Mutex<Option<ChannelClosedCallback>>,
    request_received_callbacks: Mutex<HashMap<u32, (RequestReceivedCallback, u64)>>,
}

impl StreamServer {
    pub fn new() -> Self {
        Self {
            channels: Default::default(),
            channel_closed_callback: Default::default(),
            request_received_callbacks: Default::default(),
        }
    }

    pub async fn run(&self, server_name: String) {
        std::fs::remove_file(&server_name).unwrap_or(());
        let acceptor = Factory::create_acceptor(&server_name).await;
        let (event_sender, mut event_receiver) = mpsc::channel(128);

        loop {
            let result = select! {
                accept_result = self.accept(&acceptor, event_sender.clone()) => {
                    accept_result
                }
                event_result = self.observe_channel_event(&mut event_receiver) => {
                   event_result
                }
            };

            if result.is_err() {
                break;
            }
        }
    }

    pub async fn register_request_callback(&self, cmd: u32, cb: impl Fn(u64, Packet, u64) -> BoxFuture<'static, Packet> + Send + Sync + 'static, cookie: u64) {
        let mut callbacks = self.request_received_callbacks.lock().await;
        callbacks.insert(cmd, (Box::new(cb), cookie));
    }

    pub async fn unregister_request_callback(&self, cmd: u32) -> Option<(RequestReceivedCallback, u64)> {
        let mut callbacks = self.request_received_callbacks.lock().await;
        callbacks.remove(&cmd)
    }

    pub async fn set_channel_closed_callback(&self, cb: impl Fn(u64) -> BoxFuture<'static, ()> + Send + Sync + 'static) {
        let mut callback = self.channel_closed_callback.lock().await;
        *callback = Some(Box::new(cb));
    }

    pub async fn send_push(&self, channel_id: u64, packet: &Packet) -> anyhow::Result<()> {
        let mut channels = self.channels.lock().await;
        ensure!(packet.is_push(), "invalid packet");
        ensure!(channels.contains_key(&channel_id), "channel not ready");
        Ok(channels.get_mut(&channel_id).unwrap().send_packet(&packet).await?)
    }

    async fn accept(&self, acceptor: &Box<dyn StreamAcceptor>, event_sender: mpsc::Sender<Event>) -> anyhow::Result<()> {
        let chan = acceptor.accept(event_sender.clone()).await?;
        let mut channels = self.channels.lock().await;
        channels.insert(chan.id(), chan);
        Ok(())
    }

    async fn observe_channel_event(&self, channel_event_receiver: &mut mpsc::Receiver<Event>) -> anyhow::Result<()> {
        let event = channel_event_receiver.recv().await;
        ensure!(event.is_some(), "channel closed!");
        let event = event.unwrap();

        match event {
            Event::Closed(channel_id) => {
                self.on_channel_closed(channel_id).await;
            }
            Event::GotRequest(channel_id, packet) => {
                if packet.cmd() == HEART_BEAT_CMD {
                    self.on_heartbeat(channel_id, packet).await;
                } else {
                    self.on_request(channel_id, packet).await;
                }
            }
            Event::GotPush(_, _) => {
                println!("server got push, ignore it");
            }
        }

        Ok(())
    }

    async fn on_channel_closed(&self, channel_id: u64) {
        println!("recv closed event, conn_id = {}", channel_id);
        let mut channels = self.channels.lock().await;
        channels.remove(&channel_id);

        let callback = self.channel_closed_callback.lock().await;
        if let Some(cb) = callback.as_ref() {
            cb(channel_id).await;
        }
    }


    async fn on_request(&self, channel_id: u64, packet: Packet) {
        let callbacks = self.request_received_callbacks.lock().await;
        if let Some((callback, cookie)) = callbacks.get(&packet.cmd()) {
            let pack_rp = callback(channel_id, packet, *cookie).await;
            let mut channels = self.channels.lock().await;
            if let Some(channel) = channels.get_mut(&channel_id) {
                let _ = channel.send_packet(&pack_rp).await;
            }
        }
    }

    async fn on_heartbeat(&self, channel_id: u64, packet: Packet) {
        let mut channels = self.channels.lock().await;
        let channel = channels.get_mut(&channel_id);
        if let Some(channel) = channel {
            let rsp_pack = Packet::new_rsp(packet.cmd(), packet.seq(), 0, Bytes::default());
            let _ = channel.send_packet(&rsp_pack).await;
        }
    }
}