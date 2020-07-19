use crate::game_input::{Frame, FrameNum, GameInput, GAMEINPUT_MAX_BYTES, GAMEINPUT_MAX_PLAYERS};
use log::info;
use std::{cmp, fmt::Debug};

const INPUT_QUEUE_LENGTH: usize = 128;
const DEFAULT_INPUT_SIZE: usize = 4;

macro_rules! previous_frame {
    ($offset:expr, $INPUT_QUEUE_LENGTH:expr) => {
        if $offset == 0 {
            $INPUT_QUEUE_LENGTH - 1
        } else {
            $offset - 1
        }
    };
}

#[derive(Copy, Clone)]
pub struct InputQueue {
    _id: usize,
    head: usize,
    tail: usize,
    length: usize,
    first_frame: bool,

    last_user_added_frame: Frame,
    last_added_frame: Frame,
    first_incorrect_frame: Frame,
    last_frame_requested: Frame,

    frame_delay: usize,

    inputs: [GameInput; INPUT_QUEUE_LENGTH],
    prediction: GameInput,
}

impl Default for InputQueue {
    fn default() -> Self {
        InputQueue {
            _id: 0,
            head: 0,
            tail: 0,
            length: 0,
            frame_delay: 0,
            first_frame: true,
            last_user_added_frame: None,
            last_added_frame: None,
            first_incorrect_frame: None,
            last_frame_requested: None,

            prediction: GameInput::init(None, None, DEFAULT_INPUT_SIZE),
            inputs: [GameInput::init(
                None,
                Some(&[b'0'; GAMEINPUT_MAX_BYTES * GAMEINPUT_MAX_PLAYERS]),
                DEFAULT_INPUT_SIZE,
            ); INPUT_QUEUE_LENGTH],
        }
    }
}

impl InputQueue {
    pub fn new() -> Self {
        InputQueue {
            ..Default::default()
        }
    }
    pub fn init(id: usize, input_size: usize) -> Self {
        InputQueue {
            _id: id,
            head: 0,
            tail: 0,
            length: 0,
            frame_delay: 0,
            first_frame: true,
            last_user_added_frame: None,
            last_added_frame: None,
            first_incorrect_frame: None,
            last_frame_requested: None,

            prediction: GameInput::init(None, None, input_size),
            inputs: [GameInput::init(
                None,
                Some(&[b'0'; GAMEINPUT_MAX_BYTES * GAMEINPUT_MAX_PLAYERS]),
                input_size,
            ); INPUT_QUEUE_LENGTH],
        }
    }
    pub fn get_confirmed_input(&self, requested_frame: Frame, input: &mut GameInput) -> bool {
        if let Some(_first_incorrect_frame) = self.first_incorrect_frame {
            assert!(requested_frame < self.first_incorrect_frame);
        }

        if let Some(requested_frame_value) = requested_frame {
            let offset = requested_frame_value as usize % INPUT_QUEUE_LENGTH;

            if self.inputs[offset].frame != requested_frame {
                return false;
            }
            *input = self.inputs[offset].clone();
            return true;
        }

        false
    }
    pub fn get_last_confirmed_frame(&self) -> Frame {
        info!(
            "returning last confirmed frame: {:?}\n",
            self.last_added_frame
        );

        self.last_added_frame
    }

    pub fn set_frame_delay(&mut self, delay: usize) {
        self.frame_delay = delay;
    }

    pub fn get_first_incorrect_frame(&self) -> Frame {
        self.first_incorrect_frame
    }

    pub fn discard_confirmed_frames(&mut self, in_frame: FrameNum) {
        let mut frame = in_frame;

        if let Some(last_frame_requested) = self.last_frame_requested {
            frame = cmp::min(frame, last_frame_requested);
        }

        if let Some(last_added_frame) = self.last_added_frame {
            info!(
                "discarding confirmed frames up to {} (last_added:{} length:{} [head:{} tail:{}]).\n",
                frame, last_added_frame, self.length, self.head, self.tail
            );

            if frame >= last_added_frame {
                self.tail = self.head;
            } else {
                if let Some(tail_frame) = self.inputs[self.tail].frame {
                    let offset: usize = (frame - tail_frame + 1) as usize;

                    info!("difference of {} frames.\n", offset);

                    self.tail = (self.tail + offset) % INPUT_QUEUE_LENGTH;
                    self.length -= offset;

                    info!(
                        "after discarding, new tail is {} (frame:{}).\n",
                        self.tail, tail_frame,
                    );
                }
            }
        }
    }

