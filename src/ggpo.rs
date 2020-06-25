use crate::game_input::Frame;
use crate::player::{Player, PlayerHandle};
use bytes::{Bytes, BytesMut};
use thiserror::Error;

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
}

pub enum Event {
    ConnectedToPeer {
        player: PlayerHandle,
    },
    SynchronizingWithPeer {
        count: i32,
        total: i32,
    },
    SynchronizedWithPeer {
        player: PlayerHandle,
    },
    Running {},
    DisconnectedFromPeer {
        player: PlayerHandle,
    },
    Timesync {
        frames_ahead: i32,
    },
    ConnectionInterrupted {
        player: PlayerHandle,
        disconnect_timeout: i32,
    },
    ConnectionResumed {
        player: PlayerHandle,
    },
}

pub trait Session {
    fn do_poll(_timeout: usize) -> GGPOError {
        GGPOError::Ok
    }

    fn add_player(player: Player, handle: PlayerHandle) -> GGPOError;

    fn add_local_input(player: PlayerHandle, values: String, size: usize) -> GGPOError;

    fn sync_input(values: String, size: usize, disconnect_flags: i32) -> GGPOError;

    fn increment_frame() -> GGPOError {
        GGPOError::Ok
    }

    fn chat(_text: String) -> GGPOError {
        GGPOError::Ok
    }

    fn disconnect_player(_handle: PlayerHandle) -> GGPOError {
        GGPOError::Ok
    }

    fn get_network_stats(_stats: NetworkStats, _handle: PlayerHandle) -> GGPOError {
        GGPOError::Ok
    }

    //TODO: stub this with the log crate
    //fn logv()

    fn set_frame_delay(_player: PlayerHandle, _delay: i32) -> GGPOError {
        GGPOError::Unsupported
    }

    fn set_disconnect_timeout(_timeout: usize) -> GGPOError {
        GGPOError::Unsupported
    }

    fn set_disconnect_notify_start(_timeout: usize) -> GGPOError {
        GGPOError::Unsupported
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
#[no_mangle]
pub struct CallbacksStub {
    /*
     * save_game_state - The client should allocate a buffer, copy the
     * entire contents of the current game state into it, and copy the
     * length into the *len parameter.  Optionally, the client can compute
     * a checksum of the data and store it in the *checksum argument.
     */
    pub save_game_state: extern "C" fn(
        buffer: Option<BytesMut>,
        length: usize,
        checksum: Option<u32>,
        frame: Frame,
    ) -> bool,

    /*
     * load_game_state - GGPO.net will call this function at the beginning
     * of a rollback.  The buffer and len parameters contain a previously
     * saved state returned from the save_game_state function.  The client
     * should make the current game state match the state contained in the
     * buffer.
     */
    pub load_game_state: extern "C" fn(buffer: BytesMut, length: usize) -> bool,

    /*
     * log_game_state - Used in diagnostic testing.  The client should use
     * the ggpo_log function to write the contents of the specified save
     * state in a human readible form.
     */
    pub log_game_state: extern "C" fn(filename: String, buffer: BytesMut, length: usize) -> bool,

    /*
     * free_buffer - Frees a game state allocated in save_game_state.  You
     * should deallocate the memory contained in the buffer.
     */
    pub free_buffer: extern "C" fn(buffer: BytesMut),

    /*
     * advance_frame - Called during a rollback.  You should advance your game
     * state by exactly one frame.  Before each frame, call ggpo_synchronize_input
     * to retrieve the inputs you should use for that frame.  After each frame,
     * you should call ggpo_advance_frame to notify GGPO.net that you're
     * finished.
     *
     * The flags parameter is reserved.  It can safely be ignored at this time.
     */
    pub advance_frame: extern "C" fn(flags: i32) -> bool,

    /*
     * on_event - Notification that something has happened.  See the GGPOEventCode
     * structure above for more information.
     */
    pub on_event: extern "C" fn(info: &Event),
}

struct Network {
    _send_queue_len: usize,
    _recv_queue_len: usize,
    _ping: usize,
    _kbps_sent: usize,
}

struct Timesync {
    _local_frames_behind: i32,
    _remote_frames_behind: i32,
}

pub struct NetworkStats {
    _network: Network,
    _timesync: Timesync,
}

struct _LocalEndpoint {
    player_num: usize,
}
