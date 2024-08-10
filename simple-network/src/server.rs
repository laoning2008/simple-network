use std::collections::HashMap;
use std::sync::Arc;
use anyhow::ensure;
use bytes::Bytes;
use tokio::runtime::Handle;
use tokio::select;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use crate::{Packet};
use crate::channel::{Channel, Event};
use crate::factory::Factory;
use crate::packet::HEART_BEAT_CMD;

pub struct StreamServer {
    channels: Arc<RwLock<HashMap<u64, Channel>>>,
    request_chan_sender: Arc<RwLock<HashMap<u32, mpsc::Sender<(u64, Packet)>>>>,
    request_chan_receiver: Arc<RwLock<HashMap<u32, mpsc::Receiver<(u64, Packet)>>>>,
    channel_closed_event_receiver: Mutex<mpsc::Receiver<u64>>,
    stop_sender: Option<oneshot::Sender<()>>,
}

impl Drop for StreamServer {
    fn drop(&mut self) {
        let stop_sender = self.stop_sender.take();
        let _ = stop_sender.unwrap().send(());
    }
}

impl StreamServer {
    pub fn new(server_name: &str) -> Self {
        let channels = Arc::new(RwLock::new(HashMap::new()));
        let request_chan_sender = Arc::new(RwLock::new(HashMap::new()));

        let channels_start = channels.clone();
        let request_chan_sender_start = request_chan_sender.clone();
        let (stop_sender, stop_receiver) = oneshot::channel();

        let (channel_closed_event_sender, channel_closed_event_receiver) = mpsc::channel::<u64>(16);

        Self::start(server_name.to_string(), channels_start, request_chan_sender_start, stop_receiver, channel_closed_event_sender);

        Self {
            channels,
            request_chan_sender,
            request_chan_receiver: Arc::new(RwLock::new(HashMap::new())),
            channel_closed_event_receiver: Mutex::new(channel_closed_event_receiver),
            stop_sender: Some(stop_sender),
        }
    }

    pub async fn send_response(&self, channel_id: u64, packet: &Packet) -> anyhow::Result<()> {
        let mut channels = self.channels.write().await;
        ensure!(packet.is_rsp(), "invalid packet");
        ensure!(channels.contains_key(&channel_id), "channel not ready");
        Ok(channels.get_mut(&channel_id).unwrap().send_packet(&packet).await?)
    }

    pub async fn send_push(&self, channel_id: u64, packet: &Packet) -> anyhow::Result<()> {
        let mut channels = self.channels.write().await;
        ensure!(packet.is_push(), "invalid packet");
        ensure!(channels.contains_key(&channel_id), "channel not ready");
        Ok(channels.get_mut(&channel_id).unwrap().send_packet(&packet).await?)
    }

    pub async fn wait_request(&self, cmd: u32) -> (u64, Packet) {
        if !self.request_chan_receiver.read().await.contains_key(&cmd) {
            let (sender, receiver) = mpsc::channel(128);
            self.request_chan_sender.write().await.insert(cmd, sender);
            self.request_chan_receiver.write().await.insert(cmd, receiver);
        }

        self.request_chan_receiver.write().await.get_mut(&cmd).unwrap().recv().await.unwrap()
    }

    pub async fn wait_channel_closed_event(&self) -> u64 {
        self.channel_closed_event_receiver.lock().await.recv().await.unwrap()
    }
}

impl StreamServer {
    fn start(server_name: String, channels: Arc<RwLock<HashMap<u64, Channel>>>, request_chan_sender: Arc<RwLock<HashMap<u32, mpsc::Sender<(u64, Packet)>>>>,stop_receiver: oneshot::Receiver<()>, channel_closed_event_sender: mpsc::Sender<u64>) {
        let channels_observe = channels.clone();
        let (event_sender, event_receiver) = mpsc::channel(128);

        Handle::current().spawn(
            async move {
                select! {
                    _ = stop_receiver => {
                        println!("stop_receiver fired");
                    },
                    _ = Self::accept(server_name, channels, event_sender) => {
                        println!("accept complete");
                    },

                    _ = Self::observe_channel_event(channels_observe, request_chan_sender, event_receiver, channel_closed_event_sender) => {
                        println!("check complete");
                    },
                }
            }
        );
    }

    async fn accept(server_name: String, channels: Arc<RwLock<HashMap<u64, Channel>>>, event_sender: mpsc::Sender<Event>) -> anyhow::Result<()> {
        std::fs::remove_file(&server_name).unwrap_or(());
        let acceptor = Factory::create_acceptor(&server_name).await;

        loop {
            if let Ok(chan) = acceptor.accept(event_sender.clone()).await {
                println!("connect success");
                channels.write().await.insert(chan.id(), chan);
            } else {
                println!("connect failed");
            }
        }
    }


    async fn observe_channel_event(channels: Arc<RwLock<HashMap<u64, Channel>>>, request_chan_sender: Arc<RwLock<HashMap<u32, mpsc::Sender<(u64, Packet)>>>>, mut channel_event_receiver: mpsc::Receiver<Event>, channel_closed_event_sender: mpsc::Sender<u64>) {
        while let Some(event) = channel_event_receiver.recv().await {
            match event {
                Event::Closed(channel_id) => {
                    println!("recv closed event, conn_id = {}", channel_id);
                    channels.write().await.remove(&channel_id);
                    let _ = channel_closed_event_sender.send(channel_id).await;
                }
                Event::GotRequest(channel_id, packet) => {
                    if packet.cmd() == HEART_BEAT_CMD {
                        let mut channels = channels.write().await;
                        let channel = channels.get_mut(&channel_id);
                        if let Some(channel) = channel {
                            let rsp_pack = Packet::new_rsp(packet.cmd(), packet.seq(), 0, Bytes::default());
                            let _ = channel.send_packet(&rsp_pack).await;
                        }
                    } else {
                        if let Some(chan) = request_chan_sender.read().await.get(&packet.cmd()) {
                            let _ = chan.send((channel_id, packet)).await;
                        }
                    }
                }
                Event::GotPush(_, _) => {
                    println!("server got push, ignore it");
                }
            }
        }

        println!("observe_connection_event finished");
    }
}