use crate::{
    game_input::{
        Frame, FrameNum, GameInput, InputBuffer, GAMEINPUT_MAX_BYTES, GAMEINPUT_MAX_PLAYERS,
        INPUT_BUFFER_SIZE,
    },
    ggpo::{GGPOSessionCallbacks, GGPO_MAX_PREDICTION_FRAMES},
    input_queue::InputQueue,
    network::udp_msg::ConnectStatus,
};

use async_mutex::Mutex;
use bytes::Bytes;
use log::{error, info, warn};
use std::{collections::VecDeque, sync::Arc};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Config is uninitialized.")]
    ConfigNone,
}

#[derive(Debug, Clone)]
pub struct Config<T: GGPOSessionCallbacks> {
    pub callbacks: Option<Arc<Mutex<T>>>,
    pub num_prediction_frames: FrameNum,
    pub num_players: usize,
    pub input_size: usize,
}

impl<T: GGPOSessionCallbacks> Default for Config<T> {
    fn default() -> Self {
        Config {
            callbacks: None,
            num_prediction_frames: 0,
            num_players: 0,
            input_size: 0,
        }
    }
}

impl<T: GGPOSessionCallbacks> Config<T> {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn init(
        &mut self,
        callbacks: Arc<Mutex<T>>,
        num_prediction_frames: FrameNum,
        num_players: usize,
        input_size: usize,
    ) {
        self.callbacks = Some(callbacks.clone());
        self.num_players = num_players;
        self.input_size = input_size;
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Type {
    ConfirmedInput,
    Other,
}

#[derive(Debug, Copy, Clone)]
pub struct Event {
    pub input_type: Type,
    pub input: GameInput,
}

impl Event {
    pub const fn new() -> Self {
        Self {
            input_type: Type::Other,
            input: GameInput::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SavedFrame {
    size: usize,
    frame: Frame,
    checksum: Option<u32>,
    buffer: Bytes,
}

impl SavedFrame {
    const fn new() -> Self {
        SavedFrame {
            size: 0,
            frame: None,
            checksum: None,
            buffer: Bytes::new(),
        }
    }
}

const BLANK_FRAME: SavedFrame = SavedFrame::new();

#[derive(Debug, Clone)]
struct SavedState {
    frames: [SavedFrame; GGPO_MAX_PREDICTION_FRAMES as usize + 2],
    head: usize,
}

#[derive(Clone)]
pub struct GGPOSync<T: GGPOSessionCallbacks + Send + Sync + Clone> {
    callbacks: Option<Arc<Mutex<T>>>,
    saved_state: SavedState,
    config: Option<Config<T>>,

    rolling_back: bool,
    last_confirmed_frame: Frame,
    frame_count: FrameNum,
    max_prediction_frames: FrameNum,

    input_queues: Vec<InputQueue>,

    // event_queue: ArrayDeque<[Event; 32]>,
    event_queue: VecDeque<Event>,
    local_connect_status: Vec<Arc<Mutex<ConnectStatus>>>,
}

impl<T: GGPOSessionCallbacks + Send + Sync + Clone> Default for GGPOSync<T> {
    fn default() -> GGPOSync<T> {
        GGPOSync {
            local_connect_status: Vec::new(),
            frame_count: 0,
            last_confirmed_frame: None,
            max_prediction_frames: 0,
            saved_state: SavedState {
                head: 0,
                frames: [BLANK_FRAME; GGPO_MAX_PREDICTION_FRAMES as usize + 2],
            },
            callbacks: None,
            config: None,
            rolling_back: false,
            input_queues: Vec::new(),
            // event_queue: ArrayDeque::new(),
            event_queue: VecDeque::with_capacity(32),
        }
    }
}

impl<T: GGPOSessionCallbacks + Send + Sync + Clone> GGPOSync<T> {
    pub fn new(connect_status: &[Arc<Mutex<ConnectStatus>>]) -> Self {
        GGPOSync {
            local_connect_status: Vec::from(connect_status),
            ..Default::default()
        }
    }

    pub fn init(&mut self, config: Config<T>) {
        // self.callbacks = config.callbacks;
        self.max_prediction_frames = config.num_prediction_frames;
        self.config = Some(config.clone());
        self.frame_count = 0;
        self.rolling_back = false;

        self.create_queues();
    }

    pub fn create_queues(&mut self) -> bool {
        match &self.config {
            Some(config) => {
                for i in 0..config.num_players {
                    self.input_queues[i] = InputQueue::init(i, config.input_size);
                }
            }
            None => {
                error!("Config is None and not initialized");
                return false;
            }
        }

        true
    }

    pub fn set_last_confirmed_frame(&mut self, frame: Frame) {
        self.last_confirmed_frame = frame;
        match (self.last_confirmed_frame, self.config.as_ref()) {
            (Some(last_confirmed_frame), Some(config)) => {
                if last_confirmed_frame > 0 {
                    for i in 0..config.num_players {
                        self.input_queues[i].discard_confirmed_frames(last_confirmed_frame - 1);
                    }
                }
            }
            (_, None) => error!("Config not initialized"),
            // If last_confirmed_frame is null, just move on.
            _ => (),
        }
    }

    pub fn add_local_input(&mut self, queue: u32, input: &mut GameInput) -> bool {
        let frames_behind: FrameNum;
        match self.last_confirmed_frame {
            Some(last_confirmed_frame) => frames_behind = self.frame_count - last_confirmed_frame,
            None => frames_behind = self.frame_count + 1,
        }

        if self.frame_count >= self.max_prediction_frames
            && frames_behind >= self.max_prediction_frames
        {
            info!("Rejecting input from emulator: reached prediction barrier.\n");
            return false;
        }

        if self.frame_count == 0 {
            self.save_current_frame();
        }

        info!(
            "Sending undelayed local frame {} to queue {}.\n",
            self.frame_count, queue
        );

        input.frame = Some(self.frame_count);

        self.input_queues[queue as usize].add_input(*input);

        true
    }

    pub fn add_remote_input(&mut self, queue: u32, input: &GameInput) {
        self.input_queues[queue as usize].add_input(*input);
    }

    pub async fn save_current_frame(&mut self) {
        // Copying this for reference later.
        // TODO: zstd compression
        /*
         * See StateCompress for the real save feature implemented by FinalBurn.
         * Write everything into the head, then advance the head pointer.
         */

        // it was pointer arithmetic
        let mut state: &mut SavedFrame = &mut self.saved_state.frames[self.saved_state.head];
        match (&mut self.config, &state.buffer) {
            (
                Some(Config {
                    callbacks: Some(callbacks),
                    ..
                }),
                buffer,
            ) => {
                callbacks.lock().await.free_buffer(buffer);
                state.buffer.clear();
            }
            (
                Some(Config {
                    callbacks: None, ..
                }),
                _,
            ) => error!("Callbacks not initialized"),
            (None, _) => error!("Config not initialized"),
        }

        state.frame = Some(self.frame_count);
        match self.callbacks.as_mut() {
            Some(callbacks) => {
                callbacks.lock().await.save_game_state(
                    &state.buffer,
                    &state.size,
                    state.checksum,
                    state.frame,
                );
            }
            None => error!("Callbacks not initialized"),
        }

        match (state.frame, state.checksum) {
            (Some(frame), None) => info!(
                "=== Saved frame info {} (size: {}  checksum: None).\n",
                frame, state.size
            ),
            (Some(frame), Some(checksum)) => info!(
                "=== Saved frame info {} (size: {}  checksum: {:#x}).\n",
                frame, state.size, checksum
            ),
            _ => info!(
                "=== Saved frame info None (size: {}  checksum: None).\n",
                state.size
            ),
        }

        self.saved_state.head += 1;
    }

    pub fn get_last_saved_frame(&self) -> &SavedFrame {
        let mut i: isize = self.saved_state.head as isize - 1;
        if i < 0 {
            i = self.saved_state.frames.len() as isize - 1;
        }
        &self.saved_state.frames[i as usize]
    }

    pub fn find_saved_frame_index(&self, frame: Frame) -> usize {
        let count = self.saved_state.frames.len();
        let mut j: usize = 0;

        for i in 0..count {
            if self.saved_state.frames[i].frame == frame {
                break;
            }
            j = i;
        }
        if j == count {
            unreachable!();
        }

        j
    }

    pub fn set_frame_delay(&mut self, queue: usize, delay: usize) {
        self.input_queues[queue].set_frame_delay(delay);
    }

    pub fn reset_prediction(&mut self, frame_number: FrameNum) {
        match self.config.as_ref() {
            Some(config) => {
                for i in 0..config.num_players {
                    self.input_queues[i].reset_prediction(frame_number);
                }
            }
            None => error!("Config is uninitialized"),
        }
    }

    pub fn get_event(&mut self, event: &mut Event) -> bool {
        if let Some(e) = self.event_queue.pop_front() {
            *event = e;
            return true;
        }
        false
    }

    pub fn get_frame_count(&self) -> FrameNum {
        self.frame_count
    }

    pub fn in_rollback(&self) -> bool {
        self.rolling_back
    }

    pub fn increment_frame(&mut self) {
        self.frame_count += 1;
        self.save_current_frame();
    }

    pub async fn get_confirmed_inputs(
        &mut self,
        values: &mut InputBuffer,
        frame: Frame,
    ) -> Result<usize, SyncError> {
        let mut disconnect_flags: usize = 0;

        assert!(
            values.len()
                >= self
                    .config
                    .as_ref()
                    .ok_or(SyncError::ConfigNone)?
                    .num_players
        );
        values.fill([b'0'; GAMEINPUT_MAX_BYTES]);
        for i in 0..self
            .config
            .as_ref()
            .ok_or(SyncError::ConfigNone)?
            .num_players
        {
            let mut input: GameInput = GameInput::new();
            if let Some(frame_value) = frame {
                // TODO: What was the original intent when -1 is received as a frame.
                let connect_status = *self.local_connect_status[i].lock().await;
                if connect_status.disconnected
                    && frame_value as i32 > connect_status.last_frame.unwrap_or(0) as i32 - 1
                {
                    disconnect_flags |= 1 << i;
                    input.erase();
                } else {
                    self.input_queues[i].get_confirmed_input(frame, &mut input);
                }
                values[i] = input.bits[i];
            }
        }

        Ok(disconnect_flags)
    }

    pub async fn synchronize_inputs(
        &mut self,
        values: &mut Vec<InputBuffer>,
    ) -> Result<i32, SyncError> {
        let mut disconnect_flags = 0;

        assert!(
            values.capacity()
                >= self
                    .config
                    .as_ref()
                    .ok_or(SyncError::ConfigNone)?
                    .num_players
        );

        values.fill([[b'0'; GAMEINPUT_MAX_BYTES]; GAMEINPUT_MAX_PLAYERS]);

        for i in 0..self
            .config
            .as_ref()
            .ok_or(SyncError::ConfigNone)?
            .num_players
        {
            let mut input: GameInput = GameInput::new();
            let connect_status = *self.local_connect_status[i].lock().await;
            if connect_status.disconnected
                && self.frame_count as i32 > connect_status.last_frame.unwrap_or(0) as i32 - 1
            {
                disconnect_flags |= 1 << i;
                input.erase();
            } else {
                self.input_queues[i].get_input(self.frame_count, &mut input);
            }
            values[i] = input.bits;
        }

        Ok(disconnect_flags)
    }

    pub async fn check_simulation(&mut self) -> Result<(), SyncError> {
        let mut seek_to: FrameNum = 0;
        if !self.check_simulation_consistency(&mut seek_to)? {
            self.adjust_simulation(seek_to).await;
        }
        Ok(())
    }

    pub fn check_simulation_consistency(
        &mut self,
        seek_to: &mut FrameNum,
    ) -> Result<bool, SyncError> {
        let mut first_incorrect: Frame = None;

        for i in 0..self
            .config
            .as_ref()
            .ok_or(SyncError::ConfigNone)?
            .num_players
        {
            match (
                self.input_queues[i].get_first_incorrect_frame(),
                first_incorrect,
            ) {
                (Some(incorrect), Some(f_cor)) => {
                    info!(
                        "considering incorrect frame {} reported by queue {}.\n",
                        incorrect, i
                    );
                    first_incorrect = if incorrect < f_cor {
                        Some(incorrect)
                    } else {
                        first_incorrect
                    }
                }
                (Some(incorrect), None) => (first_incorrect = Some(incorrect)),
                (None, _) => (),
            }
        }

        if let Some(f_cor) = first_incorrect {
            *seek_to = f_cor;
        } else {
            info!("Prediction ok. Proceeding.\n");
            return Ok(true);
        }

        Ok(false)
    }

    pub async fn adjust_simulation(&mut self, seek_to: FrameNum) {
        let framecount = self.frame_count;
        let count = self.frame_count - seek_to;

        info!("Catching up\n");
        self.rolling_back = true;
        /*
         * Flush our input queue and load the last frame.
         */
        self.load_frame(Some(seek_to));
        assert!(self.frame_count == seek_to);

        /*
         * Advance frame by frame (stuffing notifications back to
         * the master).
         */
        self.reset_prediction(self.frame_count);

        if let Some(callbacks) = &mut self.callbacks {
            for _i in 0..count {
                callbacks.lock().await.advance_frame(0);
            }
        } else {
            error!("Callbacks are uninitialized");
        }

        assert!(self.frame_count == framecount);

        self.rolling_back = false;

        info!("---\n");
    }

    pub async fn load_frame(&mut self, frame: Frame) {
        // find the frame in question
        if frame == Some(self.frame_count) {
            info!("Skipping NOP.\n");
            return;
        }

        // Move the head pointer back and load it up
        self.saved_state.head = self.find_saved_frame_index(frame);
        let state: &mut SavedFrame = &mut self.saved_state.frames[self.saved_state.head];

        match (state.frame, state.checksum) {
            (Some(frame), Some(checksum)) => info!(
                "=== Loading frame info {} (size: {}  checksum: {:#X}).\n",
                frame, state.size, checksum
            ),
            (Some(frame), _) => info!(
                "=== Loading frame info {} (size: {}  checksum: None).\n",
                frame, state.size,
            ),
            (None, _) => info!(
                "=== Loading frame info None (size: {}  checksum: None).\n",
                state.size,
            ),
        }

        // TODO: Obviously these serve the same purpose, but still testing the use of the `bytes` crate
        assert!(state.buffer.len() > 0 && state.size > 0);

        match (&mut self.callbacks, &state.buffer) {
            (_, s) if s.len() == 0 => error!("State buffer not initialized"),
            (Some(callbacks), buffer) => {
                callbacks.lock().await.load_game_state(&buffer, state.size);
            }
            (None, _) => error!("Callbacks not initialized!"),
        };

        // Reset framecount and the head of the state ring-buffer to point in
        // advance of the current frame (as if we had just finished executing it).
        self.frame_count = if let Some(frame) = state.frame {
            frame
        } else {
            self.frame_count
        };
        self.saved_state.head = self.saved_state.head + 1;
    }
}
