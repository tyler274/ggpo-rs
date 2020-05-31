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

struct ConnectStatus {
    disconnected: i32,
    last_frame: i32,
}

struct Header {
    magic: u16,
    sequence_number: u16,
    packet_type: MsgType,
}

const UDP_MSG_MAX_PLAYERS: usize = 4;
const MAX_COMPRESSED_BITS: usize = 4096;

enum MsgEnum {
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
        input_size: u8, // TODO: shouldn't be in every single packet
        bits: [u8; MAX_COMPRESSED_BITS],
    },
    InputAck {
        ack_frame: i32, // default value should be 31
    },
}

pub struct UdpMsg {
    header: Header,
    u: MsgEnum,
}

impl UdpMsg {
    pub fn payload_size(self) -> usize {
        match self.header.packet_type {
            _ => std::mem::size_of::<MsgEnum>(),
        }
    }
    pub fn packet_size(self) -> usize {
        std::mem::size_of::<UdpMsg>()
    }
    // pub fn new(t: MsgType) -> UdpMsg {
    //     match t {
    //         MsgType::SyncRequest => UdpMsg {
    //             header: Header {
    //                 magic: 0,
    //                 sequence_number: 0,
    //                 packet_type: t,
    //             },
    //             u: MsgEnum::SyncRequest { random_request: 0 },
    //         },
    //     }
    // }
}
