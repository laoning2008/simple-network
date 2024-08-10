use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use anyhow::{ensure};
use bytes::{Buf, BytesMut};
use scopeguard::defer;
use tokio::runtime::Handle;
use tokio::{select, time};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use crate::Packet;
use crate::reader::StreamReader;
use crate::writer::StreamWriter;

const RECV_BUF_SIZE: usize = 128*1024;
const ACTIVE_CONNECTION_LIFETIME_MILLIS: u128 = 6 * 1000;
const ACTIVE_CONNECTION_LIFETIME_CHECK_INTERVAL_MILLIS: u64 = 1 * 1000;
static CHANNEL_ID: AtomicU64 = AtomicU64::new(1);

pub enum Event {
    Closed(u64),
    GotRequest(u64, Packet),
    GotPush(u64, Packet),
}

pub struct Channel {
    channel_id: u64,
    stream_writer: Box<dyn StreamWriter>,
    rsp_chan_sender: Arc<RwLock<HashMap<u64, oneshot::Sender<Packet>>>>,
    stop_sender: Option<oneshot::Sender<()>>,
}

impl Drop for Channel {
    fn drop(&mut self) {
        let stop_sender = self.stop_sender.take();
        let _ = stop_sender.unwrap().send(());
    }
}

impl Channel {
    pub fn new(stream_reader: Box<dyn StreamReader>, stream_writer: Box<dyn StreamWriter>, event_sender: mpsc::Sender<Event>) -> Self {
        let channel_id = CHANNEL_ID.fetch_add(1, Ordering::SeqCst);
        let rsp_chan_sender: Arc<RwLock<HashMap<u64, oneshot::Sender<Packet>>>> = Arc::new(Default::default());
        let rsp_chan_sender_clone = rsp_chan_sender.clone();
        let (stop_sender, stop_receiver) = oneshot::channel();
        Self::start(channel_id, stream_reader, event_sender, rsp_chan_sender_clone, stop_receiver);

        Self {
            channel_id,
            stream_writer,
            rsp_chan_sender,
            stop_sender: Some(stop_sender),
        }
    }

    pub async fn send_packet(&mut self, packet: &Packet) -> anyhow::Result<()> {
        let data = packet.to_bytes();
        self.stream_writer.write_all(data.as_bytes()).await?;
        Ok(())
    }


    pub async fn send_request_wait_response(&mut self, packet: &Packet, timeout_millis: u64) -> anyhow::Result<Packet> {
        ensure!(packet.is_req(), "must be request packet");
        let data = packet.to_bytes();
        self.stream_writer.write_all(data.as_bytes()).await?;

        let (sender, receiver) = oneshot::channel();
        let packet_id = Self::get_packet_id(packet.cmd(), packet.seq());

        self.rsp_chan_sender.write().unwrap().insert(packet_id, sender);
        defer! {self.rsp_chan_sender.write().unwrap().remove(&packet_id);}

        Ok(timeout(Duration::from_millis(timeout_millis), receiver).await??)
    }

    pub fn id(&self) -> u64 {
        self.channel_id
    }
}



impl Channel {
    fn start(channel_id: u64, stream_reader: Box<dyn StreamReader>, event_sender: mpsc::Sender<Event>, rsp_chan_sender: Arc<RwLock<HashMap<u64, oneshot::Sender<Packet>>>>, stop_receiver: oneshot::Receiver<()>) {
        let event_sender_check = event_sender.clone();
        let last_recv_time = Arc::new(RwLock::new(Instant::now()));

        let last_recv_time_clone = last_recv_time.clone();

        Handle::current().spawn(
            async move {
                select! {
                    _ = stop_receiver => {
                        println!("stop_receiver fired");
                    },
                    _ = Self::receive_packet(channel_id, rsp_chan_sender, stream_reader, last_recv_time, event_sender) => {
                        println!("receive_packet complete");
                    },
                    _ = Self::check(channel_id, last_recv_time_clone, event_sender_check) => {
                        println!("check complete");
                    },
                }
            }
        );
    }



    async fn receive_packet(channel_id: u64, rsp_chan_sender: Arc<RwLock<HashMap<u64, oneshot::Sender<Packet>>>>, mut stream_reader: Box<dyn StreamReader>, last_recv_time: Arc<RwLock<Instant>>, event_sender: mpsc::Sender<Event>) {
        let mut read_buffer =  BytesMut::with_capacity(RECV_BUF_SIZE);
        loop {
            let received_packets = Self::process_packet(&mut read_buffer);
            let mut should_stop = false;

            for pack in received_packets {
                *last_recv_time.write().unwrap() = Instant::now();
                if pack.is_rsp() {
                    let packet_id = Self::get_packet_id(pack.cmd(), pack.seq());
                    let mut rsp_chan_sender = rsp_chan_sender.write().unwrap();
                    if !rsp_chan_sender.contains_key(&packet_id) {
                        continue;
                    }

                    let rsp_chan_sender_for_packet = rsp_chan_sender.remove(&packet_id).unwrap();
                    if rsp_chan_sender_for_packet.send(pack).is_err() {
                        println!("send response to channel failed");
                    }

                } else if pack.is_req() {
                    if event_sender.send(Event::GotRequest(channel_id, pack)).await.is_err() {
                        println!("send request to channel failed");
                        should_stop = true;
                        break;
                    }
                } else {
                    if event_sender.send(Event::GotPush(channel_id, pack)).await.is_err() {
                        println!("send push to channel failed");
                        should_stop = true;
                        break;
                    }
                }
            }

            if should_stop {
                println!("should_stop");
                break;
            }

            let read_result = stream_reader.read_some(&mut read_buffer).await;
            if read_result.is_err() {
                println!("receive_buffer failed");
                break;
            }
        }

        let _ = event_sender.send(Event::Closed(channel_id)).await;
    }


    async fn check(channel_id: u64, last_recv_time: Arc<RwLock<Instant>>, connection_event_sender: mpsc::Sender<Event>) {
        loop {
            time::sleep(Duration::from_millis(ACTIVE_CONNECTION_LIFETIME_CHECK_INTERVAL_MILLIS)).await;

            let now = Instant::now();
            let duration = now.duration_since(*last_recv_time.read().unwrap());
            if duration.as_millis() > ACTIVE_CONNECTION_LIFETIME_MILLIS {
                break;
            }
        }

        let _ = connection_event_sender.send(Event::Closed(channel_id)).await;
    }



    fn process_packet(read_buffer: &mut BytesMut) -> Vec<Packet> {
        let mut buf = Cursor::new(&read_buffer[..]);
        let mut packets: Vec<Packet> = Vec::new();

        loop {
            if let Some(pack) = Packet::from_bytes(&mut buf) {
                packets.push(pack);
                continue;
            } else {
                break;
            }
        }

        let consume_len = buf.position() as usize;
        read_buffer.advance(consume_len);
        return packets;
    }



    fn get_packet_id(cmd: u32, seq: u32) -> u64 {
        let mut packet_id : u64 = cmd as u64;
        packet_id = packet_id << 31;
        packet_id |= seq as u64;
        return packet_id;
    }
}
