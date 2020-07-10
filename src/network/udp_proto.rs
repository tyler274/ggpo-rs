use crate::{
    game_input::GameInput,
    network::{
        udp,
        udp::{Udp, UdpCallback},
        udp_msg,
        udp_msg::{ConnectStatus, UdpMsg, UDP_MSG_MAX_PLAYERS},
    },
    time_sync::TimeSync,
};
use arraydeque::ArrayDeque;
use async_dup::{Arc, Mutex};
use rand::prelude::*;
use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UdpProtocolError {
    #[error("UDP struct unitialized")]
    UdpUninit,
}

enum Event {
    Unknown,
    Connected,
    Synchronizing { total: i32, count: i32 },
    Synchronzied,
    Input(GameInput),
    Disconnected,
    NetworkInterrupted { disconnect_timeout: i32 },
    NetworkResumed,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum CurrentState {
    Syncing,
    Synchronized,
    Running,
    Disconnected,
    Starting,
}

struct QueueEntry {
    _queue_time: i32,
    _dest_addr: SocketAddr,
    _msg: Arc<UdpMsg>,
}

impl Default for QueueEntry {
    fn default() -> Self {
        Self {
            _queue_time: 0,
            _dest_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
            _msg: Default::default(),
        }
    }
}

impl QueueEntry {
    pub const fn new(time: i32, dst: &SocketAddr, m: Arc<UdpMsg>) -> QueueEntry {
        QueueEntry {
            _queue_time: time,
            _dest_addr: *dst,
            _msg: m,
        }
    }
}

enum State {
    Sync {
        roundtrips_remaining: u32,
        random: u32,
    },
    Running {
        last_quality_report_time: u32,
        last_network_stats_interval: u32,
        last_input_packet_recv_time: u32,
    },
    Start,
}

// #[derive()]
struct OoPacket {
    send_time: i32,
    dest_addr: SocketAddr,
    _msg: Arc<UdpMsg>,
}

impl Default for OoPacket {
    fn default() -> Self {
        Self {
            send_time: 0,
            dest_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
            _msg: Default::default(),
        }
    }
}

pub struct UdpProtocol<'a, Callback: UdpCallback> {
    /*
     * Network transmission information
     */
    udp: Option<Arc<Udp<'a, Callback>>>,
    _peer_addr: Option<SocketAddr>,
    magic_number: u16,
    queue: i32,
    remote_magic_number: u16,
    _connected: bool,
    _send_latency: i32,
    _oop_percent: i32,
    _oo_packet: OoPacket,
    _send_queue: ArrayDeque<[QueueEntry; 64]>,
    /*
     * Stats
     */
    _round_trip_time: i32,
    packets_sent: i32,
    bytes_sent: i32,
    _kbps_sent: i32,
    _stats_start_time: i32,
    /*
     * The state machine
     */
    _local_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],
    _peer_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],

    _current_state: CurrentState,

    state: State,

    /*
     * Fairness.
     */
    local_frame_advantage: i32,
    remote_frame_advantage: i32,

    /*
     * Packet loss...
     */
    // pending_output: ArrayDeque<[GameInput; 64]>,
    pending_output: VecDeque<GameInput>,
    last_receieved_input: GameInput,
    _last_sent_input: GameInput,
    last_acked_input: GameInput,
    _last_send_time: u32,
    _last_recv_time: u32,
    _shutdown_timeout: u32,
    _disconnect_event_sent: bool,
    _disconnect_timeout: u32,
    _disconnect_notify_start: u32,
    _disconnect_notify_sent: bool,

    _next_send_seq: u16,
    _next_recv_seq: u16,

    /*
     * Rift synchronization.
     */
    timesync: TimeSync,
    /*
     * Event queue
     */
    _event_queue: VecDeque<Event>,
}

impl<'a, Callback: UdpCallback> UdpProtocol<'a, Callback> {
    pub fn new() -> Self {
        Self {
            local_frame_advantage: 0,
            remote_frame_advantage: 0,
            queue: -1,
            magic_number: 0,
            remote_magic_number: 0,
            packets_sent: 0,
            bytes_sent: 0,
            _stats_start_time: 0,
            _last_send_time: 0,
            _shutdown_timeout: 0,
            _disconnect_timeout: 0,
            _disconnect_notify_start: 0,
            _disconnect_notify_sent: false,
            _disconnect_event_sent: false,
            _connected: false,
            _next_send_seq: 0,
            _next_recv_seq: 0,
            udp: None,

            _last_sent_input: Default::default(),
            last_receieved_input: Default::default(),
            last_acked_input: Default::default(),

            state: State::Start,
            _peer_connect_status: [Default::default(); UDP_MSG_MAX_PLAYERS],
            _peer_addr: None,
            _send_latency: std::env::var("ggpo.network.delay")
                .unwrap_or("".to_string())
                .parse()
                .unwrap_or(0),
            _oop_percent: std::env::var("ggpo.oop.percent")
                .unwrap_or("".to_string())
                .parse()
                .unwrap_or(0),
            _oo_packet: Default::default(),
            _send_queue: ArrayDeque::new(),
            _round_trip_time: 0,
            _kbps_sent: 0,
            _local_connect_status: [Default::default(); UDP_MSG_MAX_PLAYERS],
            _current_state: CurrentState::Starting,
            pending_output: VecDeque::with_capacity(64),
            _last_recv_time: 0,
            timesync: Default::default(),
            _event_queue: VecDeque::with_capacity(64),
        }
    }
    pub fn init(
        &mut self,
        udp: Arc<Udp<'a, Callback>>,
        queue: i32,
        addr: SocketAddr,
        status: &[ConnectStatus; UDP_MSG_MAX_PLAYERS],
    ) {
        self.udp = Some(udp);
        self.queue = queue;
        self._local_connect_status = status.clone();

        self._peer_addr = Some(addr);
        self.magic_number = rand::thread_rng().gen();
    }

    pub async fn send_input(&mut self, input: &GameInput) -> Result<(), String> {
        let udp = self
            .udp
            .as_ref()
            .ok_or(UdpProtocolError::UdpUninit)
            .map_err(|e| e.to_string())?;

        if self._current_state == CurrentState::Running {
            /*
             * Check to see if this is a good time to adjust for the rift...
             */
            self.timesync.advance_frame(
                input,
                self.local_frame_advantage,
                self.remote_frame_advantage,
            );

            /*
            TODO: Solve the below problem by switching to a growable queue, probably just VecDeque or the threadsafe and async ver.
             * Save this input packet
             *
             * XXX: This queue may fill up for spectators who do not ack input packets in a timely
             * manner.  When this happens, we can either resize the queue (ug) or disconnect them
             * (better, but still ug).  For the meantime, make this queue really big to decrease
             * the odds of this happening...
             */
            // This should be fixed but needs to be tested.
            self.pending_output.push_back(input.clone());
        }

        self.send_pending_output().await
    }

    pub async fn send_pending_output(&mut self) -> Result<(), String> {
        let mut msg = UdpMsg::new(udp_msg::MsgType::Input);
        let mut offset = 0;
        let mut bits: &[u8] = &[b'0'];
        let mut last = GameInput::new();
        if self.pending_output.len() > 0 {
            last = self.last_acked_input;
            if let udp_msg::MsgEnum::Input(input) = msg.message {
                bits = &input.bits;
            }
        }

        Ok(())
    }
    pub fn clear_send_queue(&mut self) {
        self._send_queue.clear();
    }
}
