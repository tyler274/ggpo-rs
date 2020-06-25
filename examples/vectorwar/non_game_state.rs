use crate::constants::MAX_PLAYERS;
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

struct PlayerConnectionInfo {
    _type: PlayerType,
    handle: PlayerHandle,
    state: PlayerConnectState,
    connect_progress: i32,
    disconnect_timeout: i32,
    disconnect_start: i32,
}

struct ChecksumInfo {
    frame_number: Frame,
    checksum: u32,
}

pub struct NonGameState {
    local_player_handle: PlayerHandle,
    players: [PlayerConnectionInfo; MAX_PLAYERS],
    num_players: usize,
    now: ChecksumInfo,
    periodic: ChecksumInfo,
}

impl NonGameState {
    pub fn set_player_connect_state(&mut self, handle: PlayerHandle, state: PlayerConnectState) {
        for i in 0..self.num_players {
            if self.players[i].handle == handle {
                self.players[i].connect_progress = 0;
                self.players[i].state = state;
                break;
            }
        }
    }
    pub fn set_disconnect_timeout(&mut self, handle: PlayerHandle, now: i32, timeout: i32) {
        for i in 0..self.num_players {
            if self.players[i].handle == handle {
                self.players[i].disconnect_start = now;
                self.players[i].disconnect_timeout = timeout;
                self.players[i].state = PlayerConnectState::Disconnecting;
                break;
            }
        }
    }
    pub fn set_connect_state(&mut self, state: PlayerConnectState) {
        for i in 0..self.num_players {
            self.players[i].state = state;
        }
    }
    pub fn update_connect_progress(&mut self, handle: PlayerHandle, progress: i32) {
        for i in 0..self.num_players {
            if self.players[i].handle == handle {
                self.players[i].connect_progress = progress;
                break;
            }
        }
    }
    
}
