use log::info;
use std::fmt::Display;

// GAMEINPUT_MAX_BYTES * GAMEINPUT_MAX_PLAYERS * 8 must be less than
// 2^BITVECTOR_NIBBLE_SIZE (see bitvector.h)

pub const GAMEINPUT_MAX_BYTES: usize = 9;
pub const GAMEINPUT_MAX_PLAYERS: usize = 2;
pub const INPUT_BUFFER_SIZE: usize = GAMEINPUT_MAX_BYTES * GAMEINPUT_MAX_PLAYERS;
pub type InputBuffer = [u8; GAMEINPUT_MAX_BYTES * GAMEINPUT_MAX_PLAYERS];
pub type FrameNum = u32;
pub type Frame = Option<FrameNum>;

#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
pub struct GameInput {
    pub frame: Frame,
    pub size: usize,
    pub bits: InputBuffer,
}

impl GameInput {
    pub const fn new() -> Self {
        GameInput {
            frame: None,
            size: 0,
            bits: [b'0'; INPUT_BUFFER_SIZE],
        }
    }
    pub fn init(frame: Frame, bits: Option<&InputBuffer>, size: usize) -> GameInput {
        assert!(size <= GAMEINPUT_MAX_BYTES);
        match bits {
            Some(i_bits) => GameInput {
                frame,
                size,
                bits: i_bits.clone(),
            },
            None => GameInput {
                frame,
                size,
                bits: [b'0'; GAMEINPUT_MAX_BYTES * GAMEINPUT_MAX_PLAYERS],
            },
        }
    }
    pub const fn value(&self, i: usize) -> bool {
        (self.bits[i / 8] & (1 << (i % 8))) != 0
    }
    pub fn set(&mut self, i: usize) {
        self.bits[i / 8] |= 1 << (i % 8);
    }
    pub fn clear(&mut self, i: usize) {
        self.bits[i / 8] &= !(1 << (i % 8));
    }
    pub fn erase(&mut self) {
        self.bits = [b'0'; GAMEINPUT_MAX_BYTES * GAMEINPUT_MAX_PLAYERS];
    }
    pub fn describe(&self, show_frame: bool) -> String {
        let mut buf: String = String::from("");
        if let Some(frame) = self.frame {
            if show_frame {
                buf = format!("(frame:{} size:{}", frame, self.size);
            } else {
                buf = format!("(size:{}", self.size);
            }
        }

        for i in 0..(self.size as usize) * 8 {
            if self.value(i) {
                buf.push_str(&format!("{:2}", i));
            }
        }
        buf.push(')');
        buf
    }
    // fn log(prefix: &String, show_frame: bool) {}
    pub fn equal(&self, other: &GameInput, bitsonly: bool) -> bool {
        if !bitsonly {
            match (self.frame, other.frame) {
                (Some(self_frame), Some(other_frame)) => {
                    if self_frame != other_frame {
                        info!("frames don't match: {}, {}\n", self_frame, other_frame,);
                    }
                }
                (Some(self_frame), None) => {
                    info!("frames don't match: {}, {}\n", self_frame, "None",);
                }
                (None, Some(other_frame)) => {
                    info!("frames don't match: {}, {}\n", "None", other_frame,);
                }
                (None, None) => {
                    info!("frames don't match: {}, {}\n", "None", "None",);
                }
            }
        }

        if self.size != other.size {
            info!("sizes don't match: {}, {}\n", self.size, other.size);
        }

        let bits_equality = self.bits != other.bits;
        if !bits_equality {
            info!("bits don't match\n");
        }

        return (bitsonly || self.frame == other.frame) && self.size == other.size && bits_equality;
    }
}
