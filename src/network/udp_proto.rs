use crate::network::udp_msg::{ConnectStatus, UdpMsg, UDP_MSG_MAX_PLAYERS};
use arraydeque::ArrayDeque;
use std::collections::VecDeque;
use std::net::SocketAddr;

use crate::network::udp::Udp;

enum State {
    Syncing,
    Synchronized,
    Running,
    Disconnected,
}

struct QueueEntry {
    queue_time: i32,
    dest_addr: SocketAddr,
    msg: *mut UdpMsg,
}

impl QueueEntry {
    pub const fn new(time: i32, dst: &SocketAddr, m: *mut UdpMsg) -> QueueEntry {
        QueueEntry {
            queue_time: time,
            dest_addr: *dst,
            msg: m,
        }
    }
}

struct OoPacket<'a> {
    send_time: i32,
    dest_addr: SocketAddr,
    msg: &'a UdpMsg,
}

pub struct UdpProtocol<'a, 'b, 'c> {
    /*
     * Network transmission information
     */
    udp: &'a mut Udp<'a, 'a>,
    peer_addr: SocketAddr,
    magic_number: u16,
    queue: i32,
    remote_magic_number: u16,
    connected: bool,
    send_latency: i32,
    oop_percent: i32,
    oo_packet: OoPacket<'b>,
    send_queue: ArrayDeque<[QueueEntry; 64]>,
    /*
     * Stats
     */
    round_trip_time: i32,
    packets_sent: i32,
    bytes_sent: i32,
    kbps_sent: i32,
    stats_start_time: i32,
    /*
     * The state machine
     */
    local_connect_status: &'c ConnectStatus,
    peer_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],

    current_state: State,
}

// impl UdpProtocol {
//     fn new(&mut self) {
//         self.send_queue.pu
//     }
// }
