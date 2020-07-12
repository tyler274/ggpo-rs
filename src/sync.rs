use crate::game_input::{Frame, FrameNum, GameInput, InputBuffer, INPUT_BUFFER_SIZE};
use crate::ggpo::GGPOSessionCallbacks;
use crate::input_queue::InputQueue;
use crate::network::udp_msg::ConnectStatus;
use bytes::Bytes;
use log::{error, info, warn};
use std::collections::VecDeque;

// use arraydeque::ArrayDeque;

const MAX_PREDICTION_FRAMES: usize = 8;

#[derive(Debug)]
pub struct Config<'a, T: GGPOSessionCallbacks> {
    callbacks: Option<&'a mut T>,
    num_prediction_frames: FrameNum,
    num_players: usize,
    input_size: usize,
}

impl<'a, T: GGPOSessionCallbacks> Default for Config<'a, T> {
    fn default() -> Self {
        Config {
            callbacks: None,
            num_prediction_frames: 0,
            num_players: 0,
            input_size: 0,
        }
    }
}

impl<'a, T: GGPOSessionCallbacks> Config<'a, T> {
    pub fn new() -> Self {
        Default::default()
    }
}

enum Type {
    _ConfirmedInput,
}

pub struct Event {
    _input_type: Type,
    _input: GameInput,
}

#[derive(Debug)]
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

struct SavedState {
    frames: [SavedFrame; MAX_PREDICTION_FRAMES + 2],
    head: usize,
}

pub struct GGPOSync<'callbacks, 'network, Callbacks>
where
    Callbacks: GGPOSessionCallbacks,
{
    callbacks: Option<&'callbacks mut Callbacks>,
    saved_state: SavedState,
    config: Option<&'callbacks mut Config<'callbacks, Callbacks>>,

    rolling_back: bool,
    last_confirmed_frame: Frame,
    frame_count: FrameNum,
    max_prediction_frames: FrameNum,

    input_queues: Option<Vec<InputQueue>>,

    // event_queue: ArrayDeque<[Event; 32]>,
    event_queue: VecDeque<Event>,
    local_connect_status: Option<Vec<&'network ConnectStatus>>,
}

impl<'a, 'b, Callbacks> Default for GGPOSync<'a, 'b, Callbacks>
where
    Callbacks: GGPOSessionCallbacks,
{
    fn default() -> GGPOSync<'a, 'b, Callbacks> {
        GGPOSync {
            local_connect_status: None,
            frame_count: 0,
            last_confirmed_frame: None,
            max_prediction_frames: 0,
            saved_state: SavedState {
                head: 0,
                frames: [BLANK_FRAME; MAX_PREDICTION_FRAMES + 2],
            },
            callbacks: None,
            config: None,
            rolling_back: false,
            input_queues: None,
            // event_queue: ArrayDeque::new(),
            event_queue: VecDeque::with_capacity(32),
        }
    }
}

