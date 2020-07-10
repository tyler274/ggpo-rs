use crate::network::udp_msg::UdpMsg;

use async_net::UdpSocket;
use bytes::{Bytes, BytesMut};
use log::{error, info};
use smol::Async;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use thiserror::Error;

pub trait UdpCallback {
    fn on_msg(&self, _from: &SocketAddr, _msg: &UdpMsg, _len: usize) {}
}

#[derive(Debug, Error)]
pub enum UdpError {
    #[error("Socket unbound/unitialized.")]
    SocketUninit,
    #[error("Session callbacks uninitialized.")]
    CallbacksUninit,
}

async fn create_socket(socket_address: SocketAddr, retries: usize) -> std::io::Result<UdpSocket> {
    for port in (socket_address.port() as usize)..(socket_address.port() as usize) + retries + 1 {
        match UdpSocket::bind(SocketAddr::new(socket_address.ip(), port as u16)).await {
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

pub struct Udp<'callbacks, Callbacks>
where
    Callbacks: UdpCallback,
{
    // Network transmission information
    socket: Option<UdpSocket>,

    // state management
    callbacks: Option<&'callbacks mut Callbacks>,
    // poll: Option<&'poll mut Poll>,
}

impl<'callbacks, Callbacks> Default for Udp<'callbacks, Callbacks>
where
    Callbacks: UdpCallback,
{
    fn default() -> Self {
        Udp {
            socket: None,
            callbacks: None,
            // poll: None,
        }
    }
}

impl<'callbacks, Callbacks> Udp<'callbacks, Callbacks>
where
    Callbacks: UdpCallback,
{
    pub const fn new() -> Self {
        Udp {
            socket: None,
            callbacks: None,
            // poll: None,
        }
    }
    pub async fn init(
        &mut self,
        port: u16,
        // poll: &'poll mut Poll,
        callbacks: &'callbacks mut Callbacks,
    ) -> Result<(), String> {
        self.callbacks = Some(callbacks);
        // self.poll = Some(poll);
        // ? unwrapping sketch time.
        // self.poll?
        //     .registry()
        //     .register(self.socket?, token, Interest::WRITABLE)
        info!("binding udp socket to port {}.\n", port);
        self.socket = Some(
            create_socket(
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port),
                3,
            )
            .await
            .map_err(|e| e.to_string())?,
        );
        // self.poll
        //     .as_ref()
        //     .ok_or("Poll not init'd")?
        //     .registry()
        //     .register(
        //         self.socket.as_mut().ok_or("Socket not init'd")?,
        //         SENDER,
        //         Interest::WRITABLE,
        //     )
        //     .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn send_to(
        &mut self,
        // mut buffer: Bytes,
        msg: &[UdpMsg],
        // _len: usize,
        // _flags: i32,
        destination: &[SocketAddr],
        // _destlen: usize,
    ) -> Result<(), String> {
        let serialized = bincode::serialize(msg).map_err(|e| e.to_string())?;
        let resp = self
            .socket
            .as_ref()
            .ok_or(UdpError::SocketUninit)
            .map_err(|e| e.to_string())?
            .send_to(&serialized, destination)
            .await
            .map_err(|e| e.to_string())?;

        let peer_addr = self
            .socket
            .as_ref()
            .ok_or(UdpError::SocketUninit)
            .map_err(|e| e.to_string())?
            .peer_addr()
            .map_err(|e| e.to_string())?;
        info!(
            "sent packet length {} to {}:{} (resp:{}).\n",
            serialized.len(),
            peer_addr.ip(),
            peer_addr.port(),
            resp
        );
        Ok(())
    }
    // void*
    pub async fn on_loop_poll(&self, _cookie: i32) -> Result<bool, String> {
        let mut recv_buf = BytesMut::new();
        let (len, recv_address) = self
            .socket
            .as_ref()
            .ok_or(UdpError::SocketUninit)
            .map_err(|e| e.to_string())?
            .recv_from(recv_buf.as_mut())
            .await
            .map_err(|e| e.to_string())?;

        let msg: UdpMsg = bincode::deserialize(recv_buf.as_mut()).map_err(|e| e.to_string())?;
        self.callbacks
            .as_ref()
            .ok_or(UdpError::CallbacksUninit)
            .map_err(|e| e.to_string())?
            .on_msg(&recv_address, &msg, len);
        Ok(true)
    }
}
