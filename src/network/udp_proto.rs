use crate::network::udp_msg::{ConnectStatus, UdpMsg, UDP_MSG_MAX_PLAYERS};
use arraydeque::ArrayDeque;
use std::net::SocketAddr;

use crate::network::udp::Udp;

enum State {
    _Syncing,
    _Synchronized,
    _Running,
    _Disconnected,
}

struct QueueEntry {
    _queue_time: i32,
    _dest_addr: SocketAddr,
    _msg: Box<UdpMsg>,
}

impl QueueEntry {
    pub const fn _new(time: i32, dst: &SocketAddr, m: Box<UdpMsg>) -> QueueEntry {
        QueueEntry {
            _queue_time: time,
            _dest_addr: *dst,
            _msg: m,
        }
    }
}

struct OoPacket<'a> {
    _send_time: i32,
    _dest_addr: SocketAddr,
    _msg: &'a UdpMsg,
}

pub struct UdpProtocol<'a, 'b, 'c> {
    /*
     * Network transmission information
     */
    _udp: &'a mut Udp<'a, 'a>,
    _peer_addr: SocketAddr,
    _magic_number: u16,
    _queue: i32,
    _remote_magic_number: u16,
    _connected: bool,
    _send_latency: i32,
    _oop_percent: i32,
    _oo_packet: OoPacket<'b>,
    _send_queue: ArrayDeque<[QueueEntry; 64]>,
    /*
     * Stats
     */
    _round_trip_time: i32,
    _packets_sent: i32,
    _bytes_sent: i32,
    _kbps_sent: i32,
    _stats_start_time: i32,
    /*
     * The state machine
     */
    _local_connect_status: &'c ConnectStatus,
    _peer_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],

    _current_state: State,
}

// impl UdpProtocol {
//     fn new(&mut self) {
//         self.send_queue.pu
//     }
// }