pub trait SyncTrait<'a, 'b, Callbacks>
where
    Callbacks: GGPOSessionCallbacks,
{
    fn new(connect_status: Vec<&'b ConnectStatus>) -> GGPOSync<'a, 'b, Callbacks>;
    fn init(&mut self, config: &'a mut Config<'a, Callbacks>);
    fn set_last_confirmed_frame(&mut self, frame: Frame);
    fn set_frame_delay(&mut self, queue: usize, delay: usize);
    fn add_local_input(&mut self, queue: usize, input: &mut GameInput) -> bool;
    fn add_remote_input(&mut self, queue: usize, input: &GameInput);
    // void *....................
    fn get_confirmed_inputs(&mut self, values: &mut Vec<InputBuffer>, frame: Frame) -> usize;
    fn synchronize_inputs(&mut self, values: &mut Vec<InputBuffer>) -> usize;

    fn check_simulation(&mut self);
    fn adjust_simulation(&mut self, seek_to: FrameNum);
    // void......again
    fn increment_frame(&mut self);

    fn get_frame_count(&self) -> FrameNum;
    fn in_rollback(&self) -> bool;
    fn get_event(&mut self) -> Option<Event>;

    fn load_frame(&mut self, frame: Frame);
    fn save_current_frame(&mut self);
    fn find_saved_frame_index(&self, frame: Frame) -> usize;
    fn get_last_saved_frame(&self) -> &SavedFrame;

    fn create_queues(&mut self) -> bool;
    fn check_simulation_consistency(&mut self, seek_to: &mut FrameNum) -> bool;
    fn reset_prediction(&mut self, frame_number: FrameNum);
}

impl<'a, 'b, T: GGPOSessionCallbacks> SyncTrait<'a, 'b, T> for GGPOSync<'a, 'b, T> {
    fn new(connect_status: Vec<&'b ConnectStatus>) -> Self {
        GGPOSync {
            local_connect_status: Some(Vec::from(connect_status)),
            ..Default::default()
        }
    }

    fn init(&mut self, config: &'a mut Config<'a, T>) {
        // self.callbacks = config.callbacks;
        self.max_prediction_frames = config.num_prediction_frames;
        self.config = Some(config);
        self.frame_count = 0;
        self.rolling_back = false;

        self.create_queues();
    }

    fn create_queues(&mut self) -> bool {
        match (&self.config, &mut self.input_queues) {
            (Some(config), None) => {
                self.input_queues = Some(Vec::with_capacity(config.num_players));
                return self.create_queues();
            }
            (Some(config), Some(input_queues)) => {
                for i in 0..config.num_players {
                    input_queues[i] = InputQueue::init(i, config.input_size);
                }
            }
            (None, _) => {
                error!("Config is None and not initialized");
                return false;
            }
        }

        true
    }

    fn set_last_confirmed_frame(&mut self, frame: Frame) {
        self.last_confirmed_frame = frame;
        match (
            self.last_confirmed_frame,
            self.config.as_ref(),
            &mut self.input_queues,
        ) {
            (Some(last_confirmed_frame), Some(config), Some(input_queues)) => {
                if last_confirmed_frame > 0 {
                    for i in 0..config.num_players {
                        input_queues[i].discard_confirmed_frames(last_confirmed_frame - 1);
                    }
                }
            }
            (_, None, _) => error!("Config not initialized"),
            (_, _, None) => {
                // Recurse here if we hit this arm with the input queues as None after initializing them.
                warn!("Input queues weren't initialized, handling it now");
                self.create_queues();
                self.set_last_confirmed_frame(frame);
            }
            // If last_confirmed_frame is null, just move on.
            _ => (),
        }
    }

    fn add_local_input(&mut self, queue: usize, input: &mut GameInput) -> bool {
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

        match &mut self.input_queues {
            Some(input_queues) => input_queues[queue].add_input(*input),
            None => {
                self.create_queues();
                if let Some(input_queues) = &mut self.input_queues {
                    input_queues[queue].add_input(*input);
                }
            }
        }

        true
    }

    fn add_remote_input(&mut self, queue: usize, input: &GameInput) {
        if let Some(input_queues) = &mut self.input_queues {
            input_queues[queue].add_input(*input);
        }
    }

    fn save_current_frame(&mut self) {
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
                callbacks.free_buffer(buffer);
                // state.buffer = None;
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
                callbacks.save_game_state(&state.buffer, &state.size, state.checksum, state.frame);
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

    fn get_last_saved_frame(&self) -> &SavedFrame {
        let mut i: isize = self.saved_state.head as isize - 1;
        if i < 0 {
            i = self.saved_state.frames.len() as isize - 1;
        }
        &self.saved_state.frames[i as usize]
    }

    fn find_saved_frame_index(&self, frame: Frame) -> usize {
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

    fn set_frame_delay(&mut self, queue: usize, delay: usize) {
        match self.input_queues.as_mut() {
            Some(input_queues) => input_queues[queue].set_frame_delay(delay),
            None => {
                warn!("Attempting to use input queues before initializing them, handling it");
                self.create_queues();
                self.set_frame_delay(queue, delay);
            }
        }
    }

    fn reset_prediction(&mut self, frame_number: FrameNum) {
        match (self.input_queues.as_mut(), self.config.as_ref()) {
            (Some(input_queues), Some(config)) => {
                for i in 0..config.num_players {
                    input_queues[i].reset_prediction(frame_number);
                }
            }
            (None, Some(_)) => {
                warn!("Attempting to use input queues before initializing them, handling it");
                self.create_queues();
                self.reset_prediction(frame_number);
            }
            (_, None) => error!("Config is uninitialized"),
        }
    }

    fn get_event(&mut self) -> Option<Event> {
        self.event_queue.pop_front()
    }

    fn get_frame_count(&self) -> FrameNum {
        self.frame_count
    }

    fn in_rollback(&self) -> bool {
        self.rolling_back
    }

    fn increment_frame(&mut self) {
        self.frame_count += 1;
        self.save_current_frame();
    }

    fn get_confirmed_inputs(&mut self, values: &mut Vec<InputBuffer>, frame: Frame) -> usize {
        let mut disconnect_flags = 0;

        match (
            self.local_connect_status.as_ref(),
            self.input_queues.as_ref(),
            self.config.as_ref(),
        ) {
            (Some(local_connect_status), Some(input_queues), Some(config)) => {
                assert!(values.capacity() >= config.num_players);
                values.fill([b'0'; INPUT_BUFFER_SIZE]);
                for i in 0..config.num_players {
                    let mut input: GameInput = GameInput::new();
                    if let Some(frame_value) = frame {
                        if local_connect_status[i].disconnected > 0
                            && frame_value as i32
                                > local_connect_status[i].last_frame.unwrap_or(0) as i32 - 1
                        {
                            disconnect_flags |= 1 << i;
                            input.erase();
                        } else {
                            input_queues[i].get_confirmed_input(frame, &mut input);
                        }
                        values[i] = input.bits;
                    }
                }
            }
            (Some(_), None, Some(_)) => {
                warn!("Attempting to use input queues before initializing them, handling it");
                self.create_queues();
                return self.get_confirmed_inputs(values, frame);
            }
            (Some(_), None, None) => error!("input_queues, and config are None"),
            (None, Some(_), None) => error!("local_connect_status, config are None"),
            (None, None, Some(_)) => error!("local_connect_status, and input_queues are None"),
            (None, Some(_), Some(_)) => error!("local_connect_status is None"),
            (Some(_), Some(_), None) => error!("config is None"),
            (None, None, None) => error!("local_connect_status, input_queues, and config are None"),
        }

        disconnect_flags
    }

    fn synchronize_inputs(&mut self, values: &mut Vec<InputBuffer>) -> usize {
        let mut disconnect_flags: usize = 0;

        match (
            self.local_connect_status.as_ref(),
            self.input_queues.as_mut(),
            self.config.as_ref(),
        ) {
            (Some(local_connect_status), Some(input_queues), Some(config)) => {
                assert!(values.capacity() >= config.num_players);

                values.fill([b'0'; INPUT_BUFFER_SIZE]);

                for i in 0..config.num_players {
                    let mut input: GameInput = GameInput::new();
                    if local_connect_status[i].disconnected > 0
                        && self.frame_count as i32
                            > local_connect_status[i].last_frame.unwrap_or(0) as i32 - 1
                    {
                        disconnect_flags |= 1 << i;
                        input.erase();
                    } else {
                        input_queues[i].get_input(self.frame_count, &mut input);
                    }
                    values[i] = input.bits;
                }
            }
            (Some(_), None, Some(_)) => {
                warn!("Attempting to use input queues before initializing them, handling it");
                self.create_queues();
                return self.synchronize_inputs(values);
            }
            (Some(_), None, None) => error!("input_queues, and config are None"),
            (None, Some(_), None) => error!("local_connect_status, config are None"),
            (None, None, Some(_)) => error!("local_connect_status, and input_queues are None"),
            (None, Some(_), Some(_)) => error!("local_connect_status is None"),
            (Some(_), Some(_), None) => error!("config is None"),
            (None, None, None) => error!("local_connect_status, input_queues, and config are None"),
        }

        disconnect_flags
    }

    fn check_simulation(&mut self) {
        let mut seek_to: FrameNum = 0;
        if !self.check_simulation_consistency(&mut seek_to) {
            self.adjust_simulation(seek_to);
        }
    }

    fn check_simulation_consistency(&mut self, seek_to: &mut FrameNum) -> bool {
        let mut first_incorrect: Frame = None;

        match (&self.input_queues, self.config.as_ref()) {
            (Some(input_queues), Some(config)) => {
                for i in 0..config.num_players {
                    match (input_queues[i].get_first_incorrect_frame(), first_incorrect) {
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
            }
            (None, Some(_)) => {
                warn!("Attempting to use input queues before initializing them, handling it");
                self.create_queues();
                return self.check_simulation_consistency(seek_to);
            }
            (_, None) => error!("config is None"),
        }

        if let Some(f_cor) = first_incorrect {
            *seek_to = f_cor;
        } else {
            info!("Prediction ok. Proceeding.\n");
            return true;
        }

        false
    }

    fn adjust_simulation(&mut self, seek_to: FrameNum) {
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
                callbacks.advance_frame(0);
            }
        } else {
            error!("Callbacks are uninitialized");
        }

        assert!(self.frame_count == framecount);

        self.rolling_back = false;

        info!("---\n");
    }

    fn load_frame(&mut self, frame: Frame) {
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
                callbacks.load_game_state(&buffer, state.size);
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
