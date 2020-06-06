use crate::player::{Player, PlayerHandle};
pub enum ErrorCode {
    Ok,
    Success,
    GeneralFailure,
    InvalidSession,
    InvalidPlayerHandle,
    PlayerOutOfRange,
    PredictionThreshold,
    Unsupported,
    NotSynchronized,
    InRollback,
    InputDropped,
    PlayerDisconnected,
    TooManySpectators,
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
    fn do_poll(timeout: usize) -> ErrorCode {
        ErrorCode::Ok
    }

    fn add_player(player: Player, handle: PlayerHandle) -> ErrorCode;

    fn add_local_input(player: PlayerHandle, values: String, size: usize) -> ErrorCode;

    fn sync_input(values: String, size: usize, disconnect_flags: i32) -> ErrorCode;

    fn increment_frame() -> ErrorCode {
        ErrorCode::Ok
    }

    fn chat(text: String) -> ErrorCode {
        ErrorCode::Ok
    }

    fn disconnect_player(handle: PlayerHandle) -> ErrorCode {
        ErrorCode::Ok
    }

    fn get_network_stats(stats: NetworkStats, handle: PlayerHandle) -> ErrorCode {
        ErrorCode::Ok
    }

    //TODO: stub this with the log crate
    //fn logv()

    fn set_frame_delay(player: PlayerHandle, delay: i32) -> ErrorCode {
        ErrorCode::Unsupported
    }

    fn set_disconnect_timeout(timeout: usize) -> ErrorCode {
        ErrorCode::Unsupported
    }

    fn set_disconnect_notify_start(timeout: usize) -> ErrorCode {
        ErrorCode::Unsupported
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
        buffer: Option<&mut [u8]>,
        length: &usize,
        checksum: Option<usize>,
        frame: Option<usize>,
    ) -> bool;

    /*
     * load_game_state - GGPO.net will call this function at the beginning
     * of a rollback.  The buffer and len parameters contain a previously
     * saved state returned from the save_game_state function.  The client
     * should make the current game state match the state contained in the
     * buffer.
     */
    fn load_game_state(&mut self, buffer: &[u8], length: usize) -> bool;

    /*
     * log_game_state - Used in diagnostic testing.  The client should use
     * the ggpo_log function to write the contents of the specified save
     * state in a human readible form.
     */
    fn log_game_state(&mut self, filename: String, buffer: &[u8], length: usize) -> bool;

    /*
     * free_buffer - Frees a game state allocated in save_game_state.  You
     * should deallocate the memory contained in the buffer.
     */
    fn free_buffer(&mut self, buffer: &[u8]);

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
        buffer: Option<&[u8]>,
        length: usize,
        checksum: Option<usize>,
        frame: Option<usize>,
    ) -> bool,

    /*
     * load_game_state - GGPO.net will call this function at the beginning
     * of a rollback.  The buffer and len parameters contain a previously
     * saved state returned from the save_game_state function.  The client
     * should make the current game state match the state contained in the
     * buffer.
     */
    pub load_game_state: extern "C" fn(buffer: &[u8], length: usize) -> bool,

    /*
     * log_game_state - Used in diagnostic testing.  The client should use
     * the ggpo_log function to write the contents of the specified save
     * state in a human readible form.
     */
    pub log_game_state: extern "C" fn(filename: String, buffer: &[u8], length: usize) -> bool,

    /*
     * free_buffer - Frees a game state allocated in save_game_state.  You
     * should deallocate the memory contained in the buffer.
     */
    pub free_buffer: extern "C" fn(buffer: &[u8]),

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
    send_queue_len: usize,
    recv_queue_len: usize,
    ping: usize,
    kbps_sent: usize,
}

struct Timesync {
    local_frames_behind: i32,
    remote_frames_behind: i32,
}

pub struct NetworkStats {
    network: Network,
    timesync: Timesync,
}

struct LocalEndpoint {
    player_num: usize,
}
