#![feature(const_fn)]
#![feature(const_panic)]
// #![feature(const_generics)]
#![feature(slice_fill)]
// #![feature(const_in_array_repeat_expressions)]
// #![feature(move_ref_pattern)]
#![warn(clippy::all)]
#![forbid(unsafe_code)]

pub mod backends {
    pub mod p2p;
    pub mod spectator;
    pub mod sync_test;
}
pub mod game_input;
pub mod ggpo;
pub mod input_queue;
pub mod network {
    pub mod udp;
    pub mod udp_msg;
    pub mod udp_proto;
}
pub mod bitvector;
pub mod player;
pub mod sync;
pub mod time_sync;
