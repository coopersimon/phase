mod cop0;
mod internalmem;

use mips::{coproc::EmptyCoproc, cpu::{MIPSCore, MIPSICore, mips1::{MIPSI, MIPSIInstruction}}, mem::{Data, Mem32}};
use internalmem::InternalMem;
use crate::gte::GTE;
use crate::PlayStationConfig;

type MIPSCPU = MIPSI<InternalMem, EmptyCoproc, GTE, EmptyCoproc>;

/// PlayStation CPU object.
/// This drives the CPU and manages memory.
pub struct CPU {
    core: MIPSCPU
}

impl CPU {
    pub fn new(config: &PlayStationConfig) -> Self {
        let mem = Box::new(InternalMem::new(config));
        let mut core = MIPSCPU::with_memory(mem)
            .add_coproc2(GTE::new())
            .build();
        core.reset();
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


/// Debugger for PlayStation.
/// This allows the user to step instruction-by-instruction and
/// inspect internal state.
pub struct PSDebugger {
    core: MIPSCPU
}

impl PSDebugger {
    pub fn new(cpu: CPU) -> Self {
        Self {
            core: cpu.core
        }
    }

    pub fn step(&mut self) {
        self.core.step();
    }

    pub fn get_state(&mut self) -> CPUState {
        let mut regs = [0; 32];
        for reg in 0..32_u8 {
            regs[reg as usize] = self.core.read_gp(reg);
        }
        let pc = self.core.read_pc();
        let Data{data: instr_bits, ..} = self.core.mut_mem().read_word(pc);
        let instr = MIPSIInstruction::decode(instr_bits);
        CPUState {
            regs,
            hi: self.core.read_hi(),
            lo: self.core.read_lo(),
            pc: pc,
            instr: instr,
        }
    }

    pub fn read_byte(&mut self, addr: u32) -> Data<u8> {
        self.core.mut_mem().read_byte(addr)
    }

    pub fn read_halfword(&mut self, addr: u32) -> Data<u16> {
        self.core.mut_mem().read_halfword(addr)
    }

    pub fn read_word(&mut self, addr: u32) -> Data<u32> {
        self.core.mut_mem().read_word(addr)
    }
}

pub struct CPUState {
    pub regs: [u32; 32],
    pub hi: u32,
    pub lo: u32,
    pub pc: u32,
    pub instr: Option<MIPSIInstruction>,
    // TODO: cop0 stuff?
}