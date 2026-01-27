
use super::{Input, Frame};

use crossbeam_channel::{
    Sender, Receiver, unbounded
};
use std::sync::{
    Arc, Mutex
};

/// Syncing and communicating with the real-time system.
/// 
/// Button inputs are provided as input,
/// frame data is taken as output.
/// 
/// TODO: AUDIO.
pub struct IO {
    frame_rx: Receiver<()>,
    input_tx: Sender<Input>,
    
    /// We hold a single frame internally, and then copy over
    /// the data on output. This allows us to reduce allocations.
    frame:  Arc<Mutex<Frame>>,
}

impl IO {
    pub fn new() -> (Self, BusIO) {
        let (frame_tx, frame_rx) = unbounded();
        let (input_tx, input_rx) = unbounded();
        let frame = Arc::new(Mutex::new(Frame::new()));
        let io = Self {
            frame_rx,
            input_tx,

            frame: frame.clone(),
        };
        let bus_io = BusIO {
            frame_tx,
            input_rx,

            frame,
        };
        (io, bus_io)
    }

    /// Blocks until a frame is ready from the system.
    pub fn get_frame(&mut self, input: &Input, frame: &mut Frame) {
        if self.input_tx.send(input.clone()).is_ok() {
            if self.frame_rx.recv().is_ok() {
                let frame_data = self.frame.lock().unwrap();
                frame.size = frame_data.size;
                frame.frame_buffer.resize(frame_data.frame_buffer.len(), 0);
                frame.frame_buffer.copy_from_slice(&frame_data.frame_buffer);
            }
        }
    } 
}

/// The component of the I/O system that lives
/// on the CPU/Memory Bus side.
pub struct BusIO {
    frame_tx: Sender<()>,
    input_rx: Receiver<Input>,

    /// The frame isn't actually used here, it's just passed
    /// over to the render thread.
    frame:  Arc<Mutex<Frame>>,
}

impl BusIO {
    pub fn send_frame(&mut self) -> Input {
        if let Some(i) = self.input_rx.recv().ok() {
            let _ = self.frame_tx.send(());
            i
        } else {
            // The channel has been dropped. This either means
            // that the program is about to close, or that we are in
            // debug mode.
            Input::empty()
        }
    }

    pub fn clone_frame_arc(&self) -> Arc<Mutex<Frame>> {
        self.frame.clone()
    }
}