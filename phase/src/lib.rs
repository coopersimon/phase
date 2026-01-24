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

use std::path::PathBuf;

pub use crate::cpu::PSDebugger as PSDebugger;

/// Config for PlayStation.
pub struct PlayStationConfig {
    pub bios_path:  PathBuf,
}

/// A PlayStation console.
pub struct PlayStation {
    cpu: cpu::CPU
}

impl PlayStation {
    pub fn new(config: &PlayStationConfig) -> Self {
        let cpu = cpu::CPU::new(config);
        Self {
            cpu
        }
    }

    pub fn frame(&mut self, frame: &mut Frame) {
        // TODO.
    }

    pub fn make_debugger(self) -> PSDebugger {
        PSDebugger::new(self.cpu)
    }
}

/// Information for frame.
pub struct Frame {
    pub frame_buffer: Vec<u8>,
    pub size: (usize, usize)
}