    pub fn reset_prediction(&mut self, frame: FrameNum) {
        if let Some(first_incorrect_frame) = self.first_incorrect_frame {
            assert!(frame <= first_incorrect_frame);
        }

        info!("resetting all prediction errors back to frame {}.\n", frame);

        self.prediction.frame = None;
        self.first_incorrect_frame = None;
        self.last_frame_requested = None;
    }

    pub fn get_input(&mut self, requested_frame: FrameNum, input: &mut GameInput) -> bool {
        info!("requesting input frame {}.\n", requested_frame);

        /*
         * No one should ever try to grab any input when we have a prediction
         * error.  Doing so means that we're just going further down the wrong
         * path.  ASSERT this to verify that it's true.
         */
        assert!(self.first_incorrect_frame == None);

        /*
         * Remember the last requested frame number for later.  We'll need
         * this in AddInput() to drop out of prediction mode.
         */
        self.last_frame_requested = Some(requested_frame);

        if let Some(input_tail_frame) = self.inputs[self.tail].frame {
            assert!(requested_frame >= input_tail_frame);

            if self.prediction.frame == None {
                let mut offset: usize = (requested_frame - input_tail_frame) as usize;

                if offset < self.length {
                    offset = (offset + self.tail) % INPUT_QUEUE_LENGTH;
                    if let Some(input_offset_frame) = self.inputs[offset].frame {
                        assert!(input_offset_frame == requested_frame);
                        *input = self.inputs[offset];
                    }
                    if let Some(input_frame) = input.frame {
                        info!("returning confirmed frame number {}.\n", input_frame);
                        return true;
                    }
                }

                /*
                 * The requested frame isn't in the queue.  Bummer.  This means we need
                 * to return a prediction frame.  Predict that the user will do the
                 * same thing they did last time.
                 */
                if requested_frame == 0 {
                    info!(
                        "basing new prediction frame from nothing, you're client wants frame 0.\n"
                    );
                    self.prediction.erase();
                } else if self.last_added_frame == None {
                    info!(
                        "basing new prediction frame from nothing, since we have no frames yet.\n"
                    );
                    self.prediction.erase();
                } else {
                    if let Some(input_previous_frame) =
                        self.inputs[previous_frame!(self.head, INPUT_QUEUE_LENGTH)].frame
                    {
                        info!("basing new prediction frame from previously added frame (queue entry:{}, frame:{}).\n",
                    previous_frame!(self.head, INPUT_QUEUE_LENGTH), input_previous_frame);
                    }
                    self.prediction = self.inputs[previous_frame!(self.head, INPUT_QUEUE_LENGTH)];
                }
                if let Some(prediction_frame) = self.prediction.frame {
                    self.prediction.frame = Some(prediction_frame + 1);
                }
            }
        }

        assert!(self.prediction.frame != None);
        if let Some(prediction_frame) = self.prediction.frame {
            /*
             * If we've made it this far, we must be predicting.  Go ahead and
             * forward the prediction frame contents.  Be sure to return the
             * frame number requested by the client, though.
             */
            *input = self.prediction;
            input.frame = Some(requested_frame);
            if let Some(input_frame) = input.frame {
                info!(
                    "returning prediction frame number {} ({}).\n",
                    input_frame, prediction_frame
                );
            }
        }

        false
    }

