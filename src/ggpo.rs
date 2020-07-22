use crate::{
    backends::p2p::Peer2PeerError,
    game_input::{Frame, FrameNum, InputBuffer},
    network::udp_proto::UdpProtoError,
    player::{Player, PlayerHandle},
    sync::SyncError,
};
use async_trait::async_trait;
use bytes::Bytes;
// use log::info;
use thiserror::Error;

pub const GGPO_MAX_PLAYERS: usize = 4;
pub const GGPO_MAX_SPECTATORS: usize = 32;
pub const GGPO_MAX_PREDICTION_FRAMES: FrameNum = 8;

#[derive(Error, Debug)]
pub enum GGPOError {
    #[error("GGPO OK.")]
    Ok,
    #[error("GGPO Success.")]
    Success,
    #[error("GGPO general Failure.")]
    GeneralFailure,
    #[error("GGPO invalid session.")]
    InvalidSession,
    #[error("GGPO invalid player handle.")]
    InvalidPlayerHandle,
    #[error("GGPO player out of range.")]
    PlayerOutOfRange,
    #[error("GGPO prediction threshold.")]
    PredictionThreshold,
    #[error("GGPO unsupported.")]
    Unsupported,
    #[error("GGPO not synchronized.")]
    NotSynchronized,
    #[error("GGPO in rollback.")]
    InRollback,
    #[error("GGPO input dropped.")]
    InputDropped,
    #[error("GGPO player disconnected.")]
    PlayerDisconnected,
    #[error("GGPO too many spectators.")]
    TooManySpectators,
    #[error("GGPO invalid request.")]
    InvalidRequest,
    #[error("P2P Backend error.")]
    P2P {
        #[from]
        source: Peer2PeerError,
    },
    #[error("UDP Protocol error.")]
    UdpProto {
        #[from]
        source: UdpProtoError,
    },
    #[error("Synchronization engine error.")]
    Sync {
        #[from]
        source: SyncError,
    },
}
pub struct ConnectedToPeer {
    pub player: PlayerHandle,
}

pub struct SynchronizingWithPeer {
    pub count: u32,
    pub total: u32,
    pub player: PlayerHandle,
}

pub struct SynchronizedWithPeer {
    pub player: PlayerHandle,
}

pub struct DisconnectedFromPeer {
    pub player: PlayerHandle,
}

pub struct TimeSyncEvent {
    pub frames_ahead: FrameNum,
}

pub struct ConnectionInterrupted {
    pub player: PlayerHandle,
    pub disconnect_timeout: u128,
}

pub struct ConnectionResumed {
    pub player: PlayerHandle,
}

pub enum Event {
    ConnectedToPeer(ConnectedToPeer),
    SynchronizingWithPeer(SynchronizingWithPeer),
    SynchronizedWithPeer(SynchronizedWithPeer),
    Running,
    DisconnectedFromPeer(DisconnectedFromPeer),
    TimeSync(TimeSyncEvent),
    ConnectionInterrupted(ConnectionInterrupted),
    ConnectionResumed(ConnectionResumed),
}

#[async_trait()]
pub trait Session {
    async fn do_poll(&mut self, _timeout: usize) -> Result<(), GGPOError> {
        unimplemented!()
    }

    async fn add_player(
        &mut self,
        _player: Player,
        _handle: &mut PlayerHandle,
    ) -> Result<(), GGPOError> {
        unimplemented!()
    }

    async fn add_local_input(
        &mut self,
        _player: PlayerHandle,
        _values: &InputBuffer,
        _size: usize,
    ) -> Result<(), GGPOError> {
        unimplemented!()
    }

    async fn synchronize_input(
        &mut self,
        _values: &mut Vec<InputBuffer>,
        _disconnect_flags: Option<&mut i32>,
    ) -> Result<(), GGPOError> {
        unimplemented!()
    }

    async fn increment_frame(&mut self) -> Result<(), GGPOError> {
        unimplemented!()
    }

