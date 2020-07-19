use crate::{
    game_input::Frame,
    ggpo::{
        GGPOError, GGPOSessionCallbacks, NetworkStats, Session, GGPO_MAX_PLAYERS,
        GGPO_MAX_SPECTATORS,
    },
    network::{
        udp::{Udp, UdpCallback, UdpError},
        udp_msg::{ConnectStatus, UdpMsg, UDP_MSG_MAX_PLAYERS},
        udp_proto::{UdpProtoError, UdpProtocol},
    },
    player::{Player, PlayerHandle},
    sync::GGPOSync,
};
use async_mutex::Mutex;
use async_net::UdpSocket;
use async_trait::async_trait;
use std::cell::RefCell;
use std::net::{self, IpAddr, SocketAddr};
use std::sync::Arc;
// use async_trait_ext::async_trait_ext;
use thiserror::Error;

const RECOMMENDATION_INTERVAL: i32 = 240;
const DEFAULT_DISCONNECT_TIMEOUT: i32 = 5000;
const DEFAULT_DISCONNECT_NOTIFY_START: i32 = 750;

#[derive(Debug, Error)]
pub enum Peer2PeerError {
    #[error("UDP protocol error.")]
    UdpProtocol {
        #[from]
        source: UdpProtoError,
    },
    #[error("UDP network error.")]
    Udp {
        #[from]
        source: UdpError,
    },
}

#[derive(Clone)]
pub struct Peer2PeerBackend<GGPOCallbacks>
where
    GGPOCallbacks: GGPOSessionCallbacks + Send + Sync + Clone,
{
    callbacks: Arc<Mutex<GGPOCallbacks>>,
    sync: GGPOSync<GGPOCallbacks>,
    udp: Arc<Mutex<Udp<Self>>>,
    endpoints: [Arc<Mutex<UdpProtocol<Self>>>; GGPO_MAX_PLAYERS],
    spectators: [Arc<Mutex<UdpProtocol<Self>>>; GGPO_MAX_SPECTATORS],
    num_spectators: usize,
    input_size: usize,

    synchronizing: bool,
    num_players: usize,
    _next_recommended_sleep: i32,

    _next_spectator_frame: Frame,
    disconnect_timeout: u128,
    disconnect_notify_start: u128,

    local_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],
}

#[async_trait(?Send)]
impl<GGPOCallbacks> UdpCallback for Peer2PeerBackend<GGPOCallbacks>
where
    GGPOCallbacks: GGPOSessionCallbacks + Send + Sync,
{
    async fn on_msg(&mut self, from: &SocketAddr, msg: &UdpMsg, len: usize) -> Result<(), String> {
        for i in 0..self.num_players {
            if self.endpoints[i]
                .lock()
                .await
                .handles_msg(from, msg)
                .map_err(|e| e.to_string())?
            {
                self.endpoints[i]
                    .lock()
                    .await
                    .on_msg(msg)
                    .await
                    .map_err(|e| e.to_string())?
            }
        }
        Ok(())
    }
}

#[async_trait(?Send)]
impl<T> Session for Peer2PeerBackend<T>
where
    T: GGPOSessionCallbacks + Send + Sync,
{
    fn do_poll(_timeout: usize) -> Result<(), GGPOError> {
        Ok(())
    }
    fn add_player(player: Player, handle: PlayerHandle) -> Result<(), GGPOError> {
        Ok(())
    }
    fn add_local_input(player: PlayerHandle, values: String, size: usize) -> Result<(), GGPOError> {
        Ok(())
    }
    fn synchronize_input(
        values: String,
        size: usize,
        disconnect_flags: i32,
    ) -> Result<(), GGPOError> {
        Ok(())
    }
    fn increment_frame() -> Result<(), GGPOError> {
        Ok(())
    }
    fn chat(_text: String) -> Result<(), GGPOError> {
        Ok(())
    }
    fn disconnect_player(_handle: PlayerHandle) -> Result<(), GGPOError> {
        Ok(())
    }
    fn get_network_stats(_stats: NetworkStats, _handle: PlayerHandle) -> Result<(), GGPOError> {
        Ok(())
    }
    fn logv(fmt: &str) -> Result<(), GGPOError> {
        Ok(())
    }
    fn set_frame_delay(&mut self, player: PlayerHandle, delay: i32) -> Result<(), GGPOError> {
        let queue = self.player_handle_to_queue(player)?;
        self.sync.set_frame_delay(queue as usize, delay as usize);
        Ok(())
    }
    async fn set_disconnect_timeout(&mut self, timeout: u128) -> Result<(), GGPOError> {
        self.disconnect_timeout = timeout;
        for i in 0..self.num_players {
            if self.endpoints[i].lock().await.is_initialized() {
                self.endpoints[i]
                    .lock()
                    .await
                    .set_disconnect_timeout(self.disconnect_timeout);
            }
        }
        Ok(())
    }
    async fn set_disconnect_notify_start(&mut self, timeout: u128) -> Result<(), GGPOError> {
        self.disconnect_notify_start = timeout;
        for i in 0..self.num_players {
            if self.endpoints[i].lock().await.is_initialized() {
                self.endpoints[i]
                    .lock()
                    .await
                    .set_disconnect_notify_start(self.disconnect_notify_start);
            }
        }
        Ok(())
    }
}

