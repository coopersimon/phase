use super::StereoVolume;

/// SPU reverb registers.
#[derive(Default)]
pub struct ReverbUnit {
    pub output_vol:         StereoVolume,
    pub base_addr:          u16,
    pub apf_offset:         [u16; 2],
    pub apf_vol:            [u16; 2],
    pub apf_addr_left:      [u16; 2],
    pub apf_addr_right:     [u16; 2],
    pub impulse_response:   u16,
    pub wall_response:      u16,
    pub comb_vol:           [u16; 4],
    pub comb_addr_left:     [u16; 4],
    pub comb_addr_right:    [u16; 4],
    pub same_side_reflect_addr_left:  [u16; 2],
    pub same_side_reflect_addr_right: [u16; 2],
    pub diff_side_reflect_addr_left:  [u16; 2],
    pub diff_side_reflect_addr_right: [u16; 2],
    pub input_vol:          StereoVolume,

    buffer_addr: u32,
    buffer_size: u32,
}

impl ReverbUnit {
    fn offset_addr(&self, addr: u16) -> u32 {
        let addr = self.buffer_addr + (addr as u32 * 8);
        if addr > 0x7FFFF {
            addr - self.buffer_size
        } else {
            addr
        }
    }
    fn offset_prev_addr(&self, addr: u16) -> u32 {
        let addr = self.buffer_addr + (addr as u32 * 8) - 2;
        if addr > 0x7FFFF {
            addr - self.buffer_size
        } else {
            addr
        }
    }

    pub fn set_base_addr(&mut self, addr: u16) {
        self.base_addr = addr;
        self.buffer_size = 0x80000 - (self.base_addr as u32) * 8;
        self.reset_buffer_addr();
    }

    pub fn reset_buffer_addr(&mut self) {
        self.buffer_addr = (self.base_addr as u32) * 8;
    }

    pub fn inc_buffer_addr(&mut self) {
        let start_addr = (self.base_addr as u32) * 8;
        self.buffer_addr = ((self.buffer_addr + 2) & 0x7FFFF).max(start_addr);
    }

    // TODO: clean up the following...

    /// Get left-side same-side addr (m, d, m)
    /// Final m is written back to.
    pub fn same_side_addr_left(&self) -> (u32, u32, u32) {
        (self.offset_prev_addr(self.same_side_reflect_addr_left[0]),
        self.offset_addr(self.same_side_reflect_addr_left[1]),
        self.offset_addr(self.same_side_reflect_addr_left[0]))
    }

    /// Get right-side same-side addr (m, d, m)
    /// Final m is written back to.
    pub fn same_side_addr_right(&self) -> (u32, u32, u32) {
        (self.offset_prev_addr(self.same_side_reflect_addr_right[0]),
        self.offset_addr(self.same_side_reflect_addr_right[1]),
        self.offset_addr(self.same_side_reflect_addr_right[0]))
    }

    /// Get left-side diff-side addr (m, d, m)
    /// Final m is written back to.
    pub fn diff_side_addr_left(&self) -> (u32, u32, u32) {
        (self.offset_prev_addr(self.diff_side_reflect_addr_left[0]),
        self.offset_addr(self.diff_side_reflect_addr_left[1]),
        self.offset_addr(self.diff_side_reflect_addr_left[0]))
    }

    /// Get right-side diff-side addr (m, d, m)
    /// Final m is written back to.
    pub fn diff_side_addr_right(&self) -> (u32, u32, u32) {
        (self.offset_prev_addr(self.diff_side_reflect_addr_right[0]),
        self.offset_addr(self.diff_side_reflect_addr_right[1]),
        self.offset_addr(self.diff_side_reflect_addr_right[0]))
    }

    pub fn apply_reverb_input(&self, input: i32, d_val: u16, m_val: u16) -> u16 {
        let wall_response = self.wall_response as i16 as i32;
        let impulse_response = self.impulse_response as i16 as i32;
        let d_val = d_val as i16 as i32;
        let m_val = m_val as i16 as i32;
        let out = (((input + ((d_val * wall_response) >> 15) - m_val) * impulse_response) >> 15) + m_val;
        out.clamp(-0x8000, 0x7FFF) as u16
    }

    pub fn comb_filter_addr_left(&self) -> [u32; 4] {
        std::array::from_fn(|n| self.offset_addr(self.comb_addr_left[n]))
    }

    pub fn comb_filter_addr_right(&self) -> [u32; 4] {
        std::array::from_fn(|n| self.offset_addr(self.comb_addr_right[n]))
    }

    pub fn apply_comb_filter(&self, comb_val: &[u16]) -> i32 {
        (0..4).fold(0, |acc, n| {
            let comb_val = comb_val[n] as i16 as i32;
            let vol = self.comb_vol[n] as i16 as i32;
            acc + ((comb_val * vol) >> 15)
        }).clamp(-0x8000, 0x7FFF)
    }

    pub fn apf_src_addr_left(&self, n: usize) -> u32 {
        self.offset_addr(self.apf_addr_left[n] - self.apf_offset[n])
    }

    pub fn apf_src_addr_right(&self, n: usize) -> u32 {
        self.offset_addr(self.apf_addr_right[n] - self.apf_offset[n])
    }

    pub fn apf_dst_addr_left(&self, n: usize) -> u32 {
        self.offset_addr(self.apf_addr_left[n])
    }

    pub fn apf_dst_addr_right(&self, n: usize) -> u32 {
        self.offset_addr(self.apf_addr_right[n])
    }

    pub fn apply_apf(&self, data: i32, n: usize) -> i32 {
        let vol = self.apf_vol[n] as i16 as i32;
        (data * vol) >> 15
    }
}