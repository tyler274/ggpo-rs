pub enum GGPOErrorCode<T> {
    Ok(T),
    Success(T),
    GeneralFailure(T),
    InvalidSession(T),
    InvalidPlayerHandle(T),
    PlayerOutOfRange(T),
    PredictionThreshold(T),
    Unsupported(T),
    NotSynchronized(T),
    InRollback(T),
    InputDropped(T),
    PlayerDisconnected(T),
    TooManySpectators(T),
    InvalidRequest(T),
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

pub trait GGPOSessionCallbacks {
    fn begin_game() -> bool;
    fn save_game_state() -> bool;
    fn load_game_state() -> bool;
    fn free_buffer();
    fn advance_frame(flags: i32) -> bool;
    fn on_event(info: Event);
}

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
    fn do_poll(timeout: i32) -> GGPOErrorCode<()> {
        GGPOErrorCode::Ok(())
    }

    fn add_player(player: Player, handle: PlayerHandle) -> GGPOErrorCode<()>;

    fn add_local_input(player: PlayerHandle, values: String, size: i32) -> GGPOErrorCode<()>;

    fn sync_input(values: String, size: i32, disconnect_flags: i32) -> GGPOErrorCode<()>;

    fn increment_frame() -> GGPOErrorCode<()> {
        GGPOErrorCode::Ok(())
    }

    fn chat(text: String) -> GGPOErrorCode<()> {
        GGPOErrorCode::Ok(())
    }

    fn disconnect_player(handle: PlayerHandle) -> GGPOErrorCode<()> {
        GGPOErrorCode::Ok(())
    }

    fn get_network_stats(stats: GGPONetworkStats, handle: PlayerHandle) -> GGPOErrorCode<()> {
        GGPOErrorCode::Ok(())
    }

    //TODO: stub this with the log crate
    //fn logv()

    fn set_frame_delay(player: PlayerHandle, delay: i32) -> GGPOErrorCode<()> {
        GGPOErrorCode::Unsupported(())
    }

    fn set_disconnect_timeout(timeout: i32) -> GGPOErrorCode<()> {
        GGPOErrorCode::Unsupported(())
    }

    fn set_disconnect_notify_start(timeout: i32) -> GGPOErrorCode<()> {
        GGPOErrorCode::Unsupported(())
    }
}
