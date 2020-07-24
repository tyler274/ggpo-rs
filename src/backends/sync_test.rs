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
    sync::{self, GGPOSync, SyncError},
};
use log::{error, info};
use parking_lot::Mutex;
use std::{collections::VecDeque, net::SocketAddr, sync::Arc, time::Duration};
use thiserror::Error;

// const RECOMMENDATION_INTERVAL: u32 = 240;
// const DEFAULT_DISCONNECT_TIMEOUT: u128 = 5000;
// const DEFAULT_DISCONNECT_NOTIFY_START: u128 = 750;
#[derive(Debug, Error)]
pub enum SyncTestError {
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
    #[error("Sync Error")]
    Sync {
        #[from]
        source: SyncError,
    },
    #[error("Saves Frames queue was empty.")]
    SavedFramesEmpty,
}

struct SavedInfo {
    frame: Frame,
    checksum: u32,
    buf: bytes::Bytes,
    cbuf: usize,
    input: GameInput,
}

pub struct SyncTestBackend<T>
where
    T: GGPOSessionCallbacks,
{
    callbacks: Arc<Mutex<T>>,
    sync: Arc<Mutex<GGPOSync<T>>>,
    num_players: usize,
    check_distance: u32,
    last_verified: u32,
    rolling_back: bool,
    running: bool,
    log_file: Option<std::fs::File>,
    game: [u8; 128],

    current_input: GameInput,
    last_input: Arc<Mutex<GameInput>>,
    saved_frames: VecDeque<SavedInfo>,
}

impl<T> Session for SyncTestBackend<T>
where
    T: GGPOSessionCallbacks + Send + Sync,
{
    fn do_poll(&mut self, _timeout: Option<Duration>) -> Result<(), GGPOError> {
        if !self.running {
            let info = ggpo::Event::Running;
            self.callbacks.lock().on_event(&info);
            self.running = true;
        }
        Ok(())
    }

    fn add_player(&mut self, player: Player, handle: &mut PlayerHandle) -> Result<(), GGPOError> {
        if player.player_num < 1 || player.player_num > self.num_players {
            return Err(GGPOError::PlayerOutOfRange);
        }
        *handle = player.player_num as u32 - 1;

        Ok(())
    }

    fn add_local_input(
        &mut self,
        player: PlayerHandle,
        values: &InputBuffer,
        size: usize,
    ) -> Result<(), GGPOError> {
        if !self.running {
            return Err(GGPOError::NotSynchronized);
        }
        let index = player as usize;
        for i in 0..self.current_input.bits[index].len() {
            self.current_input.bits[index][i] |= values[index][i];
        }
        Ok(())
    }

    fn synchronize_input(
        &self,
        values: &mut Vec<InputBuffer>,
        disconnect_flags: Option<&mut i32>,
    ) -> Result<(), GGPOError> {
        // TODO: self.begin_log(false);
        if self.rolling_back {
            *self.last_input.lock() = self
                .saved_frames
                .front()
                .ok_or(SyncTestError::SavedFramesEmpty)?
                .input;
        } else {
            let mut sync = self.sync.lock();
            if sync.get_frame_count() == 0 {
                sync.save_current_frame()?;
            }
            *self.last_input.lock() = self.current_input;
        }
        values.push(self.last_input.lock().bits);
        if let Some(flags) = disconnect_flags {
            *flags = 0;
        }
        Ok(())
    }
}

impl<T> SyncTestBackend<T>
where
    T: GGPOSessionCallbacks,
{
    pub fn new(
        callbacks: Arc<Mutex<T>>,
        frames: u32,
        num_players: usize,
    ) -> Result<Self, SyncTestError> {
        /*
         * Initialize the synchronziation layer
         */
        let mut sync_config = sync::Config::new();
        sync_config.callbacks = Some(callbacks.clone());
        sync_config.num_prediction_frames = ggpo::GGPO_MAX_PREDICTION_FRAMES;
        let sync = Arc::new(Mutex::new(GGPOSync::new(&[])));
        sync.lock().init(sync_config)?;

        let s = Self {
            callbacks: callbacks.clone(),
            num_players,
            rolling_back: false,
            running: false,
            check_distance: frames,
            log_file: None,
            current_input: GameInput::new(),
            sync: sync.clone(),
            last_verified: 0,
            last_input: Arc::new(Mutex::new(GameInput::new())),
            saved_frames: VecDeque::with_capacity(32),
            game: [b'0'; 128],
        };

        Ok(s)
    }
}
