use crate::{
    bitvector,
    game_input::{Frame, FrameNum, GameInput},
    ggpo,
    network::{
        udp,
        udp::{Udp, UdpCallback},
        udp_msg,
        udp_msg::{ConnectStatus, UdpMsg, UDP_MSG_MAX_PLAYERS},
    },
    time_sync::TimeSync,
};
use async_dup::{Arc, Mutex};
use log::{error, info, warn};
use rand::prelude::*;
use rand_distr::{Distribution, Normal};
use thiserror::Error;

use std::{
    collections::VecDeque,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

// TODO: Refactor all Strings in Result types here with error's from this enum.
#[derive(Debug, Error)]
pub enum UdpProtocolError {
    #[error("UDP struct unitialized.")]
    UdpUninit,
    #[error("Pending Output Queue empty.")]
    PendingOutputQueueEmpty,
    #[error("Pending output item out of bounds {:?}", .0)]
    PendingOutputItemOOB(usize),
    #[error("Send queue is empty.")]
    SendQueueEmpty,
    #[error("UDP socket error.")]
    Udp {
        #[from]
        source: udp::UdpError,
    },
    #[error("Peer address is uninitialized.")]
    PeerAddrUninit,
    #[error("OO Packet's msg uninitalized")]
    OOPacketMsgUninit,
    #[error("Time Travel has been reported, please double check system clocks.")]
    TimeTravel {
        #[from]
        source: SystemTimeError,
    },
}

pub enum Event {
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
pub enum CurrentState {
    Syncing,
    Synchronized,
    Running,
    Disconnected,
    Starting,
}

pub struct QueueEntry {
    pub queue_time: std::time::SystemTime,
    pub dest_addr: SocketAddr,
    pub msg: Arc<UdpMsg>,
}

impl Default for QueueEntry {
    fn default() -> Self {
        Self {
            queue_time: std::time::SystemTime::now(),
            dest_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
            msg: Default::default(),
        }
    }
}

impl QueueEntry {
    pub const fn new(time: std::time::SystemTime, dst: &SocketAddr, m: Arc<UdpMsg>) -> QueueEntry {
        QueueEntry {
            queue_time: time,
            dest_addr: *dst,
            msg: m,
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
    send_time: u128,
    dest_addr: SocketAddr,
    msg: Option<Arc<UdpMsg>>,
}

impl Default for OoPacket {
    fn default() -> Self {
        Self {
            send_time: 0,
            dest_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
            msg: Default::default(),
        }
    }
}

pub struct UdpProtocol<'a, Callback: UdpCallback> {
    /*
     * Network transmission information
     */
    udp: Option<Mutex<Udp<'a, Callback>>>,
    peer_addr: Option<SocketAddr>,
    magic_number: u16,
    queue: i32,
    remote_magic_number: u16,
    _connected: bool,
    send_latency: i32,
    oop_percent: i32,
    oo_packet: OoPacket,
    // default to 64
    send_queue: VecDeque<QueueEntry>,
    /*
     * Stats
     */
    round_trip_time: u128,
    packets_sent: i32,
    bytes_sent: usize,
    kbps_sent: usize,
    _stats_start_time: i32,
    /*
     * The state machine
     */
    local_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],
    _peer_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],

    current_state: CurrentState,

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
    last_sent_input: GameInput,
    last_acked_input: GameInput,
    last_send_time: std::time::SystemTime,
    last_recv_time: std::time::SystemTime,
    _shutdown_timeout: u32,
    _disconnect_event_sent: bool,
    disconnect_timeout: u32,
    disconnect_notify_start: u32,
    _disconnect_notify_sent: bool,

    next_send_seq: u16,
    _next_recv_seq: u16,

    /*
     * Rift synchronization.
     */
    timesync: TimeSync,
    /*
     * Event queue
     */
    event_queue: VecDeque<Event>,
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
            last_send_time: std::time::SystemTime::now(),
            _shutdown_timeout: 0,
            disconnect_timeout: 0,
            disconnect_notify_start: 0,
            _disconnect_notify_sent: false,
            _disconnect_event_sent: false,
            _connected: false,
            next_send_seq: 0,
            _next_recv_seq: 0,
            udp: None,

            last_sent_input: Default::default(),
            last_receieved_input: Default::default(),
            last_acked_input: Default::default(),

            state: State::Start,
            _peer_connect_status: [Default::default(); UDP_MSG_MAX_PLAYERS],
            peer_addr: None,
            send_latency: std::env::var("ggpo.network.delay")
                .unwrap_or("".to_string())
                .parse()
                .unwrap_or(0),
            oop_percent: std::env::var("ggpo.oop.percent")
                .unwrap_or("".to_string())
                .parse()
                .unwrap_or(0),
            oo_packet: Default::default(),
            send_queue: VecDeque::with_capacity(64),
            round_trip_time: 0,
            kbps_sent: 0,
            local_connect_status: [Default::default(); UDP_MSG_MAX_PLAYERS],
            current_state: CurrentState::Starting,
            pending_output: VecDeque::with_capacity(64),
            last_recv_time: std::time::SystemTime::now(),
            timesync: Default::default(),
            event_queue: VecDeque::with_capacity(64),
        }
    }
    pub fn init(
        &mut self,
        udp: Mutex<Udp<'a, Callback>>,
        queue: i32,
        addr: SocketAddr,
        status: &[ConnectStatus; UDP_MSG_MAX_PLAYERS],
    ) {
        self.udp = Some(udp);
        self.queue = queue;
        self.local_connect_status = status.clone();

        self.peer_addr = Some(addr);
        self.magic_number = rand::thread_rng().gen();
    }

    pub async fn send_input(&mut self, input: &GameInput) -> Result<(), UdpProtocolError> {
        let udp = self.udp.as_ref().ok_or(UdpProtocolError::UdpUninit)?;

        if self.current_state == CurrentState::Running {
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
            // Implemented the "resize the queue" solution by using a growable VedDeque.
            // Can I use VecDeques for all the Ring Buffers? Need to profile.
            self.pending_output.push_back(input.clone());
        }

        self.send_pending_output().await
    }

    pub async fn send_pending_output(&mut self) -> Result<(), UdpProtocolError> {
        let mut msg = UdpMsg::new(udp_msg::MsgType::Input);
        let mut offset = 0;
        let mut bits = [b'0'; udp_msg::MAX_COMPRESSED_BITS];
        let mut last = GameInput::new();
        if let udp_msg::MsgEnum::Input(mut input) = &mut msg.message {
            if self.pending_output.len() > 0 {
                last = self.last_acked_input;
                bits = input.bits;
                let front = self
                    .pending_output
                    .front()
                    .ok_or(UdpProtocolError::PendingOutputQueueEmpty)?;
                input.start_frame = front.frame;

                assert!(
                    last.frame == None
                        || last.frame.unwrap_or(0) + 1 == input.start_frame.unwrap_or(0)
                );

                for j in 0..self.pending_output.len() {
                    let current: &GameInput = self
                        .pending_output
                        .get(j)
                        .ok_or(UdpProtocolError::PendingOutputItemOOB(j))?;

                    if current.bits != last.bits {
                        assert!(
                            (crate::game_input::GAMEINPUT_MAX_BYTES
                                * crate::game_input::GAMEINPUT_MAX_PLAYERS
                                * 8)
                                < (1 << bitvector::BITVECTOR_NIBBLE_SIZE)
                        );
                        for i in 0..current.size * 8 {
                            assert!(i < (1 << bitvector::BITVECTOR_NIBBLE_SIZE));
                            if current.value(i) != last.value(i) {
                                bitvector::set_bit(&mut input.bits, &mut offset);
                                if current.value(i) {
                                    bitvector::set_bit(&mut bits, &mut offset);
                                } else {
                                    bitvector::clear_bit(&mut bits, &mut offset);
                                }
                                bitvector::write_nibblet(&mut bits, i, &mut offset);
                            }
                        }
                    }
                    bitvector::clear_bit(&mut input.bits, &mut offset);
                    self.last_sent_input = current.clone();
                    last = self.last_sent_input;
                }
            } else {
                input.start_frame = Some(0);
            }
            input.ack_frame = self.last_receieved_input.frame;
            input.num_bits = offset as u16;

            input.disconnect_requested = self.current_state == CurrentState::Disconnected;
            input.peer_connect_status = self.local_connect_status;

            assert!(offset < udp_msg::MAX_COMPRESSED_BITS);
        }
        self.send_msg(&mut msg).await?;

        Ok(())
    }

    pub async fn send_input_ack(&mut self) -> Result<(), UdpProtocolError> {
        let mut msg = UdpMsg::new(udp_msg::MsgType::InputAck);
        if let udp_msg::MsgEnum::InputAck(mut input_ack) = &mut msg.message {
            input_ack.ack_frame = self.last_receieved_input.frame;
        }
        self.send_msg(&mut msg).await?;

        Ok(())
    }

    pub fn get_event(&mut self) -> Option<Event> {
        self.event_queue.pop_front()
    }

    pub async fn send_msg(&mut self, msg: &mut UdpMsg) -> Result<(), UdpProtocolError> {
        self.packets_sent += 1;
        self.last_send_time = std::time::SystemTime::now();
        self.bytes_sent += msg.packet_size();

        msg.header.magic = self.magic_number;
        self.next_send_seq += 1;
        msg.header._sequence_number = self.next_send_seq;

        self.send_queue.push_back(QueueEntry {
            dest_addr: self.peer_addr.ok_or(UdpProtocolError::PeerAddrUninit)?,
            msg: Arc::new(*msg),
            queue_time: std::time::SystemTime::now(),
        });

        self.pump_send_queue().await?;
        Ok(())
    }

    pub async fn on_loop_pool(&mut self, cookie: i32) -> Result<(bool), UdpProtocolError> {
        if self.udp.is_none() {
            return Ok(true);
        }

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        Ok(true)
    }

    pub fn on_input_ack(&mut self, msg: Arc<UdpMsg>) -> Result<(), UdpProtocolError> {
        /*
         * Get rid of our buffered input
         */
        match msg.message {
            udp_msg::MsgEnum::InputAck(input_ack) => {
                while self.pending_output.len() > 0
                    && self
                        .pending_output
                        .front()
                        .ok_or(UdpProtocolError::PendingOutputQueueEmpty)?
                        .frame
                        < input_ack.ack_frame
                {
                    if self
                        .pending_output
                        .front()
                        .ok_or(UdpProtocolError::PendingOutputQueueEmpty)?
                        .frame
                        .is_none()
                    {
                        info!("Throwing away pending output frame: Null Frame\n",);
                    } else {
                        info!(
                            "Throwing away pending output frame: {}\n",
                            self.pending_output
                                .front()
                                .ok_or(UdpProtocolError::PendingOutputQueueEmpty)?
                                .frame
                                .unwrap(),
                        );
                    }
                    self.last_acked_input = self
                        .pending_output
                        .pop_front()
                        .ok_or(UdpProtocolError::PendingOutputQueueEmpty)?;
                }
            }
            _ => (),
        }

        Ok(())
    }

    pub async fn on_quality_report(&mut self, msg: Arc<UdpMsg>) -> Result<(), UdpProtocolError> {
        // send a reply so the other side can compute the round trip transmit time.
        let mut reply = UdpMsg::new(udp_msg::MsgType::QualityReply);
        match (&mut reply.message, msg.message) {
            (udp_msg::MsgEnum::QualityReply(reply), udp_msg::MsgEnum::QualityReport(report)) => {
                reply.pong = report.ping;
            }
            _ => (),
        }
        self.send_msg(&mut reply).await?;

        Ok(())
    }

    pub fn on_quality_reply(&mut self, msg: Arc<UdpMsg>) -> Result<(), UdpProtocolError> {
        let pong = match msg.message {
            udp_msg::MsgEnum::QualityReply(reply) => reply.pong,
            _ => 0,
        };
        self.round_trip_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
            - pong as u128;

        Ok(())
    }

    pub fn on_keep_alive(&self, _: Arc<UdpMsg>) -> Result<(), UdpProtocolError> {
        Ok(())
    }

    pub fn get_network_stats(&self) -> ggpo::NetworkStats {
        ggpo::NetworkStats {
            network: ggpo::Network {
                ping: self.round_trip_time as usize,
                send_queue_len: self.pending_output.len(),
                kbps_sent: self.kbps_sent,
                recv_queue_len: Default::default(),
            },
            timesync: ggpo::TimeSync {
                remote_frames_behind: self.remote_frame_advantage,
                local_frames_behind: self.local_frame_advantage,
            },
        }
    }

    pub fn set_local_frame_number(&mut self, local_frame: Frame) {
        /*
         * Estimate which frame the other guy is one by looking at the
         * last frame they gave us plus some delta for the one-way packet
         * trip time.
         */
        let remoteFrame = self.last_receieved_input.frame.unwrap_or(0)
            + (self.round_trip_time as FrameNum * 60 / 1000);

        /*
         * Our frame advantage is how many frames *behind* the other guy
         * we are.  Counter-intuative, I know.  It's an advantage because
         * it means they'll have to predict more often and our moves will
         * pop more frequenetly.
         */
        self.local_frame_advantage = (remoteFrame as i64 - local_frame.unwrap_or(0) as i64) as i32;
    }

    pub fn recommend_frame_delay(&mut self) -> u32 {
        // XXX: require idle input should be a configuration parameter
        return self.timesync.recommend_frame_wait_duration(false);
    }

    pub fn set_disconnect_timeout(&mut self, timeout: u32) {
        self.disconnect_timeout = timeout;
    }

    pub fn set_disconnect_notify_start(&mut self, timeout: u32) {
        self.disconnect_notify_start = timeout;
    }

    pub async fn pump_send_queue(&mut self) -> Result<(), UdpProtocolError> {
        while !self.send_queue.is_empty() {
            let mut rng = rand::thread_rng();
            let entry = self.send_queue.front().unwrap();

            if self.send_latency > 0 {
                // should really come up with a gaussian distributation based on the configured
                // value, but this will do for now.
                // 2020-7: Below is the gaussian version.
                // TODO: Test other standard deviations.
                let jitter = Normal::new(self.send_latency as f64, 1.)
                    .unwrap()
                    .sample(&mut rng);

                if std::time::SystemTime::now()
                    < self
                        .send_queue
                        .front()
                        .ok_or(UdpProtocolError::SendQueueEmpty)?
                        .queue_time
                        + std::time::Duration::from_millis(jitter as u64)
                {
                    break;
                }
            }

            if self.oop_percent > 0
                && self.oo_packet.msg.is_none()
                && ((rng.gen_range(0, 100)) < self.oop_percent)
            {
                let delay = rng.gen_range(0, self.send_latency * 10 + 1000);
                info!(
                    "creating rogue oop (seq: {} delay: {})\n",
                    entry.msg.header._sequence_number, delay
                );
                self.oo_packet.send_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("Time travel is afoot")
                    .as_millis();
                self.oo_packet.msg = Some(entry.msg.clone());
                self.oo_packet.dest_addr = entry.dest_addr;
            } else {
                // TODO: figure out what exactly this assert wants to check for.
                // assert!(entry.dest_addr)

                self.udp
                    .as_mut()
                    .ok_or(UdpProtocolError::UdpUninit)?
                    .get_mut()
                    .send_to(entry.msg.clone(), &[entry.dest_addr])
                    .await?;
            }
            self.send_queue.pop_front();
        }
        if self.oo_packet.msg.is_some()
            && self.oo_packet.send_time
                < std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis()
        {
            info!("Sending rogue oop!");
            self.udp
                .as_mut()
                .ok_or(UdpProtocolError::UdpUninit)?
                .get_mut()
                .send_to(
                    self.oo_packet
                        .msg
                        .as_ref()
                        .ok_or(UdpProtocolError::OOPacketMsgUninit)?
                        .clone(),
                    &[self.oo_packet.dest_addr],
                )
                .await?;
            self.oo_packet.msg = None;
        }

        Ok(())
    }

    pub fn clear_send_queue(&mut self) {
        self.send_queue.clear();
    }
}
