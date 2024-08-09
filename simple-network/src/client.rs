use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc};
use std::time::Duration;
use anyhow::ensure;
use bytes::Bytes;
use tokio::runtime::Handle;
use tokio::{select, time};
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;
use crate::{Packet};
use crate::channel::{Channel, Event};
use crate::factory::Factory;
use crate::packet::HEART_BEAT_CMD;


const RECONNECT_INTERVAL_MILLIS : u64 = 1000;
const RE_OBSERVER_EVENT_INTERVAL_MILLIS : u64 = 500;
const HEARTBEAT_INTERVAL_MILLIS : u64 = 2 * 1000;


pub struct StreamClient {
    channel: Arc<RwLock<Option<Channel>>>,
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
        let channel: Arc<RwLock<Option<Channel>>> = Arc::new(Default::default());
        let push_chan_sender = Arc::new(RwLock::new(HashMap::new()));
        let push_chan_receiver = Arc::new(RwLock::new(HashMap::new()));

        let (stop_sender, stop_receiver) = oneshot::channel();
        let channel_start = channel.clone();
        let push_chan_sender_start = push_chan_sender.clone();

        Self::start(server_name.to_string(), channel_start, push_chan_sender_start, stop_receiver);

        Self {
            channel,
            push_chan_sender,
            push_chan_receiver,
            stop_sender: Some(stop_sender),
        }
    }

    pub async fn send_request_wait_response(&mut self, packet: &Packet, timeout_seconds: u64) -> anyhow::Result<Packet> {
        let mut channel = self.channel.write().await;
        ensure!(channel.is_some(), "must be request packet");
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
    fn start(server_name: String, channel: Arc<RwLock<Option<Channel>>>, push_chan_sender: Arc<RwLock<HashMap<u32, mpsc::Sender<Packet>>>>, stop_receiver: oneshot::Receiver<()>) {
        let channel_event = channel.clone();
        let channel_heartbeat = channel.clone();
        let push_chan_sender = push_chan_sender.clone();

        let event_receiver_connect: Arc<RwLock<Option<mpsc::Receiver<Event>>>> = Arc::new(RwLock::new(None));
        let event_receiver_observe = event_receiver_connect.clone();

        Handle::current().spawn(
            async move {
                select! {
                    _ = stop_receiver => {
                        println!("stop_receiver fired");
                    },
                    _ = Self::connect(&server_name, channel, event_receiver_connect) => {
                        println!("connect complete");
                    },
                    _ = Self::observe_channel_event(channel_event, push_chan_sender, event_receiver_observe) => {
                        println!("check complete");
                    },
                    _ = Self::heartbeat(channel_heartbeat) => {
                        println!("heartbeat complete");
                    },
                }
            }
        );
    }



    async fn connect(server_name: &str, channel: Arc<RwLock<Option<Channel>>>, event_receiver: Arc<RwLock<Option<mpsc::Receiver<Event>>>>) {
        let mut connector = Factory::create_connector(server_name);

        async move {
            loop {
                if channel.read().await.is_none() {
                    let (chan_event_sender, chan_event_receiver) = mpsc::channel(128);
                    if let Ok(chan) = connector.connect(chan_event_sender.clone()).await {
                        println!("connect success");
                        *event_receiver.write().await = Some(chan_event_receiver);
                        *channel.write().await = Some(chan);
                    } else {
                        println!("connect failed");
                    }
                }

                time::sleep(Duration::from_millis(RECONNECT_INTERVAL_MILLIS)).await;
            }
        }
    }



    async fn heartbeat(channel: Arc<RwLock<Option<Channel>>>) {
        async move {
            loop {
                if let Some(channel) = channel.write().await.as_mut() {
                    let heartbeat_pack = Packet::new_req(HEART_BEAT_CMD, Bytes::default());
                    let _ = channel.send_packet(&heartbeat_pack).await;
                }

                time::sleep(Duration::from_millis(HEARTBEAT_INTERVAL_MILLIS)).await;
            }
        }
    }



    async fn observe_channel_event(channel: Arc<RwLock<Option<Channel>>>, push_chan_sender: Arc<RwLock<HashMap<u32, mpsc::Sender<Packet>>>>, event_receiver: Arc<RwLock<Option<mpsc::Receiver<Event>>>>) {
        async move {
            loop {
                let mut event_receiver = event_receiver.write().await;
                if let Some(receiver) = event_receiver.as_mut() {
                    while let Some(event) = receiver.recv().await {
                        match event {
                            Event::Closed(_) => {
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
}
