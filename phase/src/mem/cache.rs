use crate::utils::interface::MemInterface;

use super::RAM;

/// Instruction Cache for CPU.
/// 
/// 4kB
pub struct ICache {
    data: RAM
}

impl ICache {
    pub fn new() -> Self {
        Self {
            data: RAM::new(1024 * 4),
        }
    }
}

impl MemInterface for ICache {
    fn read_byte(&mut self, addr: u32) -> u8 {
        self.data.read_byte(addr)
    }
    fn read_halfword(&mut self, addr: u32) -> u16 {
        self.data.read_halfword(addr)
    }
    fn read_word(&mut self, addr: u32) -> u32 {
        self.data.read_word(addr)
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        self.data.write_byte(addr, data);
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        self.data.write_halfword(addr, data);
    }
    fn write_word(&mut self, addr: u32, data: u32) {
        self.data.write_word(addr, data);
    }
}