    fn chat(&mut self, _text: String) -> Result<(), GGPOError> {
        unimplemented!()
    }

    async fn disconnect_player(&self, _handle: PlayerHandle) -> Result<(), GGPOError> {
        unimplemented!()
    }

    async fn get_network_stats(&self, _handle: PlayerHandle) -> Result<NetworkStats, GGPOError> {
        unimplemented!()
    }

    //TODO: stub this with the log crate
    fn logv(_fmt: &str) -> Result<(), GGPOError> {
        unimplemented!()
    }

    async fn set_frame_delay(
        &mut self,
        _player: PlayerHandle,
        _delay: i32,
    ) -> Result<(), GGPOError> {
        Err(GGPOError::Unsupported)
    }

    async fn set_disconnect_timeout(&mut self, _timeout: u128) -> Result<(), GGPOError> {
        Err(GGPOError::Unsupported)
    }

    async fn set_disconnect_notify_start(&mut self, _timeout: u128) -> Result<(), GGPOError> {
        Err(GGPOError::Unsupported)
    }
}

pub trait GGPOSessionCallbacks: Clone + Sized {
    // was deprecated anyway
    // fn begin_game() -> bool;

    /*
     * save_game_state - The client should allocate a buffer, copy the
     * entire contents of the current game state into it, and copy the
     * length into the *len parameter.  Optionally, the client can compute
     * a checksum of the data and store it in the *checksum argument.
     */
    fn save_game_state(
        &mut self,
        buffer: &Bytes,
        length: &usize,
        checksum: Option<u32>,
        frame: Frame,
    ) -> bool;

    /*
     * load_game_state - GGPO.net will call this function at the beginning
     * of a rollback.  The buffer and len parameters contain a previously
     * saved state returned from the save_game_state function.  The client
     * should make the current game state match the state contained in the
     * buffer.
     */
    fn load_game_state(&mut self, buffer: &Bytes, length: usize) -> bool;

    /*
     * log_game_state - Used in diagnostic testing.  The client should use
     * the ggpo_log function to write the contents of the specified save
     * state in a human readible form.
     */
    fn log_game_state(&mut self, filename: String, buffer: Bytes, length: usize) -> bool;

    /*
     * free_buffer - Frees a game state allocated in save_game_state.  You
     * should deallocate the memory contained in the buffer.
     */
    fn free_buffer(&mut self, buffer: &Bytes);

    /*
     * advance_frame - Called during a rollback.  You should advance your game
     * state by exactly one frame.  Before each frame, call ggpo_synchronize_input
     * to retrieve the inputs you should use for that frame.  After each frame,
     * you should call ggpo_advance_frame to notify GGPO.net that you're
     * finished.
     *
     * The flags parameter is reserved.  It can safely be ignored at this time.
     */
    fn advance_frame(&mut self, flags: i32) -> bool;

    /*
     * on_event - Notification that something has happened.  See the GGPOEventCode
     * structure above for more information.
     */
    fn on_event(&mut self, info: &Event);
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Network {
    pub send_queue_len: usize,
    pub recv_queue_len: usize,
    pub ping: usize,
    pub kbps_sent: usize,
}

impl Network {
    pub const fn new() -> Self {
        Self {
            send_queue_len: 0,
            recv_queue_len: 0,
            ping: 0,
            kbps_sent: 0,
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct TimeSync {
    pub local_frames_behind: i32,
    pub remote_frames_behind: i32,
}

impl TimeSync {
    pub const fn new() -> Self {
        Self {
            local_frames_behind: 0,
            remote_frames_behind: 0,
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct NetworkStats {
    pub network: Network,
    pub timesync: TimeSync,
}

impl NetworkStats {
    pub const fn new() -> Self {
        Self {
            network: Network::new(),
            timesync: TimeSync::new(),
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
struct LocalEndpoint {
    player_num: usize,
}
