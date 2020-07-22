pub type PlayerHandle = u32;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum PlayerType {
    Local,
    Remote(std::net::SocketAddr),
    Spectator(std::net::SocketAddr),
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct Player {
    pub size: usize,
    pub player_type: PlayerType,
    pub player_num: usize,
}

impl Player {
    pub fn new(player_type: PlayerType, player_num: usize) -> Player {
        Player {
            player_num: player_num,
            player_type: player_type,
            size: std::mem::size_of::<Player>(),
        }
    }
}
