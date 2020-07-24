use crate::network::udp_msg::UdpMsg;

// use async_mutex::Mutex;
// use async_net::UdpSocket;
// use async_trait::async_trait;
// use blocking::unblock;
use bytes::BytesMut;
use log::{error, info};
use mio::{net::UdpSocket, Events, Interest, Poll, Token};
use parking_lot::Mutex;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Deref,
    sync::Arc,
};

use thiserror::Error;

pub const ZSTD_LEVEL: i32 = 7;

// #[async_trait(?Send)]
// #[async_trait()]
pub trait UdpCallback {
    fn on_msg(&mut self, from: &SocketAddr, msg: &UdpMsg, len: usize) -> Result<(), String>;
}

#[derive(Debug, Error)]
pub enum UdpError {
    #[error("Socket unbound/unitialized.")]
    SocketUninit,
    #[error("Session callbacks uninitialized.")]
    CallbacksUninit,
    #[error("IO Error")]
    Io {
        #[from]
        source: std::io::Error,
    },
    #[error("Bincode (de)serialization Error")]
    Bincode {
        #[from]
        source: bincode::Error,
    },
    #[error("Callback error {0}")]
    Callback(String),
}

fn create_socket(socket_address: SocketAddr, retries: usize) -> std::io::Result<UdpSocket> {
    for port in (socket_address.port() as usize)..(socket_address.port() as usize) + retries + 1 {
        match UdpSocket::bind(SocketAddr::new(socket_address.ip(), port as u16)) {
            Ok(soc) => {
                info!("Udp bound to port: {}.\n", port);
                return Ok(soc);
            }
            Err(error) => {
                error!("Failed to bind to socket. {:?}", error);
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!(
            "failed to bind socket after {} successive retries.",
            retries
        ),
    ))
}

pub struct Udp<T: UdpCallback> {
    // Network transmission information
    socket: Option<UdpSocket>,

    // state management
    callbacks: Option<Arc<Mutex<T>>>,

    poll: Option<Arc<Mutex<Poll>>>,
}

impl<T: UdpCallback> Default for Udp<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: UdpCallback> Udp<T> {
    pub fn new() -> Self {
        let u = Udp {
            socket: None,
            callbacks: None,
            poll: None,
        };

        return u;
    }
    pub fn init(
        &mut self,
        port: u16,
        poll: Arc<Mutex<Poll>>,
        callbacks: Arc<Mutex<T>>,
    ) -> Result<(), UdpError> {
        self.callbacks = Some(callbacks);
        info!("binding udp socket to port {}.\n", port);
        let mut socket = create_socket(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port),
            3,
        )?;
        // TODO: _poll->RegisterLoop(this);
        poll.lock().registry().register(
            &mut socket,
            Token(0),
            Interest::READABLE | Interest::WRITABLE,
        )?;

        self.socket = Some(socket);
        self.poll = Some(poll.clone());

        Ok(())
    }

    pub fn send_to(&mut self, msg: Arc<UdpMsg>, destination: &SocketAddr) -> Result<(), UdpError> {
        /*
        TODO: Can we store the serialized result into a BytesMut/buffer and be compressed in place to avoid another allocation?
        TODO: Worthwhile to spawn here?
        TODO: Will doing the above actually improve performance?
         */
        let serialized = bincode::serialize(msg.clone().deref())?;
        let compressed = zstd::block::compress(&serialized, ZSTD_LEVEL)?;

        let resp = self
            .socket
            .as_ref()
            .ok_or(UdpError::SocketUninit)?
            .send_to(&compressed, *destination)?;

        info!(
            "sent packet length {} to {}:{} (resp:{}).\n",
            compressed.len(),
            destination.ip(),
            destination.port(),
            resp
        );
        Ok(())
    }

    pub fn get_msg(&mut self) -> Result<(UdpMsg, usize, SocketAddr), UdpError> {
        let mut recv_buf = BytesMut::new();
        let (len, recv_address) = self
            .socket
            .as_ref()
            .ok_or(UdpError::SocketUninit)?
            .recv_from(recv_buf.as_mut())?;

        let decompressed =
            zstd::block::decompress(recv_buf.as_mut(), std::mem::size_of::<UdpMsg>())?;

        let msg: UdpMsg = bincode::deserialize(&decompressed)?;
        Ok((msg, len, recv_address))
    }

    pub fn on_loop_poll(&mut self, _cookie: i32) -> Result<bool, UdpError> {
        let (msg, len, recv_address) = self.get_msg()?;

        self.callbacks
            .as_mut()
            .ok_or(UdpError::CallbacksUninit)?
            .lock()
            .on_msg(&recv_address, &msg, len)
            .map_err(|e| UdpError::Callback(e))?;
        Ok(true)
    }
}
