use crate::game_input::GameInput;
use log::{error, info};
use std::cmp::min;

const FRAME_WINDOW_SIZE: usize = 40;
const MIN_UNIQUE_FRAMES: usize = 10;
const MIN_FRAME_ADVANTAGE: usize = 3;
const MAX_FRAME_ADVANTAGE: usize = 9;

pub struct TimeSync {
    local: [i32; FRAME_WINDOW_SIZE],
    remote: [i32; FRAME_WINDOW_SIZE],
    last_inputs: [GameInput; MIN_UNIQUE_FRAMES],
    next_prediction: usize,
}

impl TimeSync {
    pub const fn new() -> Self {
        TimeSync {
            local: [0; FRAME_WINDOW_SIZE],
            remote: [0; FRAME_WINDOW_SIZE],
            next_prediction: FRAME_WINDOW_SIZE * 3,
            last_inputs: [GameInput::new(); MIN_UNIQUE_FRAMES],
        }
    }
    pub fn advance_frame(&mut self, input: &GameInput, advantage: i32, r_advantage: i32) {
        let sleep_time: i32 = 0;
        // Remember the last frame and frame advantage
        match input.frame {
            Some(frame) => {
                self.last_inputs[frame % MIN_UNIQUE_FRAMES] = input.clone();
                self.local[frame % FRAME_WINDOW_SIZE] = advantage;
                self.remote[frame % FRAME_WINDOW_SIZE] = r_advantage;
            }
            None => error!("game input frame is null"),
        }
    }

    pub fn recommend_frame_wait_duration(
        &mut self,
        require_idle_input: bool,
        count: &mut usize,
    ) -> i32 {
        // Average our local and remote frame advantages
        let (i, mut sum) = (0, 0);
        let (advantage, r_advantage): (f32, f32);
        for i in 0..FRAME_WINDOW_SIZE {
            sum += self.local[i];
        }
        advantage = sum as f32 / FRAME_WINDOW_SIZE as f32;
        sum = 0;
        for i in 0..FRAME_WINDOW_SIZE {
            sum += self.remote[i];
        }
        r_advantage = sum as f32 / FRAME_WINDOW_SIZE as f32;

        *count += 1;

        // See if someone should take action.  The person furthest ahead
        // needs to slow down so the other user can catch up.
        // Only do this if both clients agree on who's ahead!!
        if advantage >= r_advantage {
            return 0;
        }

        // Both clients agree that we're the one ahead.  Split
        // the difference between the two to figure out how long to
        // sleep for.
        let sleep_frames: i32 = (((r_advantage - advantage) / 2.) + 0.5) as i32;

        info!("iteration {}: sleep frames is {}\n", count, sleep_frames);

        // Some things just aren't worth correcting for.  Make sure
        // the difference is relevant before proceeding.
        if sleep_frames < MIN_FRAME_ADVANTAGE as i32 {
            return 0;
        }

        // Make sure our input had been "idle enough" before recommending
        // a sleep.  This tries to make the emulator sleep while the
        // user's input isn't sweeping in arcs (e.g. fireball motions in
        // Street Fighter), which could cause the player to miss moves.

        if require_idle_input {
            for i in 1..MIN_UNIQUE_FRAMES {
                if !self.last_inputs[i].equal(&self.last_inputs[0], true) {
                    info!(
                        "iteration {}: rejecting due to input stuff at position {}...!!!\n",
                        count, i
                    );
                    return 0;
                }
            }
        }

        // Success!!! Recommend the number of frames to sleep and adjust
        return min(sleep_frames, MAX_FRAME_ADVANTAGE as i32);
    }
}
