use mips::mem::Mem32;


pub struct MemBus {

}

impl Mem32 for MemBus {
    type Addr = u32;
    const LITTLE_ENDIAN: bool = true;

    fn read_byte(&mut self, addr: Self::Addr) -> u8 {
        0
    }

    fn write_byte(&mut self, addr: Self::Addr, data: u8) {
        
    }

    fn read_halfword(&mut self, addr: Self::Addr) -> u16 {
        0
    }

    fn write_halfword(&mut self, addr: Self::Addr, data: u16) {
        
    }

    fn read_word(&mut self, addr: Self::Addr) -> u32 {
        0
    }

    fn write_word(&mut self, addr: Self::Addr, data: u32) {
        
    }
}