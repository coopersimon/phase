use crate::utils::interface::MemInterface;


/// PSX Memory bus control.
pub struct MemControl {
    exp_1_base_addr: u32,
    exp_2_base_addr: u32,
    exp_1_delay_size: u32,
    exp_2_delay_size: u32,
    exp_3_delay_size: u32,
    bios_delay_size: u32,
    spu_delay_size: u32,
    cdrom_delay_size: u32,
    common_delay: u32,
    ram_size: u32,
}

impl MemControl {
    pub fn new() -> Self {
        Self {
            exp_1_base_addr: 0x1F00_0000,
            exp_2_base_addr: 0x1F80_2000,
            exp_1_delay_size: 0,
            exp_2_delay_size: 0,
            exp_3_delay_size: 0,
            bios_delay_size: 0,
            spu_delay_size: 0,
            cdrom_delay_size: 0,
            common_delay: 0,
            ram_size: 0,
        }
    }
}

impl MemInterface for MemControl {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x1F801000 => self.exp_1_base_addr,
            0x1F801004 => self.exp_2_base_addr,
            0x1F801008 => self.exp_1_delay_size,
            0x1F80100C => self.exp_3_delay_size,
            0x1F801010 => self.bios_delay_size,
            0x1F801014 => self.spu_delay_size,
            0x1F801018 => self.cdrom_delay_size,
            0x1F80101C => self.exp_2_delay_size,
            0x1F801020 => self.common_delay,
            0x1F801060 => self.ram_size,
            _ => panic!("unexpected address in mem control"),
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x1F801000 => self.exp_1_base_addr = data,
            0x1F801004 => self.exp_2_base_addr = data,
            0x1F801008 => self.exp_1_delay_size = data,
            0x1F80100C => self.exp_3_delay_size = data,
            0x1F801010 => self.bios_delay_size = data,
            0x1F801014 => self.spu_delay_size = data,
            0x1F801018 => self.cdrom_delay_size = data,
            0x1F80101C => self.exp_2_delay_size = data,
            0x1F801020 => self.common_delay = data,
            0x1F801060 => self.ram_size = data,
            _ => panic!("unexpected address in mem control"),
        }
    }
}