use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use anyhow::ensure;
use bytes::Bytes;
use futures::future::BoxFuture;
use tokio::{select, time};
use tokio::sync::{mpsc};
use crate::{ChannelClosedCallback, Packet, PushReceivedCallback};
use crate::channel::{Channel, Event};
use crate::factory::Factory;
use crate::packet::HEART_BEAT_CMD;
use crate::connector::StreamConnector;

const RECONNECT_INTERVAL_MILLIS : u64 = 1000;
const RE_OBSERVER_EVENT_INTERVAL_MILLIS : u64 = 500;
const HEARTBEAT_INTERVAL_MILLIS : u64 = 2 * 1000;


pub struct StreamClient {
    server_name: String,
    channel: Mutex<Option<Channel>>,
    channel_event_sender: Mutex<Option<mpsc::Sender<Event>>>,
    channel_closed_callback: Mutex<Option<ChannelClosedCallback>>,
    push_received_callbacks: Mutex<HashMap<u32, (PushReceivedCallback, u64)>>,
}

impl StreamClient {
    pub fn new(server_name: &str) -> Self {

        Self {
            server_name: server_name.to_string(),
            channel: Default::default(),
            channel_event_sender: Default::default(),
            channel_closed_callback: Default::default(),
            push_received_callbacks: Default::default(),
        }
    }

    pub async fn connect(&self) -> anyhow::Result<()> {
        if self.channel.lock().unwrap().is_some() {
            return Ok(());
        }

        let mut connector = Factory::create_connector(&self.server_name);
        let chan= connector.connect(self.channel_event_sender.lock().unwrap().as_ref().unwrap().clone()).await?;
        let _ = self.channel.lock().unwrap().insert(chan);
        Ok(())
    }

    pub async fn send_request(&self, packet: &Packet, timeout_seconds: u64) -> anyhow::Result<Packet> {
        let mut channel = self.channel.lock().unwrap();
        ensure!(channel.is_some(), "disconnected");
        channel.as_mut().unwrap().send_request_wait_response(packet, timeout_seconds).await
    }

    pub fn register_push_callback(&self, cmd: u32, cb: impl Fn(u64, Packet, u64) -> BoxFuture<'static, ()> + Send + Sync + 'static, cookie: u64) {
        let mut callbacks = self.push_received_callbacks.lock().unwrap();
        callbacks.insert(cmd, (Box::new(cb), cookie));
    }

    pub fn unregister_push_callback(&self, cmd: u32) -> Option<(PushReceivedCallback, u64)> {
        let mut callbacks = self.push_received_callbacks.lock().unwrap();
        callbacks.remove(&cmd)
    }

    pub fn set_channel_closed_callback(&self, cb: impl Fn(u64) -> BoxFuture<'static, ()> + Send + Sync + 'static) {
        let mut callback = self.channel_closed_callback.lock().unwrap();
        *callback = Some(Box::new(cb));
    }

    pub async fn run(&self) {
        let (channel_event_sender, channel_event_receiver) = mpsc::channel(8);
        let _ = self.channel_event_sender.lock().unwrap().insert(channel_event_sender);

        select! {
            _ = self.observe_channel_event(channel_event_receiver) => {
                println!("check complete");
            },
            _ = self.do_heartbeat() => {
                println!("heartbeat complete");
            },
        }
    }

    async fn do_heartbeat(&self) {
        loop {
            let mut channel = self.channel.lock().unwrap();
            if let Some(channel) = channel.as_mut() {
                let heartbeat_pack = Packet::new_req(HEART_BEAT_CMD, Bytes::default());
                let _ = channel.send_packet(&heartbeat_pack).await;
            }

            time::sleep(Duration::from_millis(HEARTBEAT_INTERVAL_MILLIS)).await;
        }
    }

    async fn observe_channel_event(&self, mut channel_event_receiver: mpsc::Receiver<Event>) {
        while let Some(event) = channel_event_receiver.recv().await {
            match event {
                Event::Closed(channel_id) => {
                    self.on_channel_closed(channel_id).await;
                    break;
                }
                Event::GotPush(channel_id, packet) => {
                    self.on_push(channel_id, packet).await;
                }
                Event::GotRequest(_, packet) => {
                    println!("client got request, ignore it, cmd = {}", packet.cmd());
                }
            }
        }
    }

    async fn on_channel_closed(&self, channel_id: u64) {
        println!("recv closed event, conn_id = {}", channel_id);
        let mut channel = self.channel.lock().unwrap();
        *channel = None;

        let callback = self.channel_closed_callback.lock().unwrap();
        if let Some(cb) = callback.as_ref() {
            cb(channel_id).await;
        }
    }


    async fn on_push(&self, channel_id: u64, packet: Packet) {
        let callbacks = self.push_received_callbacks.lock().unwrap();
        if let Some((callback, cookie)) = callbacks.get(&packet.cmd()) {
            callback(channel_id, packet, *cookie).await;
        }
    }
}
