use crate::game_input;
use crate::ggpo::{CallbacksStub, GGPOSessionCallbacks};
use crate::input_queue;
use crate::network::udp_msg::ConnectStatus;

use arraydeque::ArrayDeque;

const MAX_PREDICTION_FRAMES: usize = 8;

pub struct Config<T: GGPOSessionCallbacks> {
    callbacks: Option<Box<T>>,
    num_prediction_frames: usize,
    num_players: usize,
    input_size: usize,
}

impl<'a> Default for Config<CallbacksStub> {
    fn default() -> Self {
        Config {
            callbacks: None,
            num_prediction_frames: 0,
            num_players: 0,
            input_size: 0,
        }
    }
}

enum Type {
    ConfirmedInput,
}

pub struct Event {
    input_type: Type,
    input: game_input::GameInput,
}

#[derive(Debug, Copy, Clone)]
pub struct SavedFrame<'a> {
    size: usize,
    frame: Option<usize>,
    checksum: Option<usize>,
    buffer: Option<&'a [u8]>,
}

impl<'a> SavedFrame<'a> {
    fn new() -> Self {
        SavedFrame {
            size: 0,
            frame: None,
            checksum: None,
            buffer: None,
        }
    }
}

struct SavedState<'a> {
    frames: [SavedFrame<'a>; MAX_PREDICTION_FRAMES + 2],
    head: usize,
}

pub struct Sync<'a, T: GGPOSessionCallbacks> {
    callbacks: Option<Box<T>>,
    saved_state: SavedState<'a>,
    config: Config<T>,

    rolling_back: bool,
    last_confirmed_frame: Option<usize>,
    frame_count: usize,
    max_prediction_frames: usize,

    input_queues: Box<[input_queue::InputQueue]>,

    event_queue: ArrayDeque<[Event; 32]>,
    local_connect_status: Option<&'a ConnectStatus>,
}

impl<'a> Default for Sync<'a, CallbacksStub> {
    fn default() -> Self {
        Sync {
            local_connect_status: None,
            frame_count: 0,
            last_confirmed_frame: None,
            max_prediction_frames: 0,
            saved_state: SavedState {
                head: 0,
                frames: [SavedFrame::new(); MAX_PREDICTION_FRAMES + 2],
            },
            callbacks: None,
            config: Config {
                ..Default::default()
            },
            rolling_back: false,
            input_queues: Box::new([input_queue::InputQueue::new(); 2]),
            event_queue: ArrayDeque::new(),
        }
    }
}

pub trait SyncTrait<'a, T: GGPOSessionCallbacks> {
    fn new(connect_status: &ConnectStatus) -> Sync<'a, T>;
    fn init(&mut self, config: &Config<T>);
    fn set_last_confirmed_frame(frame: Option<usize>);
    fn set_frame_delay(queue: usize, delay: usize);
    fn add_local_input(queue: usize, input: &game_input::GameInput) -> bool;
    fn add_remote_input(queue: usize, input: &game_input::GameInput);
    // void *....................
    fn get_confirmed_inputs(
        values: &[game_input::GameInput],
        size: usize,
        frame: Option<usize>,
    ) -> usize;
    fn synchronize_inputs(values: &[game_input::GameInput]) -> usize;

    fn check_simulation(timeout: usize);
    fn adjust_simulation(seek_to: usize);
    // void......again
    fn increment_frame();

    fn get_frame_count() -> usize;
    fn in_rollback() -> bool;
    fn get_event(e: &Event) -> bool;

    fn load_frame(frame: Option<usize>);
    fn save_current_frame();
    fn find_saved_frame_index(frame: Option<usize>) {}
    fn get_last_saved_frame() -> SavedFrame<'a>;

    fn create_queues(config: &Config<T>) -> bool;
    fn check_simulation_consistency(seek_to: &usize) -> bool;
    fn reset_prediction(frame_number: usize);
}

// impl<'a> SyncTrait<'a, CallbacksStub> for Sync<'a, CallbacksStub> {
//     fn new(connect_status: &ConnectStatus) -> Self {
//         Sync {
//             local_connect_status: connect_status,
//             ..Default::default()
//         }
//     }
// }
