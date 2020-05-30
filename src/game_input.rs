use log::{info, warn};
use std::slice;

// GAMEINPUT_MAX_BYTES * GAMEINPUT_MAX_PLAYERS * 8 must be less than
// 2^BITVECTOR_NIBBLE_SIZE (see bitvector.h)

const MAX_BYTES: usize = 9;
const MAX_PLAYERS: usize = 2;

#[derive(Debug)]
pub struct GameInput {
    frame: i32,
    size: usize,
    bits: [u8; MAX_BYTES * MAX_PLAYERS],
}

impl GameInput {
    fn init(frame: i32, i_bits: Option<&[u8; MAX_BYTES * MAX_PLAYERS]>, size: usize) -> GameInput {
        assert!(size <= MAX_BYTES);
        match i_bits {
            Some(input_bits) => GameInput {
                frame,
                size,
                bits: input_bits.clone(),
            },
            None => GameInput {
                frame,
                size,
                bits: [b'0'; MAX_BYTES * MAX_PLAYERS],
            },
        }
    }
    const fn value(&self, i: usize) -> bool {
        (self.bits[i / 8] & (1 << (i % 8))) != 0
    }
    fn set(mut self, i: usize) {
        self.bits[i / 8] |= (1 << (i % 8));
    }
    fn clear(mut self, i: usize) {
        self.bits[i / 8] &= !(1 << (i % 8));
    }
    fn erase(mut self) {
        self.bits = [b'0'; MAX_BYTES * MAX_PLAYERS];
    }
    fn describe(&self, show_frame: bool) -> String {
        let mut buf: String;
        if show_frame {
            buf = format!("(frame:{} size:{}", self.frame, self.size);
        } else {
            buf = format!("(size:{}", self.size);
        }
        for i in 0..(self.size as usize) * 8 {
            if self.value(i) {
                let buf2 = format!("{:2}", i);
                buf.push_str(&buf2);
            }
        }
        buf + ")"
    }
    fn log(prefix: *mut [u8], show_frame: bool) {}
    fn equal(&self, other: &GameInput, bitsonly: bool) -> bool {
        if (!bitsonly && self.frame != other.frame) {
            info!("frames don't match: {}, {}\n", self.frame, other.frame,);
        }
        if self.size != other.size {
            info!("sizes don't match: {}, {}\n", self.size, other.size);
        }

        return (bitsonly || self.frame == other.frame)
            && self.size == other.size
            && self.bits == other.bits;
    }
}
