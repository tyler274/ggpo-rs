use crate::{
    game_input::{FrameNum, GameInput, InputBuffer},
    ggpo::{
        self, GGPOError, GGPOSessionCallbacks, NetworkStats, Session, GGPO_MAX_PLAYERS,
        GGPO_MAX_SPECTATORS,
    },
    network::{
        udp::{Udp, UdpCallback, UdpError},
        udp_msg::{ConnectStatus, UdpMsg, UDP_MSG_MAX_PLAYERS},
        udp_proto::{self, UdpProtoError, UdpProtocol},
    },
    player::{Player, PlayerHandle},
    sync::{self, GGPOSync, SyncError},
};
use log::{error, info};
use std::{net::SocketAddr, sync::Arc};
use thiserror::Error;

const RECOMMENDATION_INTERVAL: u32 = 240;
const DEFAULT_DISCONNECT_TIMEOUT: u128 = 5000;
const DEFAULT_DISCONNECT_NOTIFY_START: u128 = 750;
#[derive(Debug, Error)]
pub enum SyncTestError {
    #[error("UDP protocol error.")]
    UdpProtocol {
        #[from]
        source: UdpProtoError,
    },
    #[error("UDP network error.")]
    Udp {
        #[from]
        source: UdpError,
    },
    #[error("Last frame is None.")]
    LastFrameNone,
    #[error("Next Spectator Frame is None.")]
    SpectatorFrameNone,
    #[error("current_remote_frame is None.")]
    CurrentRemoteFrameNone,
    #[error("GGPO Session error {0}")]
    GGPO(String),
    #[error("Sync Error")]
    Sync {
        #[from]
        source: SyncError,
    },
}
