pub type PlayerHandle = usize;

pub enum PlayerType {
    Local,
    Remote(std::net::SocketAddr),
    Spectator,
}

pub struct Player {
    size: usize,
    player_type: PlayerType,
    player_num: usize,
}

impl Player {
    pub fn new(player_type: PlayerType, player_num: usize) -> Player {
        Player {
            player_num,
            player_type,
            size: std::mem::size_of::<Player>(),
        }
    }
}
