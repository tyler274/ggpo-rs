use crate::constants::MAX_PLAYERS;
// Is the redefined constant above supposed to be 64, because of spectator's not needing inputs tracked?
use ggpo::{
    game_input::{Frame, FrameNum, GAMEINPUT_MAX_PLAYERS},
    ggpo::Event,
    player::{Player, PlayerHandle, PlayerType},
};

/*
 * These are other pieces of information not related to the state
 * of the game which are useful to carry around.  They are not
 * included in the GameState class because they specifically
 * should not be rolled back.
 */

#[derive(Debug, Copy, Clone)]
pub enum PlayerConnectState {
    Connecting,
    Synchronizing,
    Running,
    Disconnected,
    Disconnecting,
}

pub struct PlayerConnectionInfo {
    pub _type: PlayerType,
    pub handle: PlayerHandle,
    pub state: PlayerConnectState,
    pub connect_progress: i32,
    pub disconnect_timeout: std::time::Duration,
    pub disconnect_start: std::time::Instant,
}
pub struct ChecksumInfo {
    pub frame_number: Frame,
    pub checksum: u32,
}
impl ChecksumInfo {
    pub fn frame_number(&self) -> Option<FrameNum> {
        self.frame_number
    }
}

pub struct NonGameState {
    pub local_player_handle: PlayerHandle,
    pub players: [PlayerConnectionInfo; MAX_PLAYERS],
    pub num_players: u32,
    pub now: ChecksumInfo,
    pub periodic: ChecksumInfo,
}

impl NonGameState {
    pub fn set_player_connect_state(&mut self, handle: PlayerHandle, state: PlayerConnectState) {
        for i in 0..self.num_players as usize {
            if self.players[i].handle == handle {
                self.players[i].connect_progress = 0;
                self.players[i].state = state;
                break;
            }
        }
    }
    pub fn set_disconnect_timeout(
        &mut self,
        handle: PlayerHandle,
        now: std::time::Instant,
        timeout: std::time::Duration,
    ) {
        for i in 0..self.num_players as usize {
            if self.players[i].handle == handle {
                self.players[i].disconnect_start = now;
                self.players[i].disconnect_timeout = timeout;
                self.players[i].state = PlayerConnectState::Disconnecting;
                break;
            }
        }
    }
    pub fn set_connect_state(&mut self, state: PlayerConnectState) {
        for i in 0..self.num_players as usize {
            self.players[i].state = state;
        }
    }
    pub fn update_connect_progress(&mut self, handle: PlayerHandle, progress: i32) {
        for i in 0..self.num_players as usize {
            if self.players[i].handle == handle {
                self.players[i].connect_progress = progress;
                break;
            }
        }
    }
}
