use crate::{
    game_input::{Frame, FrameNum, GameInput, InputBuffer},
    ggpo::{
        self, GGPOError, GGPOSessionCallbacks, NetworkStats, Session, GGPO_MAX_PLAYERS,
        GGPO_MAX_SPECTATORS,
    },
    network::{
        udp::{Udp, UdpCallback, UdpError},
        udp_msg::{ConnectStatus, UdpMsg, UDP_MSG_MAX_PLAYERS},
        udp_proto::{self, UdpProtoError, UdpProtocol},
    },
    player::{Player, PlayerHandle},
    sync::{self, GGPOSync},
};
use async_mutex::Mutex;
use async_net::UdpSocket;
use async_trait::async_trait;
use blocking::{block_on, unblock};
use log::{error, info, warn};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use thiserror::Error;

const RECOMMENDATION_INTERVAL: u32 = 240;
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
    #[error("Last frame is None.")]
    LastFrameNone,
    #[error("Next Spectator Frame is None.")]
    SpectatorFrameNone,
    #[error("current_remote_frame is None.")]
    CurrentRemoteFrameNone,
    #[error("GGPO Session error {0}")]
    GGPO(String),
}

#[derive(Clone)]
pub struct Peer2PeerBackend<T>
where
    T: GGPOSessionCallbacks + Send + Sync + Clone,
{
    callbacks: Arc<Mutex<T>>,
    sync: Arc<Mutex<GGPOSync<T>>>,
    udp: Arc<Mutex<Udp<Self>>>,
    endpoints: [Arc<Mutex<UdpProtocol<Self>>>; GGPO_MAX_PLAYERS],
    spectators: [Arc<Mutex<UdpProtocol<Self>>>; GGPO_MAX_SPECTATORS],
    num_spectators: usize,
    input_size: usize,

    synchronizing: Arc<Mutex<bool>>,
    num_players: usize,
    next_recommended_sleep: u32,

    next_spectator_frame: FrameNum,
    disconnect_timeout: u128,
    disconnect_notify_start: u128,

    local_connect_status: [Arc<Mutex<ConnectStatus>>; UDP_MSG_MAX_PLAYERS],
}

#[async_trait()]
impl<GGPOCallbacks> UdpCallback for Peer2PeerBackend<GGPOCallbacks>
where
    GGPOCallbacks: GGPOSessionCallbacks + Send + Sync,
{
    async fn on_msg(&mut self, from: &SocketAddr, msg: &UdpMsg, len: usize) -> Result<(), String> {
        for i in 0..self.num_players {
            let mut endpoint = self.endpoints[i].lock().await;
            if endpoint.handles_msg(from, msg).map_err(|e| e.to_string())? {
                return self.endpoints[i]
                    .lock()
                    .await
                    .on_msg(msg)
                    .await
                    .map_err(|e| e.to_string());
            }
        }
        for i in 0..self.num_spectators {
            let mut spectator = self.spectators[i].lock().await;
            if spectator
                .handles_msg(from, msg)
                .map_err(|e| e.to_string())?
            {
                return self.endpoints[i]
                    .lock()
                    .await
                    .on_msg(msg)
                    .await
                    .map_err(|e| e.to_string());
            }
        }

        Ok(())
    }
}

