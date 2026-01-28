mod cpu;
mod mem;
mod gte;
mod interrupt;
mod timer;
mod cdrom;
mod expansion;
mod spu;
mod gpu;
mod utils;
mod io;

use std::path::PathBuf;

pub use crate::cpu::PSDebugger as PSDebugger;

/// Config for PlayStation.
pub struct PlayStationConfig {
    pub bios_path:  PathBuf,
}

/// A PlayStation console.
pub struct PlayStation {
    cpu: Option<cpu::CPU>,
    io: io::IO,
}

impl PlayStation {
    pub fn new(config: PlayStationConfig) -> Self {
        let (io, bus_io) = io::IO::new();
        let cpu = cpu::CPU::new(&config, bus_io);
        Self {
            cpu: Some(cpu),
            io
        }
    }

    /// Start running the CPU on its own thread.
    pub fn run_cpu(&mut self) {
        let cpu = std::mem::take(&mut self.cpu).expect("CPU thread already running!");
        let _cpu_thread = std::thread::spawn(move || {
            cpu.run();
        });
    }

    /// Drives the emulator and returns a frame.
    /// 
    /// This should be called at 60fps for NTSC,
    /// and 50fps for PAL.
    pub fn frame(&mut self, frame: &mut Frame) {
        let input = Input::empty(); // TODO!
        self.io.get_frame(&input, frame);
    }

    /// Make a debugger for stepping through instructions.
    /// 
    /// Warning: this will panic if the CPU thread has begun.
    pub fn make_debugger(self) -> PSDebugger {
        PSDebugger::new(self.cpu.expect("CPU thread running!"))
    }
}

/// Information for frame.
pub struct Frame {
    pub frame_buffer: Vec<u8>,
    pub size: (usize, usize)
}

impl Frame {
    pub fn new() -> Self {
        Self {
            frame_buffer: Vec::new(),
            size: (0, 0),
        }
    }

    pub fn resize(&mut self, size: (usize, usize)) {
        if size.0 != self.size.0 || size.1 != self.size.1 {
            self.size = size;
            self.frame_buffer.resize(size.0 * size.1 * 4, 0);
            self.frame_buffer.fill(0);
        }
    }
}

/// Input data.
#[derive(Clone)]
pub struct Input {
    // TODO: controllers..?
}

impl Input {
    pub fn empty() -> Self {
        Self {

        }
    }
}