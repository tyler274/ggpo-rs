use crate::{
    game_input::Frame,
    ggpo::{GGPOError, GGPOSessionCallbacks, NetworkStats, Session},
    network::{
        udp::{Udp, UdpCallback},
        udp_msg::{ConnectStatus, UdpMsg, UDP_MSG_MAX_PLAYERS},
    },
    player::{Player, PlayerHandle},
    sync::{GGPOSync, SyncTrait},
};
// use mio::net::UdpSocket;
use async_net::UdpSocket;
// use mio::{Events, Interest, Poll, Token};
use std::net::{self, IpAddr, SocketAddr};
// use std::task::{Context, Poll};
use thiserror::Error;

const RECOMMENDATION_INTERVAL: i32 = 240;
const DEFAULT_DISCONNECT_TIMEOUT: i32 = 5000;
const DEFAULT_DISCONNECT_NOTIFY_START: i32 = 750;

#[derive(Debug, Error)]
pub enum Peer2PeerError {}
pub struct Peer2PeerBackend<'callbacks, 'network, GGPOCallbacks, Syncer>
where
    GGPOCallbacks: GGPOSessionCallbacks + Send + Sync,
    Syncer: SyncTrait<'callbacks, 'network, GGPOCallbacks>,
{
    callbacks: GGPOCallbacks,
    // _poll: Poll,
    _sync: Syncer,
    _udp: Udp<'callbacks, Peer2PeerBackend<'callbacks, 'network, GGPOCallbacks, Syncer>>,

    _num_spectators: i32,
    input_size: usize,

    _synchronizing: bool,
    num_players: i32,
    _next_recommended_sleep: i32,

    _next_spectator_frame: Frame,
    _disconnect_timeout: std::time::Duration,
    _disconnect_notify_start: std::time::SystemTime,

    _local_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],
}

impl<'callbacks, 'network, GGPOCallbacks, Syncer> UdpCallback
    for Peer2PeerBackend<'callbacks, 'network, GGPOCallbacks, Syncer>
where
    GGPOCallbacks: GGPOSessionCallbacks + Send + Sync,
    Syncer: SyncTrait<'callbacks, 'network, GGPOCallbacks>,
{
    fn on_msg(&self, from: &SocketAddr, msg: &UdpMsg, len: usize) {}
}

impl<'callbacks, 'network, Callbacks, Syncer> Session
    for Peer2PeerBackend<'callbacks, 'network, Callbacks, Syncer>
where
    Callbacks: GGPOSessionCallbacks + Send + Sync,
    Syncer: SyncTrait<'callbacks, 'network, Callbacks>,
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
    fn set_frame_delay(_player: PlayerHandle, _delay: i32) -> Result<(), GGPOError> {
        Err(GGPOError::Unsupported)
    }
    fn set_disconnect_timeout(_timeout: usize) -> Result<(), GGPOError> {
        Err(GGPOError::Unsupported)
    }
    fn set_disconnect_notify_start(_timeout: usize) -> Result<(), GGPOError> {
        Err(GGPOError::Unsupported)
    }
}

impl<'callbacks, 'network, Callbacks, Syncer>
    Peer2PeerBackend<'callbacks, 'network, Callbacks, Syncer>
where
    Callbacks: GGPOSessionCallbacks + Send + Sync,
    Syncer: SyncTrait<'callbacks, 'network, Callbacks>,
{
    pub fn new(
        callbacks: &Callbacks,
        game_name: &str,
        local_port: u16,
        num_players: i32,
        input_size: i32,
    ) -> Result<Self, String> {
        // let mut poll = Poll::new().map_err(|e| e.to_string())?;

        let udp = Udp::<Self>::new();
        // udp.init(local_port, &mut poll, self);
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
        Err("Failed to create a new p2p backend.".to_string())
    }

    // Take a player handle and return that player's input queue....I think.
    fn player_handle_to_queue(player: PlayerHandle, queue: &[u32]) -> Result<(), GGPOError> {
        unimplemented!()
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

    fn check_initial_sync() {}

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
}
