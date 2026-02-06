use crate::mem::ram::RAM;


#[derive(Default)]
pub struct Voice {
    // Registers
    vol_left:       u16,    // 0
    vol_right:      u16,    // 2
    sample_rate:    u16,    // 4
    start_addr:     u16,    // 6
    adsr_lo:        u16,    // 8
    adsr_hi:        u16,    // A
    adsr_vol:       u16,    // C
    repeat_addr:    u16,    // E
}

impl Voice {
    pub fn read_halfword(&self, addr: u32) -> u16 {
        match addr {
            0x0 => self.vol_left,
            0x2 => self.vol_right,
            0x4 => self.sample_rate,
            0x6 => self.start_addr,
            0x8 => self.adsr_lo,
            0xA => self.adsr_hi,
            0xC => self.adsr_vol,
            0xE => self.repeat_addr,
            _ => unreachable!()
        }
    }

    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0 => self.vol_left = data,
            0x2 => self.vol_right = data,
            0x4 => self.sample_rate = data,
            0x6 => self.start_addr = data,
            0x8 => self.adsr_lo = data,
            0xA => self.adsr_hi = data,
            0xC => self.adsr_vol = data,
            0xE => self.repeat_addr = data,
            _ => unreachable!()
        }
    }

    pub fn clock(&mut self, cycles: usize) {
        // ?
    }

    pub fn get_sample(&self, ram: &RAM, irq_addr: u32) -> (i32, i32) {
        (0, 0)
    }
}