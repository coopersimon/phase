mod cop0;

use mips::{coproc::EmptyCoproc, cpu::{MIPSCore, mips1::MIPSI}};
use crate::mem::MemBus;
use crate::gte::GTE;
use cop0::SystemCoproc;

type MIPSCPU = MIPSI<MemBus, SystemCoproc, EmptyCoproc, GTE, EmptyCoproc>;

/// PlayStation CPU object.
/// This drives the CPU and manages memory.
pub struct CPU {
    core: MIPSCPU
}

impl CPU {
    pub fn new() -> Self {
        let membus = Box::new(MemBus::new());
        let core = MIPSCPU::with_memory(membus)
            .add_coproc0(SystemCoproc::new())
            .add_coproc2(GTE::new())
            .build();
        Self {
            core
        }
    }

    /// Step a single frame.
    /// 
    /// This will do a frame's worth of processing and return an image.
    /// It is the external application's responsibility to manage real-world timing.
    /// 
    /// It will also send a frame's worth of audio data along the audio channel.
    pub fn frame(&mut self) {
        let mut cycle_count = 0;
        const CYCLE_MAX: usize = 263 * 3413; // TODO: improve timing...
        while cycle_count < CYCLE_MAX {
            self.core.step();
        }
    }
}
