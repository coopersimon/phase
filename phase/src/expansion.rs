// Expansion port things

use crate::utils::interface::MemInterface;

pub struct ExpansionPort1 {

}

impl ExpansionPort1 {
    pub fn new() -> Self {
        Self {

        }
    }
}

impl MemInterface for ExpansionPort1 {
    fn read_word(&mut self, _addr: u32) -> u32 {
        0
    }

    fn write_word(&mut self, _addr: u32, _data: u32) {
        
    }
}

pub struct ExpansionPort2 {
    boot_status: u8
}

impl ExpansionPort2 {
    pub fn new() -> Self {
        Self {
            boot_status: 0,
        }
    }

    fn write_boot_status(&mut self, data: u8) {
        self.boot_status = data;
        println!("BOOT STAT {:X}", data);
    }
}

impl MemInterface for ExpansionPort2 {
    fn read_word(&mut self, _addr: u32) -> u32 {
        0
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        // TODO: exception?
        let bytes = data.to_le_bytes();
        self.write_byte(addr, bytes[0]);
        self.write_byte(addr + 1, bytes[1]);
        self.write_byte(addr + 2, bytes[2]);
        self.write_byte(addr + 3, bytes[3]);
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0x1F80_2040 => {},
            0x1F80_2041 => self.write_boot_status(data),
            0x1F80_2042 => {},
            0x1F80_2043 => {},
            _ => panic!("invalid exp2 address!"),
        }
    }
}