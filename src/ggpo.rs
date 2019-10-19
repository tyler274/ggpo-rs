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

type GGPOPlayerHandle = i32;

enum GGPOPlayerType {
    GgpoPlayertypeLocal,
    GgpoPlayertypeRemote,
    GgpoPlayertypeSpectator,
}

#[derive(Copy, Clone)]
struct Nil;

union Socket {
    local: Nil,
    remote: std::net::SocketAddr,
}

pub struct GGPOPlayer {
    size: i32,
    player_type: GGPOPlayerType,
    player_num: i32,
    socket: Socket,
}

struct GGPOLocalEndpoint {
    player_num: i32,
}

pub enum GGPOEvent {
    GgpoEventcodeConnectedToPeer {
        player: GGPOPlayerHandle,
    },
    GgpoEventcodeSynchronizingWithPeer {
        count: i32,
        total: i32,
    },
    GgpoEventcodeSynchronizedWithPeer {
        player: GGPOPlayerHandle,
    },
    GgpoEventcodeRunning {},
    GgpoEventcodeDisconnectedFromPeer {
        player: GGPOPlayerHandle,
    },
    GgpoEventcodeTimesync {
        frames_ahead: i32,
    },
    GgpoEventcodeConnectionInterrupted {
        player: GGPOPlayerHandle,
        disconnect_timeout: i32,
    },
    GgpoEventcodeConnectionResumed {
        player: GGPOPlayerHandle,
    },
}

pub trait GGPOSessionCallbacks {
    fn begin_game() -> bool;
    fn save_game_state() -> bool;
    fn load_game_state() -> bool;
    fn free_buffer();
    fn advance_frame(flags: i32) -> bool;
    fn on_event(info: GGPOEvent);
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

pub trait GGPOSession {
    fn do_poll(timeout: i32) -> GGPOErrorCode<()> {
        GGPOErrorCode::Ok(())
    }

    fn add_player(player: GGPOPlayer, handle: GGPOPlayerHandle) -> GGPOErrorCode<()>;

    fn add_local_input(player: GGPOPlayerHandle, values: String, size: i32) -> GGPOErrorCode<()>;

    fn sync_input(values: String, size: i32, disconnect_flags: i32) -> GGPOErrorCode<()>;

    fn increment_frame() -> GGPOErrorCode<()> {
        GGPOErrorCode::Ok(())
    }

    fn chat(text: String) -> GGPOErrorCode<()> {
        GGPOErrorCode::Ok(())
    }

    fn disconnect_player(handle: GGPOPlayerHandle) -> GGPOErrorCode<()> {
        GGPOErrorCode::Ok(())
    }

    fn get_network_stats(stats: GGPONetworkStats, handle: GGPOPlayerHandle) -> GGPOErrorCode<()> {
        GGPOErrorCode::Ok(())
    }

    //TODO: stub this with the log crate
    //fn logv()

    fn set_frame_delay(player: GGPOPlayerHandle, delay: i32) -> GGPOErrorCode<()> {
        GGPOErrorCode::Unsupported(())
    }

    fn set_disconnect_timeout(timeout: i32) -> GGPOErrorCode<()> {
        GGPOErrorCode::Unsupported(())
    }

    fn set_disconnect_notify_start(timeout: i32) -> GGPOErrorCode<()> {
        GGPOErrorCode::Unsupported(())
    }
}
