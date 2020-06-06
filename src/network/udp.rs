use mio::net::UdpSocket;
// use mio::{Events, Poll, Token};
// use serde::{Deserialize, Serialize};
// use std::env;
use std::net::SocketAddr;
// use std::time::Duration;

use crate::network::udp_msg::UdpMsg;

/// The socket address of where the server is located.
const SERVER_ADDR: &'static str = "127.0.0.1:12345";
// The client address from where the data is sent.
const CLIENT_ADDR: &'static str = "127.0.0.1:12346";

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
    pub fn new(time: i32, dst: &SocketAddr, m: *mut UdpMsg) -> QueueEntry {
        QueueEntry {
            queue_time: time,
            dest_addr: *dst,
            msg: m,
        }
    }
}

pub struct Udp {
    socket: *mut UdpSocket,
}

impl Udp {
    // pub fn new() -> Udp {
    //     Udp {
    //         socket: Socket::bind(),
    //     }
    // }
}
