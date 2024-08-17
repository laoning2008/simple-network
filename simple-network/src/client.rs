use std::collections::HashMap;
use std::sync::{Arc};
use std::time::Duration;
use anyhow::ensure;
use bytes::Bytes;
use tokio::runtime::Handle;
use tokio::{select, time};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use crate::{Packet};
use crate::channel::{Channel, Event};
use crate::factory::Factory;
use crate::packet::HEART_BEAT_CMD;
use crate::connector::StreamConnector;

const RECONNECT_INTERVAL_MILLIS : u64 = 1000;
const RE_OBSERVER_EVENT_INTERVAL_MILLIS : u64 = 500;
const HEARTBEAT_INTERVAL_MILLIS : u64 = 2 * 1000;


pub struct StreamClient {
    server_name: String,
    channel: Arc<RwLock<Option<Channel>>>,
    channel_event_receiver: Arc<RwLock<Option<mpsc::Receiver<Event>>>>,
    channel_closed_event_receiver: Mutex<mpsc::Receiver<()>>,

    push_chan_sender: Arc<RwLock<HashMap<u32, mpsc::Sender<Packet>>>>,
    push_chan_receiver: Arc<RwLock<HashMap<u32, mpsc::Receiver<Packet>>>>,

    stop_sender: Option<oneshot::Sender<()>>,
}

impl Drop for StreamClient {
    fn drop(&mut self) {
        let stop_sender = self.stop_sender.take();
        let _ = stop_sender.unwrap().send(());
    }
}

impl StreamClient {
    pub fn new(server_name: &str) -> Self {
        let channel: Arc<RwLock<Option<Channel>>> = Default::default();
        let channel_event_receiver: Arc<RwLock<Option<mpsc::Receiver<Event>>>> = Default::default();
        let (channel_closed_event_sender, channel_closed_event_receiver) = mpsc::channel::<()>(1);

        let push_chan_sender = Arc::new(RwLock::new(HashMap::new()));
        let push_chan_receiver = Arc::new(RwLock::new(HashMap::new()));

        let channel_event_receiver_start = channel_event_receiver.clone();
        let (stop_sender, stop_receiver) = oneshot::channel();
        let channel_start = channel.clone();
        let push_chan_sender_start = push_chan_sender.clone();
        Self::start(channel_start, channel_event_receiver_start, channel_closed_event_sender, push_chan_sender_start, stop_receiver);


        Self {
            server_name: server_name.to_string(),
            channel,
            channel_event_receiver,
            channel_closed_event_receiver: Mutex::new(channel_closed_event_receiver),
            push_chan_sender,
            push_chan_receiver,
            stop_sender: Some(stop_sender),
        }
    }

    pub async fn connect(&self) -> anyhow::Result<()> {
        if self.channel.read().await.is_some() {
            return Ok(());
        }

        let mut connector = Factory::create_connector(&self.server_name);
        let (chan_event_sender, chan_event_receiver) = mpsc::channel(128);
        let chan= connector.connect(chan_event_sender.clone()).await?;
        let _ = self.channel.write().await.insert(chan);
        let _ = self.channel_event_receiver.write().await.insert(chan_event_receiver);
        Ok(())
    }

    pub async fn wait_closed_event(&self) {
        self.channel_closed_event_receiver.lock().await.recv().await.unwrap()
    }

    pub async fn send_request(&self, packet: &Packet, timeout_seconds: u64) -> anyhow::Result<Packet> {
        let mut channel = self.channel.write().await;
        ensure!(channel.is_some(), "disconnected");
        channel.as_mut().unwrap().send_request_wait_response(packet, timeout_seconds).await
    }

    pub async fn wait_push(&self, cmd: u32) -> Packet {
        if !self.push_chan_receiver.read().await.contains_key(&cmd) {
            let (sender, receiver) = mpsc::channel(128);
            self.push_chan_sender.write().await.insert(cmd, sender);
            self.push_chan_receiver.write().await.insert(cmd, receiver);
        }

        self.push_chan_receiver.write().await.get_mut(&cmd).unwrap().recv().await.unwrap()
    }
}


impl StreamClient {
    fn start(channel: Arc<RwLock<Option<Channel>>>, channel_event_receiver: Arc<RwLock<Option<mpsc::Receiver<Event>>>>, channel_closed_event_sender: mpsc::Sender<()>, push_chan_sender: Arc<RwLock<HashMap<u32, mpsc::Sender<Packet>>>>, stop_receiver: oneshot::Receiver<()>) {
        let channel_observe = channel.clone();
        let channel_heartbeat = channel.clone();

        let push_chan_sender = push_chan_sender.clone();

        Handle::current().spawn(
            async move {
                select! {
                    _ = stop_receiver => {
                        println!("stop_receiver fired");
                    },
                    _ = Self::observe_channel_event(channel_observe, channel_closed_event_sender, push_chan_sender, channel_event_receiver) => {
                        println!("check complete");
                    },
                    _ = Self::heartbeat(channel_heartbeat) => {
                        println!("heartbeat complete");
                    },
                }
            }
        );
    }

    async fn heartbeat(channel: Arc<RwLock<Option<Channel>>>) {
        loop {
            if let Some(channel) = channel.write().await.as_mut() {
                let heartbeat_pack = Packet::new_req(HEART_BEAT_CMD, Bytes::default());
                let _ = channel.send_packet(&heartbeat_pack).await;
            }

            time::sleep(Duration::from_millis(HEARTBEAT_INTERVAL_MILLIS)).await;
        }
    }

    async fn observe_channel_event(channel: Arc<RwLock<Option<Channel>>>, channel_closed_event_sender: mpsc::Sender<()>, push_chan_sender: Arc<RwLock<HashMap<u32, mpsc::Sender<Packet>>>>, event_receiver: Arc<RwLock<Option<mpsc::Receiver<Event>>>>) {
        loop {
            let mut event_receiver = event_receiver.write().await;
            if let Some(receiver) = event_receiver.as_mut() {
                while let Some(event) = receiver.recv().await {
                    match event {
                        Event::Closed(_) => {
                            let _ = channel_closed_event_sender.send(()).await;
                            println!("connection closed");
                            break;
                        }
                        Event::GotPush(_, packet) => {
                            if let Some(sender) = push_chan_sender.read().await.get(&packet.cmd()) {
                                let _ = sender.send(packet).await;
                            }
                        }
                        Event::GotRequest(_, packet) => {
                            println!("client got request, ignore it, cmd = {}", packet.cmd());
                        }
                    }
                }

                *channel.write().await = None;
                *event_receiver = None;
            }

            time::sleep(Duration::from_millis(RE_OBSERVER_EVENT_INTERVAL_MILLIS)).await;
        }
    }
}