#[async_trait()]
impl<T> Session for Peer2PeerBackend<T>
where
    T: GGPOSessionCallbacks + Send + Sync,
{
    async fn do_poll(&mut self, timeout: usize) -> Result<(), GGPOError> {
        if self.sync.lock().await.in_rollback() {
            self.poll_udp_protocol_events().await?;
            if !*self.synchronizing.lock().await {
                self.sync.lock().await.check_simulation().await;

                // notify all of our endpoints of their local frame number for their
                // next connection quality report
                let current_frame = self.sync.lock().await.get_frame_count();
                for i in 0..self.num_players {
                    self.endpoints[i]
                        .lock()
                        .await
                        .set_local_frame_number(current_frame)
                }
                let total_min_confirmed;
                if self.num_players <= 2 {
                    total_min_confirmed = self.poll_2_players(current_frame).await?;
                } else {
                    total_min_confirmed = self.poll_n_players(current_frame).await?;
                }

                info!(
                    "last confirmed frame in p2p backend is {:?}.\n",
                    total_min_confirmed
                );

                assert!(total_min_confirmed != std::u32::MAX);
                if self.num_spectators > 0 {
                    while self.next_spectator_frame <= total_min_confirmed {
                        info!(
                            "pushing frame {:?} to spectators.\n",
                            self.next_spectator_frame
                        );

                        let mut input = crate::game_input::GameInput::new();
                        input.size = self.input_size * self.num_players;
                        input.frame = Some(self.next_spectator_frame);
                        self.sync
                            .lock()
                            .await
                            .get_confirmed_inputs(&mut input.bits, Some(self.next_spectator_frame));
                        for i in 0..self.num_spectators {
                            self.spectators[i].lock().await.send_input(&input).await?;
                        }
                        self.next_spectator_frame += 1;
                    }
                }

                info!(
                    "setting confirmed frame in sync to {:?}.\n",
                    total_min_confirmed
                );

                self.sync
                    .lock()
                    .await
                    .set_last_confirmed_frame(Some(total_min_confirmed));

                // send timesync notifications if now is the proper time
                if current_frame > self.next_recommended_sleep {
                    let mut interval = 0;
                    for i in 0..self.num_players {
                        interval = std::cmp::max(
                            interval,
                            self.endpoints[i].lock().await.recommend_frame_delay(),
                        );
                    }
                    if interval > 0 {
                        let info = ggpo::Event::TimeSync(ggpo::TimeSyncEvent {
                            frames_ahead: interval,
                        });
                        self.callbacks.lock().await.on_event(&info);
                        self.next_recommended_sleep = current_frame + RECOMMENDATION_INTERVAL;
                    }
                }
                // wat
                // XXX: this is obviously a farce...

                // if timeout > 0 {
                //     unblock!(std::thread::sleep(std::time::Duration::from_millis(1)));
                // }
            }
        }
        Ok(())
    }
    async fn add_player(
        &mut self,
        player: Player,
        handle: &mut PlayerHandle,
    ) -> Result<(), GGPOError> {
        if let crate::player::PlayerType::Spectator(remote_addr) = player.player_type {
            return self.add_spectator(remote_addr).await;
        }

        let queue = player.player_num as u32 - 1;

        if player.player_num < 1 || player.player_num > self.num_players {
            return Err(GGPOError::PlayerOutOfRange);
        }
        *handle = Self::queue_to_player_handle(queue);

        if let crate::player::PlayerType::Remote(remote_addr) = player.player_type {
            self.add_remote_player(remote_addr, queue).await?;
        }

        Ok(())
    }
    async fn add_local_input(
        &mut self,
        player: PlayerHandle,
        values: &InputBuffer,
        size: usize,
    ) -> Result<(), GGPOError> {
        if self.sync.lock().await.in_rollback() {
            return Err(GGPOError::InRollback);
        }
        if *self.synchronizing.lock().await {
            return Err(GGPOError::NotSynchronized);
        }

        let queue = self.player_handle_to_queue(player)?;

        let mut input = GameInput::init(None, Some(&values), size);

        // Feed the input for the current frame into the synchronzation layer.
        if !self.sync.lock().await.add_local_input(queue, &mut input) {
            return Err(GGPOError::PredictionThreshold);
        }

        if input.frame.is_some() {
            // This was still undone in the og code.

            // xxx: <- comment why this is the case
            // Update the local connect status state to indicate that we've got a
            // confirmed local frame for this player.  this must come first so it
            // gets incorporated into the next packet we send.
            info!(
                "setting local connect status for local queue {:?} to {:?}",
                queue, input.frame
            );
            self.local_connect_status[queue as usize]
                .lock()
                .await
                .last_frame = input.frame;

            // Send the input to all the remote players.
            for i in 0..self.num_players {
                let mut endpoint = self.endpoints[i].lock().await;
                if endpoint.is_initialized() {
                    endpoint.send_input(&input).await?;
                }
            }
        }

        Ok(())
    }
    async fn synchronize_input(
        &mut self,
        values: &mut Vec<InputBuffer>,
        disconnect_flags: Option<&mut i32>,
    ) -> Result<(), GGPOError> {
        // Wait until we've started to return inputs.
        if *self.synchronizing.lock().await {
            return Err(GGPOError::NotSynchronized);
        }
        let flags = self.sync.lock().await.synchronize_inputs(values);
        if let Some(d_flags) = disconnect_flags {
            *d_flags = flags;
        }
        Ok(())
    }
    async fn increment_frame(&mut self) -> Result<(), GGPOError> {
        {
            let mut sync = self.sync.lock().await;
            info!("End of frame ({:?})...\n", sync.get_frame_count());
            sync.increment_frame();
        }
        self.do_poll(0).await?;
        self.poll_sync_events().await?;
        Ok(())
    }
    fn chat(&mut self, _text: String) -> Result<(), GGPOError> {
        Ok(())
    }
    /*
     * Called only as the result of a local decision to disconnect. The remote
     * decisions to disconnect are a result of us parsing the peer_connect_settings
     * blob in every endpoint periodically.
     */
    async fn disconnect_player(&self, handle: PlayerHandle) -> Result<(), GGPOError> {
        let queue = self.player_handle_to_queue(handle)?;
        if self.local_connect_status[queue as usize]
            .lock()
            .await
            .disconnected
        {
            return Err(GGPOError::PlayerDisconnected);
        }

        if !self.endpoints[queue as usize].lock().await.is_initialized() {
            let current_frame = self.sync.lock().await.get_frame_count();
            // TODO: we should be tracking who the local player is, but for now assume
            // that if the endpoint is not initalized, this must be the local player.
            info!(
                "Disconnecting local player {:?} at frame {:?} by user request.\n",
                queue,
                self.local_connect_status[queue as usize]
                    .lock()
                    .await
                    .last_frame
            );
            for i in 0..self.num_players {
                if self.endpoints[i].lock().await.is_initialized() {
                    self.disconnect_player_queue(i as u32, current_frame)
                        .await?;
                }
            }
        } else {
            info!(
                "Disconnecting queue {:?} at frame {:?} by user request.\n",
                queue,
                self.local_connect_status[queue as usize]
                    .lock()
                    .await
                    .last_frame
            );
            {
                let connect_status = self.local_connect_status[queue as usize]
                    .lock()
                    .await
                    .last_frame
                    .ok_or(Peer2PeerError::LastFrameNone)?;
                self.disconnect_player_queue(queue, connect_status).await?;
            }
        }
        Ok(())
    }
    async fn get_network_stats(&self, handle: PlayerHandle) -> Result<NetworkStats, GGPOError> {
        let queue = self.player_handle_to_queue(handle)?;
        Ok(self.endpoints[queue as usize]
            .lock()
            .await
            .get_network_stats())
    }
    fn logv(fmt: &str) -> Result<(), GGPOError> {
        Ok(())
    }
    async fn set_frame_delay(&mut self, player: PlayerHandle, delay: i32) -> Result<(), GGPOError> {
        let queue = self.player_handle_to_queue(player)?;
        self.sync
            .lock()
            .await
            .set_frame_delay(queue as usize, delay as usize);
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
        // let m = Self {
        //     num_players,
        //     input_size,
        //     num_spectators: 0,
        //     next_spectator_frame: 0,
        //     next_recommended_sleep: 0,
        //     callbacks,
        //     synchronizing: true,
        //     udp: udp,
        //     disconnect_timeout: DEFAULT_DISCONNECT_TIMEOUT,
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

    async fn disconnect_player_queue(
        &self,
        queue: u32,
        sync_to: FrameNum,
    ) -> Result<(), Peer2PeerError> {
        let frame_count = self.sync.lock().await.get_frame_count();

        self.endpoints[queue as usize].lock().await.disconnect()?;
        {
            let mut local_connect_status = self.local_connect_status[queue as usize].lock().await;
            info!("Changing queue {:?} local connect status for last frame from {:?} to {:?} on disconnect request (current: {:?}).\n", queue, local_connect_status.last_frame, sync_to, frame_count);
            local_connect_status.disconnected = true;
            local_connect_status.last_frame = Some(sync_to);
        }

        if sync_to < frame_count {
            info!(
                "adjusting simulation to account for the fact that {:?} disconnected @ {:?}.\n",
                queue, sync_to
            );
            self.sync.lock().await.adjust_simulation(sync_to).await;
            info!("Finished adjusting simulation.\n");
        }

        let info = ggpo::Event::DisconnectedFromPeer(ggpo::DisconnectedFromPeer {
            player: Self::queue_to_player_handle(queue),
        });

        self.callbacks.lock().await.on_event(&info);

        self.check_initial_sync().await;

        Ok(())
    }

    async fn poll_sync_events(&mut self) -> Result<(), Peer2PeerError> {
        let mut event = crate::sync::Event::new();
        while self.sync.lock().await.get_event(&mut event) {
            self.on_sync_event(&event).await;
        }

        Ok(())
    }

    async fn poll_udp_protocol_events(&mut self) -> Result<(), Peer2PeerError> {
        let mut event = udp_proto::Event::Unknown;
        for i in 0..self.num_players {
            let mut endpoint = self.endpoints[i].lock().await;
            while endpoint.get_event(&mut event) {
                self.on_udp_protocol_peer_event(&mut event, i as u32)
                    .await?;
            }
        }
        for i in 0..self.num_spectators {
            let mut endpoint = self.endpoints[i].lock().await;
            while endpoint.get_event(&mut event) {
                self.on_udp_protocol_spectator_event(&mut event, i as u32)
                    .await?;
            }
        }
        Ok(())
    }

    async fn poll_2_players(&mut self, current_frame: FrameNum) -> Result<u32, Peer2PeerError> {
        //discard confirmed frames as appropriate
        let mut total_min_confirmed = std::u32::MAX;
        for i in 0..self.num_players {
            let mut queue_connected = true;
            // need to drop the lock here
            {
                let endpoint = self.endpoints[i].lock().await;
                if endpoint.is_running() {
                    let (_, connected) = endpoint.get_peer_connect_status(i);
                    queue_connected = connected;
                }
            }

            let local_connect_status = *self.local_connect_status[i].lock().await;
            if !local_connect_status.disconnected {
                total_min_confirmed = std::cmp::min(
                    local_connect_status
                        .last_frame
                        .ok_or(Peer2PeerError::LastFrameNone)?,
                    total_min_confirmed,
                )
            }
            info!(
                "local endp: connected = {:?}, last_received = {:?}, total_min_confirmed = {:?}.\n",
                !local_connect_status.disconnected,
                local_connect_status.last_frame,
                total_min_confirmed
            );
            if !queue_connected && !local_connect_status.disconnected {
                info!("disconnecting i {:?} by remote request.\n", i);
                self.disconnect_player_queue(i as u32, total_min_confirmed)
                    .await?;
            }

            info!("total_min_confirmed = {:?}.\n", total_min_confirmed);
        }
        Ok(total_min_confirmed)
    }

    async fn poll_n_players(&mut self, current_frame: FrameNum) -> Result<u32, Peer2PeerError> {
        // discard confirmed frames as appropriate
        let mut total_min_confirmed = std::u32::MAX;
        for queue in 0..self.num_players {
            let mut queue_connected = true;
            let mut queue_min_confirmed = std::u32::MAX;
            info!("considering queue {:?}.\n", queue);
            for i in 0..self.num_players {
                // we're going to do a lot of logic here in consideration of endpoint i.
                // keep accumulating the minimum confirmed point for all n*n packets and
                // throw away the rest.
                let endpoint = self.endpoints[i].lock().await;

                if endpoint.is_running() {
                    let (last_received, connected) = endpoint.get_peer_connect_status(queue);

                    queue_connected = queue_connected && connected;
                    queue_min_confirmed = std::cmp::min(
                        last_received.ok_or(Peer2PeerError::LastFrameNone)?,
                        queue_min_confirmed,
                    );
                    info!("endpoint {:?}: connected = {:?}, last_received = {:?}, queue_min_confirmed = {:?}.\n", i, connected, last_received, queue_min_confirmed);
                } else {
                    info!("endpoint {:?}: ignoring... not running.\n", i);
                }
            }
            let local_connect_status = *self.local_connect_status[queue].lock().await;
            // merge in our local status only if we're still connected!
            if !local_connect_status.disconnected {
                queue_min_confirmed = std::cmp::min(
                    local_connect_status
                        .last_frame
                        .ok_or(Peer2PeerError::LastFrameNone)?,
                    queue_min_confirmed,
                );
            }
            info!(
                "local endp: connected = {:?}, last_received = {:?}, queue_min_confirmed = {:?}.\n",
                !local_connect_status.disconnected,
                local_connect_status.last_frame,
                queue_min_confirmed
            );

            if queue_connected {
                total_min_confirmed = std::cmp::min(queue_min_confirmed, total_min_confirmed);
            } else {
                // check to see if this disconnect notification is further back than we've been before.  If
                // so, we need to re-adjust.  This can happen when we detect our own disconnect at frame n
                // and later receive a disconnect notification for frame n-1.
                if !local_connect_status.disconnected
                    || local_connect_status
                        .last_frame
                        .ok_or(Peer2PeerError::LastFrameNone)?
                        > queue_min_confirmed
                {
                    info!("disconnecting queue {:?} by remote request.\n", queue);
                    self.disconnect_player_queue(queue as u32, queue_min_confirmed)
                        .await?;
                }
            }
            info!("total_min_confirmed = {:?}.\n", total_min_confirmed);
        }
        Ok(total_min_confirmed)
    }

    async fn add_remote_player(
        &mut self,
        remote_addr: SocketAddr,
        queue: u32,
    ) -> Result<(), Peer2PeerError> {
        /*
         * Start the state machine (xxx: no)
         */
        // Tell me, what does the "no" meannnnnn
        {
            *self.synchronizing.lock().await = true;
        }
        let mut endpoint = self.endpoints[queue as usize].lock().await;
        endpoint.init(
            self.udp.clone(),
            queue,
            remote_addr,
            &self.local_connect_status,
        );
        endpoint.set_disconnect_timeout(self.disconnect_timeout);
        endpoint.set_disconnect_notify_start(self.disconnect_notify_start);
        Ok(endpoint.synchronize().await?)
    }

    async fn add_spectator(&mut self, remote_addr: SocketAddr) -> Result<(), GGPOError> {
        if self.num_spectators == GGPO_MAX_SPECTATORS {
            return Err(GGPOError::TooManySpectators);
        }
        /*
         * Currently, we can only add spectators before the game starts.
         */
        if !*self.synchronizing.lock().await {
            return Err(GGPOError::InvalidRequest);
        }
        let queue: u32 = self.num_spectators as u32;
        self.num_spectators += 1;

        let mut spectator = self.spectators[queue as usize].lock().await;
        spectator.init(
            self.udp.clone(),
            queue + 1000,
            remote_addr,
            &self.local_connect_status,
        );
        spectator.set_disconnect_timeout(self.disconnect_timeout);
        spectator.set_disconnect_notify_start(self.disconnect_notify_start);

        Ok(spectator.synchronize().await?)
    }

    // Is this supposed to do anything?
    async fn on_sync_event(&mut self, event: &sync::Event) {}

    async fn on_udp_protocol_event(&self, event: &udp_proto::Event, handle: PlayerHandle) {
        let info: ggpo::Event;
        match event {
            udp_proto::Event::Connected => {
                info = ggpo::Event::ConnectedToPeer(ggpo::ConnectedToPeer { player: handle });
                self.callbacks.lock().await.on_event(&info);
            }
            udp_proto::Event::Synchronizing(sync) => {
                info = ggpo::Event::SynchronizingWithPeer(ggpo::SynchronizingWithPeer {
                    count: sync.count,
                    total: sync.total,
                    player: handle,
                });
                self.callbacks.lock().await.on_event(&info);
            }
            udp_proto::Event::Synchronzied => {
                info = ggpo::Event::SynchronizedWithPeer(ggpo::SynchronizedWithPeer {
                    player: handle,
                });
                self.callbacks.lock().await.on_event(&info);
                self.check_initial_sync().await;
            }

            udp_proto::Event::NetworkInterrupted(net_interupt) => {
                info = ggpo::Event::ConnectionInterrupted(ggpo::ConnectionInterrupted {
                    player: handle,
                    disconnect_timeout: net_interupt.disconnect_timeout,
                });
                self.callbacks.lock().await.on_event(&info);
            }
            udp_proto::Event::NetworkResumed => {
                info = ggpo::Event::ConnectionResumed(ggpo::ConnectionResumed { player: handle });
                self.callbacks.lock().await.on_event(&info);
            }
            _ => {}
        }
    }

    async fn on_udp_protocol_peer_event(
        &self,
        event: &udp_proto::Event,
        queue: u32,
    ) -> Result<(), Peer2PeerError> {
        self.on_udp_protocol_event(event, Self::queue_to_player_handle(queue))
            .await;

        match event {
            udp_proto::Event::Input(input) => {
                let local_connect_status = *self.local_connect_status[queue as usize].lock().await;
                if !local_connect_status.disconnected {
                    let current_remote_frame = local_connect_status.last_frame;
                    let new_remote_frame = input.frame;
                    // ASSERT(current_remote_frame == -1 || new_remote_frame == (current_remote_frame + 1));
                    assert!(
                        current_remote_frame.is_none()
                            || new_remote_frame
                                == Some(
                                    current_remote_frame
                                        .ok_or(Peer2PeerError::CurrentRemoteFrameNone)?
                                        + 1
                                )
                    );

                    self.sync.lock().await.add_remote_input(queue, input);
                    // Notify the other endpoints which frame we received from a peer
                    info!(
                        "Setting remote connect status for queue {:?} to {:?}.\n",
                        queue, input.frame
                    );
                    self.local_connect_status[queue as usize]
                        .lock()
                        .await
                        .last_frame = input.frame;
                }
            }
            udp_proto::Event::Disconnected => {
                self.disconnect_player(Self::queue_to_player_handle(queue))
                    .await
                    .map_err(|e| Peer2PeerError::GGPO(e.to_string()))?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn on_udp_protocol_spectator_event(
        &self,
        event: &udp_proto::Event,
        queue: u32,
    ) -> Result<(), Peer2PeerError> {
        let handle = Self::queue_to_player_handle(queue);
        self.on_udp_protocol_event(event, handle).await;

        let info: ggpo::Event;
        match event {
            udp_proto::Event::Disconnected => {
                self.spectators[queue as usize].lock().await.disconnect()?;
                info = ggpo::Event::DisconnectedFromPeer(ggpo::DisconnectedFromPeer {
                    player: handle,
                });
                self.callbacks.lock().await.on_event(&info);
            }
            _ => {}
        }
        Ok(())
    }

    async fn check_initial_sync(&self) {
        if *self.synchronizing.lock().await {
            // Check to see if everyone is now synchronized.  If so,
            // go ahead and tell the client that we're ok to accept input.
            for i in 0..self.num_players {
                // xxx: IsInitialized() must go... we're actually using it as a proxy for "represents the local player"
                // TODO: How can we implement the above comments request....
                let endpoint = self.endpoints[i].lock().await;
                if endpoint.is_initialized()
                    && !endpoint.is_sychronized()
                    && !self.local_connect_status[i].lock().await.disconnected
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
            {
                self.callbacks.lock().await.on_event(&info);
                *self.synchronizing.lock().await = false;
            }
        }
    }
}
