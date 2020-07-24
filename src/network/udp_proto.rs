use crate::{
    bitvector,
    game_input::{Frame, FrameNum, GameInput, GAMEINPUT_MAX_BYTES, GAMEINPUT_MAX_PLAYERS},
    ggpo,
    network::{
        udp::{Udp, UdpCallback, UdpError},
        udp_msg::{
            ConnectStatus, MsgEnum, MsgType, UdpMsg, MAX_COMPRESSED_BITS, UDP_MSG_MAX_PLAYERS,
        },
    },
    time_sync::TimeSync,
};
// use async_mutex::Mutex;
use log::{error, info, trace};
use parking_lot::Mutex;
use rand::prelude::*;
use rand_distr::{Distribution, Normal};
use std::sync::Arc;
use thiserror::Error;

use std::{
    collections::VecDeque,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

pub const UDP_HEADER_SIZE: usize = 28; /* Size of IP + UDP headers */
pub const NUM_SYNC_PACKETS: u32 = 5;
pub const SYNC_RETRY_INTERVAL: u128 = 2000;
pub const SYNC_FIRST_RETRY_INTERVAL: u128 = 500;
pub const RUNNING_RETRY_INTERVAL: u128 = 200;
pub const KEEP_ALIVE_INTERVAL: i32 = 200;
pub const QUALITY_REPORT_INTERVAL: u128 = 1000;
pub const NETWORK_STATS_INTERVAL: u128 = 1000;
pub const UDP_SHUTDOWN_TIMER: u128 = 5000;
pub const MAX_SEQ_DISTANCE: u16 = 1 << 15;

#[derive(Debug, Error)]
pub enum UdpProtoError {
    #[error("Curent frame is None.")]
    CurrentFrameUninit,
    #[error("Last recieved input frame is None")]
    InputFrameUninit,
    #[error("Input start frame is None.")]
    StartFrameUninit,
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
        source: UdpError,
    },
    #[error("Peer address is uninitialized.")]
    PeerAddrUninit,
    #[error("OO Packet's msg uninitalized")]
    OOPacketMsgUninit,
    #[error("System Time error has been reported, please double check system clocks.")]
    SystemTime {
        #[from]
        source: SystemTimeError,
    },
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct Synchronizing {
    pub total: u32,
    pub count: u32,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct NetworkInterrupted {
    pub disconnect_timeout: u128,
}

#[derive(Debug)]
pub enum Event {
    Unknown,
    Connected,
    Synchronizing(Synchronizing),
    Synchronzied,
    Input(GameInput),
    Disconnected,
    NetworkInterrupted(NetworkInterrupted),
    NetworkResumed,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct Syncing {
    pub roundtrips_remaining: u32,
    pub random: u32,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct Running {
    pub last_quality_report_time: u128,
    pub last_network_stats_interval: u128,
    pub last_input_packet_recv_time: u128,
}

// impl Running {
//     pub fn new() -> Result<Self, UdpProtoError> {
//         let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
//         Ok(Self {
//             last_quality_report_time: now,
//             last_network_stats_interval: now,
//             last_input_packet_recv_time: now,
//         })
//     }
// }

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum State {
    Syncing(Syncing),
    Synchronized,
    Running(Running),
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

#[derive(Debug, Copy, Clone)]
enum LogPrefix {
    Send,
    Recv,
    RecvRejecting,
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

pub struct UdpProtocol<T: UdpCallback + Send + Sync> {
    // RNG
    rng: StdRng,
    /*
     * Network transmission information
     */
    udp: Option<Arc<Mutex<Udp<T>>>>,
    peer_addr: Option<SocketAddr>,
    magic_number: u16,
    //TODO: Make queue's type consistent, and more importantly is the -1 init value relevant.
    queue: i64,
    remote_magic_number: u16,
    connected: bool,
    send_latency: i32,
    oop_percent: i32,
    oo_packet: OoPacket,
    send_queue: VecDeque<QueueEntry>,
    /*
     * Stats
     */
    round_trip_time: u128,
    packets_sent: usize,
    bytes_sent: usize,
    kbps_sent: usize,
    stats_start_time: u128,
    /*
     * The state machine
     */
    local_connect_status: [Arc<Mutex<ConnectStatus>>; UDP_MSG_MAX_PLAYERS],
    peer_connect_status: [ConnectStatus; UDP_MSG_MAX_PLAYERS],

    state: State,

    // state: State,

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
    last_received_input: GameInput,
    last_sent_input: GameInput,
    last_acked_input: GameInput,
    last_send_time: std::time::SystemTime,
    last_recv_time: std::time::SystemTime,
    shutdown_timeout: u128,
    disconnect_event_sent: bool,
    disconnect_timeout: u128,
    disconnect_notify_start: u128,
    disconnect_notify_sent: bool,

    next_send_seq: u16,
    next_recv_seq: u16,

    /*
     * Rift synchronization.
     */
    timesync: TimeSync,
    /*
     * Event queue
     */
    event_queue: VecDeque<Event>,
}

impl<Callback: UdpCallback + Send + Sync> UdpProtocol<Callback> {
    pub fn new() -> Self {
        let mut connect_status: [Arc<Mutex<ConnectStatus>>; UDP_MSG_MAX_PLAYERS] =
            Default::default();
        for i in 0..UDP_MSG_MAX_PLAYERS {
            connect_status[i] = Arc::new(Mutex::new(ConnectStatus::new()));
        }
        Self {
            rng: StdRng::from_entropy(),
            local_frame_advantage: 0,
            remote_frame_advantage: 0,
            queue: -1,
            magic_number: 0,
            remote_magic_number: 0,
            packets_sent: 0,
            bytes_sent: 0,
            stats_start_time: 0,
            last_send_time: std::time::SystemTime::now(),
            shutdown_timeout: 0,
            disconnect_timeout: 0,
            disconnect_notify_start: 0,
            disconnect_notify_sent: false,
            disconnect_event_sent: false,
            connected: false,
            next_send_seq: 0,
            next_recv_seq: 0,
            udp: None,

            last_sent_input: Default::default(),
            last_received_input: Default::default(),
            last_acked_input: Default::default(),

            // state: State::Start,
            peer_connect_status: [Default::default(); UDP_MSG_MAX_PLAYERS],
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
            local_connect_status: connect_status,
            state: State::Starting,
            pending_output: VecDeque::with_capacity(64),
            last_recv_time: std::time::SystemTime::now(),
            timesync: Default::default(),
            event_queue: VecDeque::with_capacity(64),
        }
    }
    pub fn init(
        &mut self,
        udp: Arc<Mutex<Udp<Callback>>>,
        // poll: Arc<Mutex<Poll>>,
        queue: u32,
        addr: SocketAddr,
        status: &[Arc<Mutex<ConnectStatus>>; UDP_MSG_MAX_PLAYERS],
    ) {
        self.udp = Some(udp);
        self.queue = queue as i64;
        self.local_connect_status = status.clone();

        self.peer_addr = Some(addr);
        self.magic_number = self.rng.gen();
        // poll.lock().registry().register(source, token, interests)
    }

    pub fn send_input(&mut self, input: &GameInput) -> Result<(), UdpProtoError> {
        // let udp = self.udp.as_ref().ok_or(UdpProtoError::UdpUninit)?;

        match self.state {
            State::Running(Running {
                last_quality_report_time: _,
                last_network_stats_interval: _,
                last_input_packet_recv_time: _,
            }) => {
                /*
                 * Check to see if this is a good time to adjust for the rift...
                 */
                self.timesync.advance_frame(
                    input,
                    self.local_frame_advantage,
                    self.remote_frame_advantage,
                );

                /*
                 * Save this input packet
                 *
                 * XXX: This queue may fill up for spectators who do not ack input packets in a timely
                 * manner.  When this happens, we can either resize the queue (ug) or disconnect them
                 * (better, but still ug).  For the meantime, make this queue really big to decrease
                 * the odds of this happening...
                 */
                self.pending_output.push_back(input.clone());
            }
            _ => {}
        }

        self.send_pending_output()
    }

    pub fn send_pending_output(&mut self) -> Result<(), UdpProtoError> {
        let mut msg = UdpMsg::new(MsgType::Input);
        let mut offset = 0;
        let mut bits: [u8; MAX_COMPRESSED_BITS];
        let mut last: GameInput;
        if let MsgEnum::Input(mut input) = &mut msg.message {
            if self.pending_output.len() > 0 {
                last = self.last_acked_input;
                bits = input.bits;
                let front = self
                    .pending_output
                    .front()
                    .ok_or(UdpProtoError::PendingOutputQueueEmpty)?;
                input.start_frame = front.frame;

                assert!(
                    last.frame == None
                        || last.frame.unwrap_or(0) + 1 == input.start_frame.unwrap_or(0)
                );
                for current in self.pending_output.iter() {
                    if current.bits != last.bits {
                        assert!(
                            (GAMEINPUT_MAX_BYTES
                                * GAMEINPUT_MAX_PLAYERS
                                * bitvector::BITVECTOR_NIBBLE_SIZE)
                                < (1 << bitvector::BITVECTOR_NIBBLE_SIZE)
                        );

                        for i in 0..current.size * bitvector::BITVECTOR_NIBBLE_SIZE {
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
            input.ack_frame = self.last_received_input.frame;
            input.num_bits = offset as u16;

            input.disconnect_requested = self.state == State::Disconnected;
            for i in 0..self.local_connect_status.len() {
                input.peer_connect_status[i] = *self.local_connect_status[i].lock();
            }

            assert!(offset < MAX_COMPRESSED_BITS);
        }
        self.send_msg(&mut msg)
    }

    pub fn send_input_ack(&mut self) -> Result<(), UdpProtoError> {
        let mut msg = UdpMsg::new(MsgType::InputAck);
        if let MsgEnum::InputAck(mut input_ack) = &mut msg.message {
            input_ack.ack_frame = self.last_received_input.frame;
        }
        self.send_msg(&mut msg)
    }

    pub fn is_initialized(&self) -> bool {
        self.udp.is_some()
    }

    pub fn is_sychronized(&self) -> bool {
        match self.state {
            State::Synchronized => true,
            // TODO: Is this supposed to report true?
            // State::Running(_) => true,
            _ => false,
        }
    }

    pub fn is_running(&self) -> bool {
        match self.state {
            State::Running(_) => true,
            _ => false,
        }
    }

    pub fn get_event(&mut self, event: &mut Event) -> bool {
        if let Some(e) = self.event_queue.pop_front() {
            *event = e;
            return true;
        }
        false
    }

    pub fn on_loop_poll(&mut self, _cookie: i32) -> Result<bool, UdpProtoError> {
        if self.udp.is_none() {
            return Err(UdpProtoError::UdpUninit);
        }

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        self.pump_send_queue()?;

        match self.state {
            State::Syncing(Syncing {
                roundtrips_remaining,
                random: _,
            }) => {
                let next_interval = if roundtrips_remaining == NUM_SYNC_PACKETS {
                    SYNC_FIRST_RETRY_INTERVAL
                } else {
                    SYNC_RETRY_INTERVAL
                };

                if self.last_send_time.duration_since(UNIX_EPOCH)?.as_millis() > 0
                    && self.last_send_time.duration_since(UNIX_EPOCH)?.as_millis() + next_interval
                        < now
                {
                    info!(
                        "No luck syncing after {} ms... Re-queueing sync packet.\n",
                        next_interval
                    );
                    self.send_sync_request()?;
                }
            }
            State::Synchronized => {}
            State::Running(Running {
                mut last_quality_report_time,
                mut last_network_stats_interval,
                mut last_input_packet_recv_time,
            }) => {
                // xxx: rig all this up with a timer wrapper
                if !(last_input_packet_recv_time > 0)
                    || last_input_packet_recv_time + RUNNING_RETRY_INTERVAL < now
                {
                    info!("Haven't exchanged packets in a while (last received:{:?}  last sent:{:?}).  Resending.\n", self.last_received_input.frame, self.last_sent_input.frame);
                    self.send_pending_output()?;
                    last_input_packet_recv_time = now;
                    self.state = State::Running(Running {
                        last_quality_report_time,
                        last_network_stats_interval,
                        last_input_packet_recv_time: now,
                    });
                }

                if !(last_quality_report_time > 0)
                    || last_quality_report_time + QUALITY_REPORT_INTERVAL < now
                {
                    let mut msg = UdpMsg::new(MsgType::QualityReport);
                    match &mut msg.message {
                        MsgEnum::QualityReport(quality_report) => {
                            quality_report.ping =
                                SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
                            // TODO: Profile and test whether i8 is enough here in extreme cases.
                            quality_report.frame_advantage = self.local_frame_advantage as i8;
                            self.send_msg(&mut msg)?;
                        }
                        _ => {}
                    }
                    last_quality_report_time = now;
                    self.state = State::Running(Running {
                        last_quality_report_time: now,
                        last_network_stats_interval,
                        last_input_packet_recv_time,
                    });
                }

                if !(last_network_stats_interval > 0)
                    || last_network_stats_interval + NETWORK_STATS_INTERVAL < now
                {
                    self.update_network_stats()?;
                    last_network_stats_interval = now;
                    self.state = State::Running(Running {
                        last_quality_report_time,
                        last_network_stats_interval: last_network_stats_interval,
                        last_input_packet_recv_time,
                    })
                }

                if !(self.last_send_time.duration_since(UNIX_EPOCH)?.as_millis() > 0)
                    || self.last_send_time.duration_since(UNIX_EPOCH)?.as_millis()
                        + NETWORK_STATS_INTERVAL
                        < now
                {
                    info!("Sending keep alive packet.\n");
                    self.send_msg(&mut UdpMsg::new(MsgType::KeepAlive))?;
                }
                if self.disconnect_timeout > 0
                    && self.disconnect_notify_start > 0
                    && !self.disconnect_notify_sent
                    && (self.last_recv_time.duration_since(UNIX_EPOCH)?.as_millis()
                        + self.disconnect_notify_start
                        < now)
                {
                    info!("Endpoint has stopped receiving packets for {:?} ms. Sending notification.\n", self.disconnect_notify_start);
                    let event = Event::NetworkInterrupted(NetworkInterrupted {
                        disconnect_timeout: self.disconnect_timeout - self.disconnect_notify_start,
                    });

                    self.queue_event(event);
                    self.disconnect_notify_sent = true;
                }

                if self.disconnect_timeout > 0
                    && (self.last_recv_time.duration_since(UNIX_EPOCH)?.as_millis()
                        + self.disconnect_timeout
                        < now)
                {
                    if !self.disconnect_event_sent {
                        info!(
                            "Endpoint has stopped receiving packets for {:?} ms. Disconnecting.\n",
                            self.disconnect_timeout
                        );
                        self.queue_event(Event::Disconnected);
                        self.disconnect_event_sent = true;
                    }
                }
            }
            State::Disconnected => {
                if (self.shutdown_timeout as u128) < now {
                    info!("Shutting down udp connection.\n");
                    self.udp = None;
                    self.shutdown_timeout = 0;
                }
            }
            State::Starting => {}
        }

        Ok(true)
    }

    pub fn disconnect(&mut self) -> Result<(), UdpProtoError> {
        self.state = State::Disconnected;
        self.shutdown_timeout =
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() + UDP_SHUTDOWN_TIMER;
        Ok(())
    }

    pub fn send_sync_request(&mut self) -> Result<(), UdpProtoError> {
        match &mut self.state {
            State::Syncing(Syncing {
                roundtrips_remaining: _,
                random,
            }) => {
                *random = self.rng.gen::<u32>() & 0xFFFF;
                let mut msg = UdpMsg::new(MsgType::SyncRequest);
                match &mut msg.message {
                    MsgEnum::SyncRequest(sync_request) => {
                        sync_request.random_request = *random;
                    }
                    _ => {}
                }
                return self.send_msg(&mut msg);
            }
            _ => {}
        }
        Ok(())
    }

    pub fn send_msg(&mut self, msg: &mut UdpMsg) -> Result<(), UdpProtoError> {
        self.log_msg(LogPrefix::Send, msg);
        self.packets_sent += 1;
        self.last_send_time = std::time::SystemTime::now();
        self.bytes_sent += msg.packet_size();

        msg.header.magic = self.magic_number;
        self.next_send_seq += 1;
        msg.header.sequence_number = self.next_send_seq;

        self.send_queue.push_back(QueueEntry {
            dest_addr: self.peer_addr.ok_or(UdpProtoError::PeerAddrUninit)?,
            msg: Arc::new(*msg),
            queue_time: std::time::SystemTime::now(),
        });

        self.pump_send_queue()
    }

    pub fn handles_msg(&mut self, from: &SocketAddr, _msg: &UdpMsg) -> Result<bool, UdpProtoError> {
        if self.udp.is_none() {
            return Err(UdpProtoError::UdpUninit);
        }
        Ok(self.peer_addr.ok_or(UdpProtoError::PeerAddrUninit)? == *from)
    }

    pub fn on_msg(&mut self, msg: &UdpMsg) -> Result<(), UdpProtoError> {
        let mut handled = false;

        // filter out messages that don't match what we expect
        let seq = msg.header.sequence_number;
        if msg.header.packet_type != MsgType::SyncRequest
            && msg.header.packet_type != MsgType::SyncReply
        {
            if msg.header.magic != self.remote_magic_number {
                self.log_msg(LogPrefix::RecvRejecting, msg);
                return Ok(());
            }

            // filter out out-of-order packets
            let skipped: u16 = seq - self.next_recv_seq;
            // below was commented out in the original code, presumably for debugging purposes,
            trace!(
                "checking sequence number -> next - seq : {:?} - {:?} = {:?}\n",
                seq,
                self.next_recv_seq,
                skipped
            );
            if skipped > MAX_SEQ_DISTANCE {
                info!(
                    "dropping out of order packet (seq: {:?}, last seq:{:?})\n",
                    seq, self.next_recv_seq
                );
                return Ok(());
            }
        }
        self.next_recv_seq = seq;
        self.log_msg(LogPrefix::Recv, msg);
        if msg.header.packet_type > MsgType::InputAck || msg.header.packet_type == MsgType::Invalid
        {
            self.on_invalid(msg)?;
        } else {
            handled = match msg.header.packet_type {
                MsgType::SyncRequest => self.on_sync_request(msg)?,
                MsgType::Invalid => self.on_invalid(msg)?,
                MsgType::SyncReply => self.on_sync_reply(msg)?,
                MsgType::Input => self.on_input(msg)?,
                MsgType::QualityReport => self.on_quality_report(msg)?,
                MsgType::QualityReply => self.on_quality_reply(msg)?,
                MsgType::KeepAlive => self.on_keep_alive(msg)?,
                MsgType::InputAck => self.on_input_ack(msg)?,
            }
        }

        if handled {
            self.last_recv_time = SystemTime::now();
            match self.state {
                State::Running(Running {
                    last_quality_report_time: _,
                    last_network_stats_interval: _,
                    last_input_packet_recv_time: _,
                }) => {
                    if self.disconnect_notify_sent {
                        self.queue_event(Event::NetworkResumed);
                        self.disconnect_notify_sent = false;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub fn update_network_stats(&mut self) -> Result<(), UdpProtoError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        if self.stats_start_time == 0 {
            self.stats_start_time = now;
        }

        let total_bytes_sent = self.bytes_sent + (UDP_HEADER_SIZE * self.packets_sent);
        let seconds = (now - self.stats_start_time) as f64 / 1000.;
        let bytes_per_second = total_bytes_sent as f64 / seconds;
        let udp_overhead = 100. * ((UDP_HEADER_SIZE * self.packets_sent) / self.bytes_sent) as f64;

        self.kbps_sent = (bytes_per_second / 1024.) as usize;

        info!(
            "Network Stats -- Bandwidth: {:.2} KBps   Packets Sent: {:5.}%5d ({:.2} pps) 
       KB Sent: {:.2}    UDP Overhead: {:.2} %%.\n",
            self.kbps_sent,
            self.packets_sent,
            (self.packets_sent * 1000) as u128 / (now - self.stats_start_time),
            total_bytes_sent as f64 / 1024.0,
            udp_overhead
        );
        Ok(())
    }

    pub fn queue_event(&mut self, event: Event) {
        info!("Queueing Event {:?}", event);
        self.event_queue.push_back(event);
    }

    pub fn synchronize(&mut self) -> Result<(), UdpProtoError> {
        self.udp.as_ref().ok_or(UdpProtoError::UdpUninit)?;
        self.state = State::Syncing(Syncing {
            roundtrips_remaining: NUM_SYNC_PACKETS,
            random: self.rng.gen(),
        });
        self.send_sync_request()
    }

    pub fn get_peer_connect_status(&self, id: usize) -> (Frame, bool) {
        return (
            self.peer_connect_status[id].last_frame,
            !self.peer_connect_status[id].disconnected,
        );
    }

    fn log_msg(&self, prefix: LogPrefix, msg: &UdpMsg) {
        match msg.message {
            MsgEnum::SyncRequest(sync_request) => info!(
                "{:?} sync-request ({:?}).\n",
                prefix, sync_request.random_request
            ),
            MsgEnum::SyncReply(sync_reply) => {
                info!("{:?} sync-reply ({:?}).\n", prefix, sync_reply.random_reply)
            }
            MsgEnum::QualityReport(_) => info!("{:?} quality report.\n", prefix),
            MsgEnum::QualityReply(_) => info!("{:?} quality reply.\n", prefix),
            MsgEnum::Input(input) => info!(
                "{:?} game-compressed-input {:?} (+ {:?} bits).\n",
                prefix, input.start_frame, input.num_bits
            ),
            MsgEnum::InputAck(_) => {}
            MsgEnum::None => {
                error!("Unknown UdpMsg type.");
                unreachable!();
            }
            MsgEnum::KeepAlive => info!("{:?} keep alive.\n", prefix),
        };
    }

    pub fn on_invalid(&mut self, _msg: &UdpMsg) -> Result<bool, UdpProtoError> {
        error!("Invalid msg in UdpProtocol.\n");
        // panic!();
        // TODO :Is panicing here a good idea? Is it possible to recover?
        Ok(false)
    }

    pub fn on_sync_request(&mut self, msg: &UdpMsg) -> Result<bool, UdpProtoError> {
        if self.remote_magic_number != 0 && msg.header.magic != self.remote_magic_number {
            info!(
                "Ignoring sync request from unknown endpoint ({:?} != {:?}.\n",
                msg.header.magic, self.remote_magic_number
            );
            return Ok(false);
        }
        let mut reply = UdpMsg::new(MsgType::SyncReply);
        match (&mut reply.message, msg.message) {
            (MsgEnum::SyncReply(sync_reply), MsgEnum::SyncRequest(sync_request)) => {
                sync_reply.random_reply = sync_request.random_request;
            }
            _ => {}
        }
        self.send_msg(&mut reply)?;
        Ok(true)
    }

    pub fn on_sync_reply(&mut self, msg: &UdpMsg) -> Result<bool, UdpProtoError> {
        match self.state {
            State::Syncing(syncing) => match msg.message {
                MsgEnum::SyncReply(sync_reply) => {
                    if sync_reply.random_reply != syncing.random {
                        info!(
                            "sync reply {:?} != {:?}.  Keep looking...\n",
                            sync_reply.random_reply, syncing.random
                        );
                        return Ok(false);
                    }
                    if !self.connected {
                        self.queue_event(Event::Connected);
                        self.connected = true;
                    }

                    info!(
                        "Checking sync state ({:?} round trips remaining).\n",
                        syncing.roundtrips_remaining
                    );
                    if syncing.roundtrips_remaining - 1 == 0 {
                        info!("Synchronized!\n");
                        self.queue_event(Event::Synchronzied);
                        self.state = State::Running(Default::default());
                        self.last_received_input.frame = None;
                        self.remote_magic_number = msg.header.magic;
                    } else {
                        let event = Event::Synchronizing(Synchronizing {
                            total: NUM_SYNC_PACKETS,
                            count: NUM_SYNC_PACKETS - syncing.roundtrips_remaining,
                        });
                        self.queue_event(event);
                        self.send_sync_request()?;
                    }
                    return Ok(true);
                }
                _ => {}
            },
            _ => {
                info!("Ignoring SyncReply while not synching.\n");
                return Ok(msg.header.magic == self.remote_magic_number);
            }
        }

        Ok(true)
    }

    pub fn on_input(&mut self, msg: &UdpMsg) -> Result<bool, UdpProtoError> {
        /*
         * If a disconnect is requested, go ahead and disconnect now.
         */
        match msg.message {
            MsgEnum::Input(input) => {
                let disconnect_requested = input.disconnect_requested;
                if disconnect_requested {
                    if self.state != State::Disconnected && !self.disconnect_event_sent {
                        info!("Disconnecting endpoint on remote request.\n");
                        self.queue_event(Event::Disconnected);
                        self.disconnect_event_sent = true;
                    }
                } else {
                    /*
                     * Update the peer connection status if this peer is still considered to be part
                     * of the network.
                     */
                    let remote_status = input.peer_connect_status;
                    for i in 0..self.peer_connect_status.len() {
                        assert!(
                            remote_status[i].last_frame >= self.peer_connect_status[i].last_frame
                        );
                        self.peer_connect_status[i].disconnected = self.peer_connect_status[i]
                            .disconnected
                            || remote_status[i].disconnected;
                        self.peer_connect_status[i].last_frame = std::cmp::max(
                            self.peer_connect_status[i].last_frame,
                            remote_status[i].last_frame,
                        );
                    }
                }

                /*
                 * Decompress the input.
                 */
                let last_received_frame_number = self.last_received_input.frame;
                if input.num_bits > 0 {
                    let offset = 0;
                    let mut bits = input.bits;
                    let num_bits = input.num_bits;
                    let mut current_frame = input.start_frame;
                    self.last_received_input.size = input.bits.len();
                    if self.last_received_input.frame.is_none() {
                        self.last_received_input.frame =
                            Some(input.start_frame.ok_or(UdpProtoError::StartFrameUninit)? - 1);
                    }
                    while offset < num_bits {
                        /*
                         * Keep walking through the frames (parsing bits) until we reach
                         * the inputs for the frame right after the one we're on.
                         */
                        assert!(
                            current_frame
                                <= Some(
                                    self.last_received_input
                                        .frame
                                        .ok_or(UdpProtoError::InputFrameUninit)?
                                        + 1
                                )
                        );
                        let use_inputs = current_frame
                            == Some(
                                self.last_received_input
                                    .frame
                                    .ok_or(UdpProtoError::InputFrameUninit)?
                                    + 1,
                            );
                        while bitvector::read_bit(&mut bits, &mut (offset as usize)) > 0 {
                            let on = bitvector::read_bit(&mut bits, &mut (offset as usize));
                            let button = bitvector::read_nibblet(&mut bits, &mut (offset as usize));
                            if use_inputs {
                                // TODO: Fix the 1d -> 2d indexing going on here.
                                if on > 0 {
                                    self.last_received_input.set(button as usize);
                                } else {
                                    self.last_received_input.clear(button as usize);
                                }
                            }
                        }
                        assert!(offset <= num_bits);

                        /*
                         * Now if we want to use these inputs, go ahead and send them to
                         * the emulator.
                         */
                        if use_inputs {
                            /*
                             * Move forward 1 frame in the stream.
                             */
                            let desc: String;
                            assert!(
                                current_frame
                                    == Some(
                                        self.last_received_input
                                            .frame
                                            .ok_or(UdpProtoError::InputFrameUninit)?
                                            + 1
                                    )
                            );
                            self.last_received_input.frame = current_frame;

                            /*
                             * Send the event to the emualtor
                             */
                            let event = Event::Input(self.last_received_input);
                            desc = self.last_received_input.describe(true);

                            match &mut self.state {
                                State::Running(running) => {
                                    running.last_input_packet_recv_time =
                                        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
                                }
                                _ => {
                                    error!("Trying to update state machine for running state, but not running.");
                                }
                            }
                            info!(
                                "Sending frame {:?} to emu queue {:?} ({:?}).\n",
                                self.last_received_input.frame, self.queue, desc
                            );
                            self.queue_event(event);
                        } else {
                            info!(
                                "Skipping past frame:({:?}) current is {:?}.\n",
                                current_frame, self.last_received_input.frame
                            )
                        }
                        /*
                         * Move forward 1 frame in the input stream.
                         */
                        current_frame =
                            Some(current_frame.ok_or(UdpProtoError::CurrentFrameUninit)? + 1);
                    }
                }

                assert!(self.last_received_input.frame >= last_received_frame_number);

                /*
                 * Get rid of our buffered input
                 */
                while self.pending_output.len() > 0
                    && self
                        .pending_output
                        .front()
                        .ok_or(UdpProtoError::PendingOutputQueueEmpty)?
                        .frame
                        < input.ack_frame
                {
                    info!(
                        "Throwing away pending output frame {:?}\n",
                        self.pending_output
                            .front()
                            .ok_or(UdpProtoError::PendingOutputQueueEmpty)?
                            .frame
                    );
                    self.last_acked_input = self
                        .pending_output
                        .pop_front()
                        .ok_or(UdpProtoError::PendingOutputQueueEmpty)?;
                }
            }
            _ => {}
        }

        Ok(true)
    }

    pub fn on_input_ack(&mut self, msg: &UdpMsg) -> Result<bool, UdpProtoError> {
        /*
         * Get rid of our buffered input
         */
        match msg.message {
            MsgEnum::InputAck(input_ack) => {
                while self.pending_output.len() > 0
                    && self
                        .pending_output
                        .front()
                        .ok_or(UdpProtoError::PendingOutputQueueEmpty)?
                        .frame
                        < input_ack.ack_frame
                {
                    info!(
                        "Throwing away pending output frame: {:?}\n",
                        self.pending_output
                            .front()
                            .ok_or(UdpProtoError::PendingOutputQueueEmpty)?
                            .frame,
                    );
                    self.last_acked_input = self
                        .pending_output
                        .pop_front()
                        .ok_or(UdpProtoError::PendingOutputQueueEmpty)?;
                }
            }
            _ => (),
        }

        Ok(true)
    }

    pub fn on_quality_report(&mut self, msg: &UdpMsg) -> Result<bool, UdpProtoError> {
        // send a reply so the other side can compute the round trip transmit time.
        let mut reply = UdpMsg::new(MsgType::QualityReply);
        match (&mut reply.message, msg.message) {
            (MsgEnum::QualityReply(reply), MsgEnum::QualityReport(report)) => {
                reply.pong = report.ping;
            }
            _ => (),
        }
        self.send_msg(&mut reply)?;
        return Ok(true);
    }

    pub fn on_quality_reply(&mut self, msg: &UdpMsg) -> Result<bool, UdpProtoError> {
        let pong = match msg.message {
            MsgEnum::QualityReply(reply) => reply.pong,
            _ => 0,
        };
        self.round_trip_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
            - pong as u128;

        Ok(true)
    }

    pub fn on_keep_alive(&self, _: &UdpMsg) -> Result<bool, UdpProtoError> {
        Ok(true)
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

    pub fn set_local_frame_number(&mut self, local_frame: FrameNum) {
        /*
         * Estimate which frame the other guy is one by looking at the
         * last frame they gave us plus some delta for the one-way packet
         * trip time.
         */
        let remote_frame = self.last_received_input.frame.unwrap_or(0)
            + (self.round_trip_time as FrameNum * 60 / 1000);

        /*
         * Our frame advantage is how many frames *behind* the other guy
         * we are.  Counter-intuative, I know.  It's an advantage because
         * it means they'll have to predict more often and our moves will
         * pop more frequenetly.
         */
        self.local_frame_advantage = (remote_frame as i64 - local_frame as i64) as i32;
    }

    pub fn recommend_frame_delay(&mut self) -> u32 {
        // XXX: require idle input should be a configuration parameter
        return self.timesync.recommend_frame_wait_duration(false);
    }

    pub fn set_disconnect_timeout(&mut self, timeout: u128) {
        self.disconnect_timeout = timeout;
    }

    pub fn set_disconnect_notify_start(&mut self, timeout: u128) {
        self.disconnect_notify_start = timeout;
    }

    pub fn pump_send_queue(&mut self) -> Result<(), UdpProtoError> {
        while !self.send_queue.is_empty() {
            let entry = self.send_queue.front().unwrap();

            if self.send_latency > 0 {
                // should really come up with a gaussian distributation based on the configured
                // value, but this will do for now.
                // 2020-7: Below is the gaussian version.
                // TODO: Test other standard deviations.
                let jitter = Normal::new(self.send_latency as f64, 3.)
                    .unwrap()
                    .sample(&mut StdRng::seed_from_u64(self.rng.gen()));

                if std::time::SystemTime::now()
                    < self
                        .send_queue
                        .front()
                        .ok_or(UdpProtoError::SendQueueEmpty)?
                        .queue_time
                        + std::time::Duration::from_millis(jitter as u64)
                {
                    break;
                }
            }

            if self.oop_percent > 0
                && self.oo_packet.msg.is_none()
                && ((self.rng.gen_range(0, 100)) < self.oop_percent)
            {
                let delay = self.rng.gen_range(0, self.send_latency * 10 + 1000);
                info!(
                    "creating rogue oop (seq: {} delay: {})\n",
                    entry.msg.header.sequence_number, delay
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
                    .ok_or(UdpProtoError::UdpUninit)?
                    .lock()
                    .send_to(entry.msg.clone(), &entry.dest_addr)?;
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
                .ok_or(UdpProtoError::UdpUninit)?
                .lock()
                .send_to(
                    self.oo_packet
                        .msg
                        .as_ref()
                        .ok_or(UdpProtoError::OOPacketMsgUninit)?
                        .clone(),
                    &self.oo_packet.dest_addr,
                )?;
            self.oo_packet.msg = None;
        }

        Ok(())
    }

    pub fn clear_send_queue(&mut self) {
        self.send_queue.clear();
    }
}
