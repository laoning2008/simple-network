use bytes::{Buf, Bytes};
use std::io::{Cursor};
use bytebuffer::{ByteBuffer, Endian};
use std::sync::atomic::{AtomicUsize, Ordering};

const PACKET_BEGIN_FLAG: u8 = 0x55;

// #[repr(packed(1))]
// struct PacketHeader {
//     flag: u8,
//     cmd: u32,
//     seq: u32,
//     type: u8,
//     ec: u32,
//     body_len: u32,
// }

const REQ_TYPE : u8 = 1u8;
const RSP_TYPE : u8 = 2u8;
const PUSH_TYPE : u8 = 3u8;

const HEADER_LENGTH : usize = 18;//size_of::<PacketHeader>();
const MAX_BODY_LENGTH: usize = 16*1024;
static SEQUENCE: AtomicUsize = AtomicUsize::new(1);

pub const HEART_BEAT_CMD : u32 = 0;
pub const HEARTBEAT_INTERVAL_SECONDS: u64 = 1;

pub struct Packet {
    cmd: u32,
    seq: u32,
    t: u8,
    ec: u32,
    body: Bytes,
}

impl Packet {
    pub fn new_req(cmd: u32, body: Bytes) -> Packet {
        let seq = SEQUENCE.fetch_add(1, Ordering::SeqCst) as u32;
        Packet { cmd, seq, t: REQ_TYPE, ec: 0, body}
    }

    pub fn new_rsp(cmd: u32, seq: u32, ec: u32, body: Bytes) -> Packet {
        Packet { cmd, seq, t: RSP_TYPE, ec, body}
    }

    pub fn new_push(cmd: u32, body: Bytes) -> Packet {
        let seq = SEQUENCE.fetch_add(1, Ordering::SeqCst) as u32;
        Packet { cmd, seq, t: PUSH_TYPE, ec: 0, body}
    }

    pub fn from_bytes(buf: &mut Cursor<&[u8]>) -> Option<Packet> {
        if buf.get_ref().is_empty() {
            return None;
        }

        loop {
            let start = buf.position() as usize;
            let end = buf.get_ref().len() - 1;

            let mut pos : usize = start;
            while pos < end {
                if buf.get_ref()[pos] == PACKET_BEGIN_FLAG {
                    break;
                }

                pos = pos + 1;
            }

            buf.set_position(pos as u64);

            if pos == end {
                return None;
            }

            let buf_valid_len = buf.remaining();

            if buf_valid_len < HEADER_LENGTH as usize {
                return None;
            }

            buf.get_u8();//flag
            let cmd = buf.get_u32_le();
            let seq = buf.get_u32_le();
            let t = buf.get_u8();
            let ec = buf.get_u32_le();
            let body_len = buf.get_u32_le();

            if body_len as usize > MAX_BODY_LENGTH {
                continue;
            }

            let packet_len = HEADER_LENGTH + body_len as usize;
            if buf_valid_len < packet_len {
                return None;
            }

            let body_start = buf.position() as usize ;
            let body_end = body_start + body_len as usize;
            let body : Bytes = Bytes::copy_from_slice(&buf.get_ref()[body_start..body_end]);
            buf.set_position(body_end as u64);

            return Some(Packet { cmd, seq, t, ec, body});
        }
    }

    pub fn to_bytes(&self) -> ByteBuffer {
        let mut buffer: ByteBuffer = ByteBuffer::new();
        buffer.set_endian(Endian::LittleEndian);//network order

        buffer.write_u8(PACKET_BEGIN_FLAG);
        buffer.write_u32(self.cmd);
        buffer.write_u32(self.seq);
        buffer.write_u8(self.t);
        buffer.write_u32(self.ec);
        buffer.write_u32(self.body.len() as u32);
        buffer.write_bytes(&self.body);
        return buffer;
    }

    pub fn cmd(&self) -> u32 {
        self.cmd
    }

    pub fn seq(&self) -> u32 {
        self.seq
    }

    pub fn packet_type(&self) -> u8 {
        self.t
    }

    pub fn is_req(&self) -> bool {
        self.t == REQ_TYPE
    }

    pub fn is_rsp(&self) -> bool {
        self.t == RSP_TYPE
    }

    pub fn is_push(&self) -> bool {
        self.t == PUSH_TYPE
    }

    pub fn ec(&self) -> u32 {
        self.ec
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}