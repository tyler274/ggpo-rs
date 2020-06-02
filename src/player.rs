pub enum GGPOErrorCode {
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

type PlayerHandle = i32;

pub enum PlayerType {
    Local,
    Remote { socket_addr: std::net::SocketAddr },
    Spectator,
}

pub struct Player {
    size: usize,
    player_type: PlayerType,
    player_num: i32,
}

impl Player {
    pub fn new(player_type: PlayerType, player_num: i32) -> Player {
        Player {
            player_num: player_num,
            player_type: player_type,
            size: std::mem::size_of::<Player>(),
        }
    }
}

struct LocalEndpoint {
    player_num: i32,
}

pub enum GGPOEvent {
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

pub trait GGPOSessionCallbacks {
    // was deprecated anyway
    // fn begin_game() -> bool;

    /*
     * save_game_state - The client should allocate a buffer, copy the
     * entire contents of the current game state into it, and copy the
     * length into the *len parameter.  Optionally, the client can compute
     * a checksum of the data and store it in the *checksum argument.
     */
    fn save_game_state(buffer: &[u8], length: &usize, checksum: &usize, frame: usize) -> bool;

    /*
     * load_game_state - GGPO.net will call this function at the beginning
     * of a rollback.  The buffer and len parameters contain a previously
     * saved state returned from the save_game_state function.  The client
     * should make the current game state match the state contained in the
     * buffer.
     */
    fn load_game_state(buffer: &[u8], length: usize) -> bool;

    /*
     * log_game_state - Used in diagnostic testing.  The client should use
     * the ggpo_log function to write the contents of the specified save
     * state in a human readible form.
     */
    fn log_game_state(filename: String, buffer: &[u8], length: usize) -> bool;

    /*
     * free_buffer - Frees a game state allocated in save_game_state.  You
     * should deallocate the memory contained in the buffer.
     */
    fn free_buffer();

    /*
     * advance_frame - Called during a rollback.  You should advance your game
     * state by exactly one frame.  Before each frame, call ggpo_synchronize_input
     * to retrieve the inputs you should use for that frame.  After each frame,
     * you should call ggpo_advance_frame to notify GGPO.net that you're
     * finished.
     *
     * The flags parameter is reserved.  It can safely be ignored at this time.
     */
    fn advance_frame(flags: i32) -> bool;

    /*
     * on_event - Notification that something has happened.  See the GGPOEventCode
     * structure above for more information.
     */
    fn on_event(info: &GGPOEvent);
}

// pub struct GGPOSessionCallbacks {}

// impl GGPOSessionCallbacksTrait for GGPOSessionCallbacks {}

struct Network {
    send_queue_len: i32,
    recv_queue_len: i32,
    ping: i32,
    kbps_sent: i32,
}

struct Timesync {
    local_frames_behind: i32,
    remote_frames_behind: i32,
}

pub struct GGPONetworkStats {
    network: Network,
    timesync: Timesync,
}

pub trait Session {
    fn do_poll(timeout: i32) -> GGPOErrorCode {
        GGPOErrorCode::Ok
    }

    fn add_player(player: Player, handle: PlayerHandle) -> GGPOErrorCode;

    fn add_local_input(player: PlayerHandle, values: String, size: i32) -> GGPOErrorCode;

    fn sync_input(values: String, size: i32, disconnect_flags: i32) -> GGPOErrorCode;

    fn increment_frame() -> GGPOErrorCode {
        GGPOErrorCode::Ok
    }

    fn chat(text: String) -> GGPOErrorCode {
        GGPOErrorCode::Ok
    }

    fn disconnect_player(handle: PlayerHandle) -> GGPOErrorCode {
        GGPOErrorCode::Ok
    }

    fn get_network_stats(stats: GGPONetworkStats, handle: PlayerHandle) -> GGPOErrorCode {
        GGPOErrorCode::Ok
    }

    //TODO: stub this with the log crate
    //fn logv()

    fn set_frame_delay(player: PlayerHandle, delay: i32) -> GGPOErrorCode {
        GGPOErrorCode::Unsupported
    }

    fn set_disconnect_timeout(timeout: i32) -> GGPOErrorCode {
        GGPOErrorCode::Unsupported
    }

    fn set_disconnect_notify_start(timeout: i32) -> GGPOErrorCode {
        GGPOErrorCode::Unsupported
    }
}
