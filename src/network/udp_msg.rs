use crate::game_input::Frame;
use crate::network::udp_msg;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use log::error;
use serde::{Deserialize, Serialize};
use serde_big_array::big_array;
use std::mem::size_of;

big_array! { BigArray; }

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum MsgType {
    Invalid = 0,
    SyncRequest = 1,
    SyncReply = 2,
    Input = 3,
    QualityReport = 4,
    QualityReply = 5,
    KeepAlive = 6,
    InputAck = 7,
}

#[derive(Serialize, Deserialize, Copy, Clone)]
pub struct ConnectStatus {
    pub disconnected: bool,
    pub last_frame: Frame,
}

impl ConnectStatus {
    pub const fn new() -> Self {
        Self {
            disconnected: true,
            last_frame: None,
        }
    }
}

impl Default for ConnectStatus {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct Header {
    pub magic: u16,
    pub sequence_number: u16,
    pub packet_type: MsgType,
}

impl Header {
    pub const fn new(t: MsgType) -> Self {
        Header {
            packet_type: t,
            magic: 0,
            sequence_number: 0,
        }
    }
}

impl Default for Header {
    fn default() -> Self {
        Header::new(MsgType::Invalid)
    }
}

pub const UDP_MSG_MAX_PLAYERS: usize = 4;
pub const MAX_COMPRESSED_BITS: usize = 4096;
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct SyncRequest {
    pub random_request: u32,
    pub remote_magic: u16,
    pub remote_endpoint: u8,
}
impl Default for SyncRequest {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncRequest {
    pub const fn new() -> Self {
        Self {
            random_request: 0,
            remote_endpoint: 0,
            remote_magic: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Copy, Clone, Debug)]
pub struct SyncReply {
    pub random_reply: u32,
}

impl SyncReply {
    pub const fn new() -> Self {
        Self { random_reply: 0 }
    }
}
#[derive(Serialize, Deserialize, Default, Copy, Clone, Debug)]
pub struct QualityReport {
    pub frame_advantage: i8,
    pub ping: u128,
}

impl QualityReport {
    pub const fn new() -> Self {
        Self {
            frame_advantage: 0,
            ping: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Copy, Clone, Debug)]
pub struct QualityReply {
    pub pong: u128,
}

impl QualityReply {
    pub const fn new() -> Self {
        Self { pong: 0 }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone)]
pub struct Input {
    pub peer_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],

    pub start_frame: Frame,

    pub disconnect_requested: bool, // default value should be 1
    pub ack_frame: Frame,           // default value should be 31

    pub num_bits: u16,

    #[serde(with = "BigArray")]
    pub bits: [u8; MAX_COMPRESSED_BITS],
}

impl Input {
    pub const fn new() -> Self {
        Self {
            bits: [b'0'; MAX_COMPRESSED_BITS],
            peer_connect_status: [ConnectStatus::new(); UDP_MSG_MAX_PLAYERS],
            start_frame: Some(0),
            disconnect_requested: true,
            ack_frame: Some(31),

            num_bits: 0,
        }
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct InputAck {
    pub ack_frame: Frame, // default value should be 31
}

impl InputAck {
    pub const fn new() -> Self {
        Self {
            ack_frame: Some(31),
        }
    }
}

impl Default for InputAck {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize, Copy, Clone)]
pub enum MsgEnum {
    SyncRequest(SyncRequest),
    SyncReply(SyncReply),
    QualityReport(QualityReport),
    QualityReply(QualityReply),
    Input(Input),
    InputAck(InputAck),
    KeepAlive,
    None,
}

#[derive(Serialize, Deserialize, Copy, Clone)]
pub struct UdpMsg {
    pub header: Header,
    pub message: MsgEnum,
}

impl Default for UdpMsg {
    fn default() -> Self {
        Self {
            header: Default::default(),
            message: MsgEnum::None,
        }
    }
}

impl UdpMsg {
    pub fn payload_size(&self) -> usize {
        let mut size: usize;

        return match self.header.packet_type {
            MsgType::SyncRequest => size_of::<SyncRequest>(),
            MsgType::SyncReply => size_of::<SyncReply>(),
            MsgType::QualityReport => size_of::<QualityReport>(),
            MsgType::QualityReply => size_of::<QualityReply>(),
            MsgType::InputAck => size_of::<InputAck>(),
            MsgType::KeepAlive => 0,
            MsgType::Input => match self.message {
                MsgEnum::Input(Input { num_bits, .. }) => {
                    // The original computed this using the addresses within the union itself.
                    size = size_of::<Input>() - size_of::<[u8; MAX_COMPRESSED_BITS]>();
                    size += (num_bits as usize + 7) / 8;
                    size
                }
                _ => {
                    error!("Input header but not input packet?");
                    unreachable!();
                }
            },
            MsgType::Invalid => {
                error!("Invalid packet payload size");
                unreachable!();
            }
        };
    }
    pub fn packet_size(self) -> usize {
        size_of::<Header>() + self.payload_size()
    }
    pub const fn new(t: MsgType) -> Self {
        match t {
            MsgType::Input => Self {
                header: Header::new(t),
                message: MsgEnum::Input(Input::new()),
            },
            MsgType::Invalid => Self {
                header: Header::new(t),
                message: MsgEnum::None,
            },
            MsgType::SyncRequest => Self {
                header: Header::new(t),
                message: MsgEnum::SyncRequest(SyncRequest::new()),
            },
            MsgType::SyncReply => Self {
                header: Header::new(t),
                message: MsgEnum::SyncReply(SyncReply::new()),
            },
            MsgType::QualityReport => Self {
                header: Header::new(t),
                message: MsgEnum::QualityReport(QualityReport::new()),
            },
            MsgType::QualityReply => Self {
                header: Header::new(t),
                message: MsgEnum::QualityReply(QualityReply::new()),
            },
            MsgType::KeepAlive => Self {
                header: Header::new(t),
                message: MsgEnum::None,
            },
            MsgType::InputAck => Self {
                header: Header::new(t),
                message: MsgEnum::InputAck(InputAck::new()),
            },
        }
    }
}