impl<T: GGPOSessionCallbacks + Send + Sync> Peer2PeerBackend<T> {
    pub fn new(
        callbacks: &T,
        game_name: &str,
        local_port: u16,
        num_players: i32,
        input_size: i32,
    ) -> Result<Self, Peer2PeerError> {
        let udp = Udp::<Self>::new();
        // udp.init(local_port, self);
        // let m = Self {
        //     num_players,
        //     input_size,
        //     _num_spectators: 0,
        //     _next_spectator_frame: Some(0),
        //     _next_recommended_sleep: 0,
        //     callbacks,
        //     _synchronizing: true,
        //     _poll: poll,
        //     _udp: udp,
        //     _disconnect_timeout: std::time::Duration::from_secs(DEFAULT_DISCONNECT_TIMEOUT),
        // };
        // Err("Failed to create a new p2p backend.".to_string())
        todo!()
    }

    pub async fn init(
        &mut self,
        p2p: Arc<Mutex<Peer2PeerBackend<T>>>,
        local_port: u16,
    ) -> Result<(), Peer2PeerError> {
        self.udp.lock().await.init(local_port, p2p).await?;
        Ok(())
    }

    // Take a player handle and return that player's input queue....I think.
    fn player_handle_to_queue(&self, player: PlayerHandle) -> Result<u32, GGPOError> {
        let offset = player - 1;
        if offset < 0 || offset >= self.num_players as u32 {
            return Err(GGPOError::InvalidPlayerHandle);
        }
        Ok(offset)
    }

    fn queue_to_player_handle(queue: u32) -> PlayerHandle {
        queue + 1
    }

    fn queue_to_spectator_handle(queue: u32) -> PlayerHandle {
        queue + 1000 /* out of range of the player array, basically */
    }

    fn disconnect_player_queues(queue: i32, sync_to: i32) {}

    fn poll_sync_events() {}

    fn poll_udp_protocol_events() {}

    fn poll_2_players(current_frame: Frame) -> i32 {
        unimplemented!()
    }

    fn poll_n_players(current_frame: Frame) -> i32 {
        unimplemented!()
    }

    fn add_remote_players(remote_ip: IpAddr, remote_port: u16, queue: u32) {}

    fn add_spectator(remote_ip: IpAddr, remote_port: u16) -> Result<(), GGPOError> {
        Ok(())
    }

    fn on_sync_event(event: &crate::sync::Event) {}

    fn on_udp_protocol_event(event: &crate::sync::Event, handle: PlayerHandle) {}

    fn on_udp_protocol_peer_event(event: &crate::sync::Event, queue: u32) {}

    fn on_udp_protocol_spectator_event(event: &crate::sync::Event, queue: u32) {}

    async fn check_initial_sync(&mut self) {
        if self.synchronizing {
            // Check to see if everyone is now synchronized.  If so,
            // go ahead and tell the client that we're ok to accept input.
            for i in 0..self.num_players {
                // xxx: IsInitialized() must go... we're actually using it as a proxy for "represents the local player"
                // TODO: How can we implement the above comments request....
                let endpoint = self.endpoints[i].lock().await;
                if endpoint.is_initialized()
                    && !endpoint.is_sychronized()
                    && !self.local_connect_status[i].disconnected
                {
                    return;
                }
            }
            for i in 0..self.num_spectators {
                let spectator = self.spectators[i].lock().await;
                if spectator.is_initialized() && !spectator.is_sychronized() {
                    return;
                }
            }
            let info = crate::ggpo::Event::Running;
        }
    }
}