    pub fn add_delayed_input_to_queue(&mut self, input: &GameInput, frame_number: FrameNum) {
        info!(
            "adding delayed input frame number {} to queue.\n",
            frame_number
        );
        assert!(input.size == self.prediction.size);
        if let Some(last_added_frame) = self.last_added_frame {
            assert!(frame_number == last_added_frame + 1);
        }
        if let Some(input_previous_head_frame) =
            self.inputs[previous_frame!(self.head, INPUT_QUEUE_LENGTH)].frame
        {
            assert!(frame_number == 0 || input_previous_head_frame == frame_number - 1);
        } else {
            assert!(frame_number == 0);
        }

        /*
         * Add the frame to the back of the queue
         */
        self.inputs[self.head] = input.clone();
        self.inputs[self.head].frame = Some(frame_number);
        self.head = (self.head + 1) % INPUT_QUEUE_LENGTH;
        self.length += 1;
        self.first_frame = false;

        self.last_added_frame = Some(frame_number);

        if let Some(prediction_frame) = self.prediction.frame {
            assert!(frame_number == prediction_frame);

            /*
             * We've been predicting...  See if the inputs we've gotten match
             * what we've been predicting.  If so, don't worry about it.  If not,
             * remember the first input which was incorrect so we can report it
             * in GetFirstIncorrectFrame()
             */
            if self.first_incorrect_frame == None && !self.prediction.equal(input, true) {
                info!(
                    "frame {} does not match prediction.  marking error.\n",
                    frame_number,
                );
                self.first_incorrect_frame = Some(frame_number);
            }

            /*
             * If this input is the same frame as the last one requested and we
             * still haven't found any mis-predicted inputs, we can dump out
             * of predition mode entirely!  Otherwise, advance the prediction frame
             * count up.
             */
            if self.prediction.frame == self.last_frame_requested
                && self.first_incorrect_frame == None
            {
                info!("prediction is correct!  dumping out of prediction mode.\n");
                self.prediction.frame = None;
            } else {
                self.prediction.frame = Some(prediction_frame + 1);
            }
        }
        assert!(self.length <= INPUT_QUEUE_LENGTH);
    }

    pub fn add_input(&mut self, mut input: GameInput) {
        // let new_frame: Frame =;
        if let Some(input_frame) = input.frame {
            info!("adding input frame number {} to queue.\n", input_frame);

            /*
             * These next two lines simply verify that inputs are passed in
             * sequentially by the user, regardless of frame delay.
             */
            if let Some(last_user_added_frame) = self.last_user_added_frame {
                assert!(input_frame == last_user_added_frame + 1);
            }

            self.last_user_added_frame = input.frame;

            /*
             * Move the queue head to the correct point in preparation to
             * input the frame into the queue.
             */
            if let Some(new_frame) = self.advance_queue_head(input.frame) {
                self.add_delayed_input_to_queue(&input, new_frame);

                /*
                 * Update the frame number for the input.  This will also set the
                 * frame to GameInput::NullFrame for frames that get dropped (by
                 * design).
                 */
                input.frame = Some(new_frame);
            } else {
                input.frame = None
            }
        }
    }

    pub fn advance_queue_head(&mut self, input_frame: Frame) -> Frame {
        if let Some(frame) = input_frame {
            info!("advancing queue head to frame {}.\n", frame);
            if let Some(input_previous_head) =
                self.inputs[previous_frame!(self.head, INPUT_QUEUE_LENGTH)].frame
            {
                let mut expected_frame = if self.first_frame {
                    0
                } else {
                    input_previous_head + 1
                };
                if expected_frame > frame {
                    /*
                     * This can occur when the frame delay has dropped since the last
                     * time we shoved a frame into the system.  In this case, there's
                     * no room on the queue.  Toss it.
                     */
                    info!(
                        "Dropping input frame {} (expected next frame to be {}).\n",
                        frame, expected_frame
                    );
                    return None;
                }

                while expected_frame < frame {
                    /*
                     * This can occur when the frame delay has been increased since the last
                     * time we shoved a frame into the system.  We need to replicate the
                     * last frame in the queue several times in order to fill the space
                     * left.
                     */
                    info!(
                        "Adding padding frame {} to account for change in frame delay.\n",
                        expected_frame
                    );
                    let last_frame_input: GameInput =
                        self.inputs[previous_frame!(self.head, INPUT_QUEUE_LENGTH)];

                    self.add_delayed_input_to_queue(&last_frame_input, expected_frame);
                    expected_frame += 1;
                }

                assert!(frame == 0 || frame == input_previous_head + 1);
            }
            return Some(frame);
        }
        input_frame
    }
}
