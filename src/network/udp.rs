use mio::net::UdpSocket;
use mio::{Events, Poll, Token};
// use serde::{Deserialize, Serialize};
// use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
// use std::time::Duration;

use crate::network::udp_msg::UdpMsg;

/// The socket address of where the server is located.
const SERVER_ADDR: &'static str = "127.0.0.1:12345";
// The client address from where the data is sent.
const CLIENT_ADDR: &'static str = "127.0.0.1:12346";

struct Callbacks {}

impl Callbacks {
    fn on_msg(from: &SocketAddr, msg: &UdpMsg, len: usize) {}
}

pub struct Udp<'callbacks, 'poll> {
    // Network transmission information
    socket: UdpSocket,

    // state management
    callbacks: &'callbacks mut Callbacks,
    poll: &'poll mut Poll,
}

impl<'callbacks, 'poll> Udp<'callbacks, 'poll> {
    // pub fn new() -> Self {
    //     Udp {
    //         socket: SocketAddr::new(ip, port),
    //     }
    // }
}
