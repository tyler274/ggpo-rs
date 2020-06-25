use crate::network::udp_msg::UdpMsg;
// use bytes::{BufMut, BytesMut};
use bytes::BytesMut;
use log::{error, info};
use mio::net::UdpSocket;
use mio::{Interest, Poll, Token};
// use socket2::{Domain, SockAddr, Socket};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
// use std::time::Duration;
// use serde::{Deserialize, Serialize};
// use std::env;

const SENDER: Token = Token(0);
const _ECHOER: Token = Token(1);

pub struct Callbacks {}

impl Callbacks {
    fn _on_msg(_from: &SocketAddr, _msg: &UdpMsg, _len: usize) {}
}

fn create_socket(socket_address: SocketAddr, retries: usize) -> Option<UdpSocket> {
    let mut socket: Option<UdpSocket> = None;
    for port in (socket_address.port() as usize)..(socket_address.port() as usize) + retries + 1 {
        match UdpSocket::bind(SocketAddr::new(socket_address.ip(), port as u16)) {
            Ok(soc) => {
                info!("Udp bound to port: {}.\n", port);
                socket = Some(soc);
            }
            Err(error) => error!("Failed to bind to socket. {:?}", error),
        }
    }
    return socket;
}

pub struct Udp<'callbacks, 'poll> {
    // Network transmission information
    socket: Option<UdpSocket>,

    // state management
    callbacks: Option<&'callbacks mut Callbacks>,
    poll: Option<&'poll mut Poll>,
}

impl<'callbacks, 'poll> Default for Udp<'callbacks, 'poll> {
    fn default() -> Self {
        Udp {
            socket: None,
            callbacks: None,
            poll: None,
        }
    }
}

impl<'callbacks, 'poll> Udp<'callbacks, 'poll> {
    pub const fn new() -> Self {
        Udp {
            socket: None,
            callbacks: None,
            poll: None,
        }
    }
    pub fn init(&mut self, port: u16, poll: &'poll mut Poll, callbacks: &'callbacks mut Callbacks) {
        self.callbacks = Some(callbacks);
        self.poll = Some(poll);
        // ? unwrapping sketch time.
        // self.poll?
        //     .registry()
        //     .register(self.socket?, token, Interest::WRITABLE)
        self.socket = create_socket(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port),
            0,
        );
        info!("binding udp socket to port {}.\n", port);
        match self.poll.as_ref().unwrap().registry().register(
            self.socket.as_mut().unwrap(),
            SENDER,
            Interest::WRITABLE,
        ) {
            Ok(()) => (),
            Err(error) => error!("Error registering socket to poll registry: {}", error),
        }
        // self.socket.unwrap().connect(addr)
    }

    pub fn send_to(
        &mut self,
        _buffer: BytesMut,
        _len: usize,
        _flags: i32,
        destination: &[SocketAddr],
        _destlen: usize,
    ) {
        let _send_addrs: &[SocketAddr] = destination;
    }
    // void*
    fn _on_loop_poll(_cookie: i32) -> bool {
        true
    }
}
