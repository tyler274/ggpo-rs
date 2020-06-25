use log::error;
use std::mem::size_of;
pub enum MsgType {
    Invalid,
    SyncRequest,
    SyncReply,
    Input,
    QualityReport,
    QualityReply,
    KeepAlive,
    InputAck,
}

pub struct ConnectStatus {
    pub disconnected: i32,
    pub last_frame: i32,
}

impl Default for ConnectStatus {
    fn default() -> Self {
        ConnectStatus {
            disconnected: 1,
            last_frame: 31,
        }
    }
}

struct Header {
    _magic: u16,
    _sequence_number: u16,
    packet_type: MsgType,
}
impl Default for Header {
    fn default() -> Self {
        Self {
            _magic: 0,
            _sequence_number: 0,
            packet_type: MsgType::Invalid,
        }
    }
}
impl Header {
    fn new(t: MsgType) -> Self {
        Header {
            packet_type: t,
            ..Default::default()
        }
    }
}

pub const UDP_MSG_MAX_PLAYERS: usize = 4;
pub const MAX_COMPRESSED_BITS: usize = 4096;

struct SyncRequest {
    _random_request: u32,
    _remote_magic: u16,
    _remote_endpoint: u8,
}

struct SyncReply {
    _random_reply: u32,
}

struct QualityReport {
    _frame_advantage: i8,
    _ping: u32,
}

struct QualityReply {
    _pong: u32,
}

struct Input {
    _peer_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],

    _start_frame: u32,

    _disconnect_requested: i32, // default value should be 1
    _ack_frame: i32,            // default value should be 31

    _num_bits: u16,

    // input_size: u8, // TODO: shouldn't be in every single packet
    _bits: [u8; MAX_COMPRESSED_BITS],
}

struct InputAck {
    _ack_frame: i32, // default value should be 31
}

pub enum MsgEnum {
    SyncRequest {
        random_request: u32,
        remote_magic: u16,
        remote_endpoint: u8,
    },
    SyncReply {
        random_reply: u32,
    },
    QualityReport {
        frame_advantage: i8,
        ping: u32,
    },
    QualityReply {
        pong: u32,
    },
    Input {
        peer_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],

        start_frame: u32,

        disconnect_requested: i32, // default value should be 1
        ack_frame: i32,            // default value should be 31

        num_bits: u16,

        // input_size: u8, // TODO: shouldn't be in every single packet
        bits: [u8; MAX_COMPRESSED_BITS],
    },
    InputAck {
        ack_frame: i32, // default value should be 31
    },
    None,
}

pub struct UdpMsg {
    header: Header,
    message: MsgEnum,
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
                MsgEnum::Input { num_bits, .. } => {
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
    pub fn new(t: MsgType) -> Self {
        match t {
            _ => Self {
                header: Header::new(t),
                ..Default::default()
            },
        }
    }
}
