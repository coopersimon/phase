use mips::{coproc::Coprocessor0, mem::{Data, Mem32}};

use crate::{
    AudioChannel, PlayStationConfig, cpu::cop0::SystemCoproc, io::BusIO, mem::{MemBus, ram::RAM}
};

const I_CACHE_SIZE: u32 = 4 * 1024;

#[inline(always)]
fn cacheable(addr: u32) -> bool {
    addr & 0x8000_0000 == 0
}

pub struct InternalMem {
    system_coproc:  SystemCoproc,
    mem_bus:        MemBus,
    i_cache:        RAM,
    cache_control:  u32,
}

impl InternalMem {
    pub fn new(config: &PlayStationConfig, io: BusIO) -> Self {
        Self {
            system_coproc:  SystemCoproc::new(),
            mem_bus:        MemBus::new(config, io),
            i_cache:        RAM::new(I_CACHE_SIZE as usize),
            cache_control:  0,
        }
    }

    pub fn enable_audio(&mut self) -> AudioChannel {
        self.mem_bus.enable_audio()
    }
}

impl Mem32 for InternalMem {
    type Addr = u32;
    const LITTLE_ENDIAN: bool = true;

    fn clock(&mut self, cycles: usize) -> u8 {
        self.mem_bus.clock(cycles)
    }

    fn read_byte(&mut self, addr: Self::Addr) -> Data<u8> {
        if self.system_coproc.isolate_cache() && cacheable(addr) {
            Data { data: self.i_cache.read_byte(addr % I_CACHE_SIZE), cycles: 1 }
        } else {
            self.mem_bus.read_byte(addr & 0x1FFF_FFFF)
        }
    }

    fn write_byte(&mut self, addr: Self::Addr, data: u8) -> usize {
        if self.system_coproc.isolate_cache() && cacheable(addr) {
            self.i_cache.write_byte(addr % I_CACHE_SIZE, data);
            1
        } else {
            self.mem_bus.write_byte(addr & 0x1FFF_FFFF, data)
        }
    }

    fn read_halfword(&mut self, addr: Self::Addr) -> Data<u16> {
        if self.system_coproc.isolate_cache() && cacheable(addr) {
            Data { data: self.i_cache.read_halfword(addr % I_CACHE_SIZE), cycles: 1 }
        } else {
            self.mem_bus.read_halfword(addr & 0x1FFF_FFFF)
        }
    }

    fn write_halfword(&mut self, addr: Self::Addr, data: u16) -> usize {
        if self.system_coproc.isolate_cache() && cacheable(addr) {
            self.i_cache.write_halfword(addr % I_CACHE_SIZE, data);
            1
        } else {
            self.mem_bus.write_halfword(addr & 0x1FFF_FFFF, data)
        }
    }

    fn read_word(&mut self, addr: Self::Addr) -> Data<u32> {
        if addr == 0xFFFE_0130 {
            Data { data: self.cache_control, cycles: 1 }
        } else if self.system_coproc.isolate_cache() && cacheable(addr) {
            Data { data: self.i_cache.read_word(addr % I_CACHE_SIZE), cycles: 1 }
        } else {
            self.mem_bus.read_word(addr & 0x1FFF_FFFF)
        }
    }

    fn write_word(&mut self, addr: Self::Addr, data: u32) -> usize {
        if addr == 0xFFFE_0130 {
            self.cache_control = data;
            1
        } else if self.system_coproc.isolate_cache() && cacheable(addr) {
            self.i_cache.write_word(addr % I_CACHE_SIZE, data);
            1
        } else {
            self.mem_bus.write_word(addr & 0x1FFF_FFFF, data)
        }
    }
}

// TODO: is wrapping the existing coproc the best way to do this?
impl Coprocessor0 for InternalMem {
    fn move_from_reg(&mut self, reg: u8) -> u32 {
        self.system_coproc.move_from_reg(reg)
    }

    fn move_to_reg(&mut self, reg: u8, data: u32) {
        self.system_coproc.move_to_reg(reg, data);
    }

    fn operation(&mut self, op: u32) {
        self.system_coproc.operation(op);
    }

    fn reset(&mut self) -> u32 {
        self.system_coproc.reset()
    }

    fn trigger_exception(&mut self, exception: &mips::coproc::Exception) -> u32 {
        self.system_coproc.trigger_exception(exception)
    }

    fn external_interrupt(&mut self, mask: u8) -> bool {
        self.system_coproc.external_interrupt(mask)
    }
}