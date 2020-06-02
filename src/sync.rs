use crate::game_input;
use crate::input_queue;
use crate::network::udp_msg::ConnectStatus;
use crate::player::GGPOSessionCallbacks;

use arraydeque::ArrayDeque;

const MAX_PREDICTION_FRAMES: usize = 8;

pub struct Config<'a, T: GGPOSessionCallbacks> {
    callbacks: &'a T,
    num_prediction_frames: usize,
    num_players: usize,
    input_size: usize,
}

enum Type {
    ConfirmedInput,
}

pub struct Event {
    input_type: Type,
    input: game_input::GameInput,
}

pub struct SavedFrame<'a> {
    size: usize,
    frame: Option<usize>,
    checksum: Option<usize>,
    buffer: Option<&'a [u8]>,
}

impl<'a> SavedFrame<'a> {
    fn new() -> SavedFrame<'a> {
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
    callbacks: &'a T,
    saved_state: SavedState<'a>,
    config: Config<'a, T>,

    rolling_back: bool,
    last_confirmed_frame: Option<usize>,
    frame_count: usize,
    max_prediction_frames: usize,

    input_queue: &'a input_queue::InputQueue,

    event_queue: ArrayDeque<[Event; 32]>,
    local_connect_status: &'a ConnectStatus,
}

pub trait SyncTrait<'a, T: GGPOSessionCallbacks> {
    fn new(connect_status: &ConnectStatus) -> Sync<T>;
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
