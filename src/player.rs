pub type PlayerHandle = u32;

pub enum PlayerType {
    Local,
    Remote(std::net::SocketAddr),
    Spectator,
}

pub struct Player {
    _size: usize,
    _player_type: PlayerType,
    _player_num: usize,
}

impl Player {
    pub fn new(player_type: PlayerType, player_num: usize) -> Player {
        Player {
            _player_num: player_num,
            _player_type: player_type,
            _size: std::mem::size_of::<Player>(),
        }
    }
}
