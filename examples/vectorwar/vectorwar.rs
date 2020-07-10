use crate::constants::MAX_PLAYERS;
use crate::game_state::MAX_SHIPS;
use crc32fast::Hasher;
use enumflags2::BitFlags;
use ggpo::{ggpo::Event, player::Player};
use std::net::{IpAddr, SocketAddr};

pub const FRAME_DELAY: usize = 2;
// TODO: Synctest during testing

#[derive(BitFlags, Debug, Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum Input {
    Thrust = 0b0000_0001,
    Break = 0b0000_0010,
    RotateLeft = 0b0000_0100,
    RotateRight = 0b0000_1000,
    Fire = 0b0001_0000,
    Bomb = 0b0010_0000,
}

pub fn init(local_port: u16, num_players: i32, players: &[Player], num_spectators: i32) {
    unimplemented!()
}

pub fn init_spectator(local_port: u16, num_players: i32, host_ip: IpAddr, host_port: u16) {
    unimplemented!()
}

pub fn draw_current_frame() {
    unimplemented!()
}

pub fn advance_frame(inputs: Vec<i32>, disconnect_flags: i32) {
    unimplemented!()
}

pub fn run_frame(input: i32) -> i32 {
    unimplemented!()
}

pub fn idle(time: i32) {}

pub fn disconnect_player(player: i32) {}

pub fn exit() {}

/*
 * The begin game callback.  We don't need to do anything special here,
 * so just return true.
 */
// TODO: Deprecated, remove later
fn begin_game_callback(name: &str) -> bool {
    true
}

/*
 * Notification from GGPO that something has happened.  Update the status
 * text at the bottom of the screen to notify the user.
 */
fn on_event_callback(info: &Event) -> bool {
    let progress: i32;
    // match info {
    //     Event::ConnectedToPeer =>
    // }
    unimplemented!()
}

/*
 * Notification from GGPO we should step foward exactly 1 frame
 * during a rollback.
 */
fn advance_frame_callback(flags: i32) -> bool {
    let inputs: [BitFlags<Input>; MAX_SHIPS] = [BitFlags::empty(); MAX_SHIPS];
    let disconnect_flags: i32 = 0;

    // Make sure we fetch new inputs from GGPO and use those to update
    // the game state instead of reading from the keyboard.

    // FIXME
    true
}
