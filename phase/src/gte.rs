use mips::coproc::Coprocessor;
use crate::utils::bits::*;

pub struct GTE {
    regs:           [u32; 32],
    control_regs:   [u32; 32]
}

impl GTE {
    pub fn new() -> Self {
        Self {
            regs:           [0; 32],
            control_regs:   [0; 32]
        }
    }
}

impl Coprocessor for GTE {
    fn load_from_mem(&mut self, reg: usize, data: u32) {
        self.move_to_reg(reg, data);
    }

    fn store_to_mem(&mut self, reg: usize) -> u32 {
        self.move_from_reg(reg)
    }

    fn move_from_control(&mut self, reg: usize) -> u32 {
        self.control_regs[reg]
    }

    fn move_to_control(&mut self, reg: usize, data: u32) {
        self.control_regs[reg] = data;
    }

    fn move_from_reg(&mut self, reg: usize) -> u32 {
        if reg == Reg::LZCR.idx() { // Count leading zeros / ones function.
            let lzcs = self.regs[Reg::LZCS.idx()];
            lzcs.leading_ones() | lzcs.leading_zeros()
        } else {
            self.regs[reg]
        }
    }

    fn move_to_reg(&mut self, reg: usize, data: u32) {
        self.regs[reg] = data;
    }

    fn operation(&mut self, instr: u32) {
        self.control_regs[Control::FLAG.idx()] = 0;
        let op = {
            const MASK: u32 = 0x3F;
            (instr & MASK) as u8
        };
        // If bit 19 is set, shift left by 12.
        let shift = || -> u8 {
            const MASK: u32 = 0x0008_0000;
            const SHIFT: usize = 19 - 12;
            ((instr & MASK) >> SHIFT) as u8
        };
        let ir_unsigned = || -> bool {
            const MASK: u32 = 0x0000_0400;
            const SHIFT: usize = 10;
            ((instr & MASK) >> SHIFT) == 1
        };
        match op {
            0x01 => self.rtps(shift(), ir_unsigned()),
            0x06 => self.nclip(),
            0x0C => self.op(shift(), ir_unsigned()),
            0x10 => self.dpcs(),
            0x11 => self.intpl(),
            0x12 => {
                let mul_mat = {
                    const MASK: u32 = 0x0006_0000;
                    const SHIFT: usize = 17;
                    ((instr & MASK) >> SHIFT) as u8
                };
                let mul_vec = {
                    const MASK: u32 = 0x0001_8000;
                    const SHIFT: usize = 15;
                    ((instr & MASK) >> SHIFT) as u8
                };
                let trans_vec = {
                    const MASK: u32 = 0x0000_6000;
                    const SHIFT: usize = 13;
                    ((instr & MASK) >> SHIFT) as u8
                };
                self.mvmva(shift(), ir_unsigned(), mul_mat, mul_vec, trans_vec)
            },
            0x13 => self.ncds(shift(), ir_unsigned()),
            0x14 => self.cdp(),
            0x16 => self.ncdt(shift(), ir_unsigned()),
            0x1B => self.nccs(shift(), ir_unsigned()),
            0x1C => self.cc(),
            0x1E => self.ncs(shift(), ir_unsigned()),
            0x20 => self.nct(shift(), ir_unsigned()),
            0x28 => self.sqr(shift(), ir_unsigned()),
            0x29 => self.dcpl(),
            0x2A => self.dpct(),
            0x2D => self.avsz3(),
            0x2E => self.avsz4(),
            0x30 => self.rtpt(shift(), ir_unsigned()),
            0x3D => self.gpf(),
            0x3E => self.gpl(),
            0x3F => self.ncct(shift(), ir_unsigned()),
            _ => {}, // Undefined
        }
    }
}

enum Reg {
    VXY0,
    VZ0,
    VXY1,
    VZ1,
    VXY2,
    VZ2,
    RGBC,
    OTZ,
    IR0,
    IR1,
    IR2,
    IR3,
    SXY0,
    SXY1,
    SXY2,
    SXYP,
    SZ0,
    SZ1,
    SZ2,
    SZ3,
    RGB0,
    RGB1,
    RGB2,
    RES1,
    MAC0,
    MAC1,
    MAC2,
    MAC3,
    IRGB,
    ORGB,
    LZCS,
    LZCR
}

impl Reg {
    fn idx(self) -> usize {
        self as usize
    }
}

enum Control {
    RT11_12,
    RT13_21,
    RT22_23,
    RT31_32,
    RT33,
    TRX,
    TRY,
    TRZ,
    L11_12,
    L13_21,
    L22_23,
    L31_32,
    L33,
    RBK,
    GBK,
    BBK,
    LR1R2,
    LR3G1,
    LG2G3,
    LB1B2,
    LB3,
    RFC,
    GFC,
    BFC,
    OFX,
    OFY,
    H,
    DQA,
    DQB,
    ZSF3,
    ZSF4,
    FLAG
}

impl Control {
    fn idx(self) -> usize {
        self as usize
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct Flag: u32 {
        const Checksum = bit!(31);
        const MAC1PosOvf = bit!(30);
        const MAC2PosOvf = bit!(29);
        const MAC3PosOvf = bit!(28);
        const MAC1NegOvf = bit!(27);
        const MAC2NegOvf = bit!(26);
        const MAC3NegOvf = bit!(25);
        const IR1Sat = bit!(24);
        const IR2Sat = bit!(23);
        const IR3Sat = bit!(22);
        const ColRSat = bit!(21);
        const ColGSat = bit!(20);
        const ColBSat = bit!(19);
        const SZ3Sat = bit!(18);
        const DivOvf = bit!(17);
        const MAC0PosOvf = bit!(16);
        const MAC0NegOvf = bit!(15);
        const SXSat = bit!(14);
        const SYSat = bit!(13);
        const IR0Sat = bit!(12);

        const ChecksumTest = bits![30, 29, 28, 27, 26, 25, 24, 23, 22, 18, 17, 16, 15, 14, 13, 12];
    }
}

// Internal helper methods.
impl GTE {
    #[inline]
    fn insert_flag(&mut self, flag: Flag) {
        self.control_regs[Control::FLAG.idx()] |= flag.bits();
        if self.control_regs[Control::FLAG.idx()] & Flag::ChecksumTest.bits() != 0 {
            self.control_regs[Control::FLAG.idx()] |= Flag::Checksum.bits();
        }
    }

    #[inline]
    fn set_flag(&mut self, flag: Flag, cond: bool) {
        if cond {
            self.control_regs[Control::FLAG.idx()] |= flag.bits();
            if self.control_regs[Control::FLAG.idx()] & Flag::ChecksumTest.bits() != 0 {
                self.control_regs[Control::FLAG.idx()] |= Flag::Checksum.bits();
            }
        }
    }

    #[inline]
    fn get_reg_u16_lo(&self, reg: Reg) -> u16 {
        self.regs[reg.idx()] as u16
    }

    #[inline]
    fn get_reg_i16_lo(&self, reg: Reg) -> i16 {
        self.regs[reg.idx()] as i16
    }

    #[inline]
    fn get_reg_i16_hi(&self, reg: Reg) -> i16 {
        (self.regs[reg.idx()] >> 16) as i16
    }

    #[inline]
    fn get_reg_i32(&self, reg: Reg) -> i32 {
        self.regs[reg.idx()] as i32
    }

    #[inline]
    fn get_control_u16_lo(&self, reg: Control) -> u16 {
        self.control_regs[reg.idx()] as u16
    }

    #[inline]
    fn get_control_i16_lo(&self, reg: Control) -> i16 {
        self.control_regs[reg.idx()] as i16
    }

    #[inline]
    fn get_control_i16_hi(&self, reg: Control) -> i16 {
        (self.control_regs[reg.idx()] >> 16) as i16
    }

    #[inline]
    fn get_control_i32(&self, reg: Control) -> i32 {
        self.control_regs[reg.idx()] as i32
    }

    #[inline]
    /// Set MAC1 and associated flags.
    /// 
    /// Returns shifted MAC1 value.
    fn set_mac1(&mut self, data: i64, shift: u8) -> i32 {
        self.set_flag(Flag::MAC1PosOvf, data > 0x7FF_FFFF_FFFF);
        self.set_flag(Flag::MAC1NegOvf, data < -0x800_0000_0000);
        let mac1 = (data >> shift) as i32;
        self.regs[Reg::MAC1.idx()] = data as u32;
        mac1 as i32
    }

    #[inline]
    /// Set IR1 and associated flag.
    /// Also clamps IR1 value.
    /// 
    /// Returns final IR1 value.
    fn set_ir1(&mut self, mac1: i32, ir_unsigned: bool) -> i64 {
        // TODO: reduce branches
        let ir1 = if mac1 > 0x7FFF {
            self.insert_flag(Flag::IR1Sat);
            0x7FFF_i16
        } else if mac1 < -0x8000 {
            self.set_flag(Flag::IR1Sat, if ir_unsigned {mac1 < 0} else {true});
            -0x8000_i16
        } else {
            self.set_flag(Flag::IR1Sat, ir_unsigned && mac1 < 0);
            mac1 as i16
        };
        self.regs[Reg::IR1.idx()] = ir1 as u32; // TODO: sign extend?
        ir1 as i64
    }

    #[inline]
    /// Set MAC2 and associated flags.
    /// 
    /// Returns shifted MAC2 value.
    fn set_mac2(&mut self, data: i64, shift: u8) -> i32 {
        self.set_flag(Flag::MAC2PosOvf, data > 0x7FF_FFFF_FFFF);
        self.set_flag(Flag::MAC2NegOvf, data < -0x800_0000_0000);
        let mac2 = (data >> shift) as i32;
        self.regs[Reg::MAC2.idx()] = data as u32;
        mac2 as i32
    }

    #[inline]
    /// Set IR2 and associated flag.
    /// Also clamps IR2 value.
    /// 
    /// Returns final IR2 value.
    fn set_ir2(&mut self, mac2: i32, ir_unsigned: bool) -> i64 {
        // TODO: reduce branches
        let ir2 = if mac2 > 0x7FFF {
            self.insert_flag(Flag::IR2Sat);
            0x7FFF_i16
        } else if mac2 < -0x8000 {
            self.set_flag(Flag::IR2Sat, if ir_unsigned {mac2 < 0} else {true});
            -0x8000_i16
        } else {
            self.set_flag(Flag::IR2Sat, ir_unsigned && mac2 < 0);
            mac2 as i16
        };
        self.regs[Reg::IR2.idx()] = ir2 as u32; // TODO: sign extend?
        ir2 as i64
    }

    #[inline]
    /// Set MAC3 and associated flags.
    /// 
    /// Returns shifted MAC3 value.
    fn set_mac3(&mut self, data: i64, shift: u8) -> i32 {
        self.set_flag(Flag::MAC3PosOvf, data > 0x7FF_FFFF_FFFF);
        self.set_flag(Flag::MAC3NegOvf, data < -0x800_0000_0000);
        let mac3 = (data >> shift) as i32;
        self.regs[Reg::MAC3.idx()] = data as u32;
        mac3 as i32
    }

    #[inline]
    /// Set IR3 and associated flag.
    /// Also clamps IR3 value.
    /// 
    /// Returns final IR3 value.
    fn set_ir3(&mut self, mac3: i32, ir_unsigned: bool) -> i64 {
        // TODO: reduce branches
        let ir3 = if mac3 > 0x7FFF {
            self.insert_flag(Flag::IR3Sat);
            0x7FFF_i16
        } else if mac3 < -0x8000 {
            self.set_flag(Flag::IR3Sat, if ir_unsigned {mac3 < 0} else {true});
            -0x8000_i16
        } else {
            self.set_flag(Flag::IR3Sat, ir_unsigned && mac3 < 0);
            mac3 as i16
        };
        self.regs[Reg::IR3.idx()] = ir3 as u32; // TODO: sign extend?
        ir3 as i64
    }

    #[inline]
    fn set_mac0(&mut self, data: i64) {
        self.regs[Reg::MAC0.idx()] = data as i32 as u32;
        self.set_flag(Flag::MAC0PosOvf, data > 0x7FFF_FFFF);
        self.set_flag(Flag::MAC0NegOvf, data < -0x8000_0000);
    }
    
    /// Rotate, translate, project for 1 vertex.
    /// 
    /// Returns divide value for use in depth queueing.
    fn rtp(&mut self, shift: u8, ir_unsigned: bool, vx: i64, vy: i64, vz: i64) -> i64 {
        use Reg::*;
        use Control::*;
        // Shift stack.
        self.regs[SZ0.idx()] = self.regs[SZ1.idx()];
        self.regs[SZ1.idx()] = self.regs[SZ2.idx()];
        self.regs[SZ2.idx()] = self.regs[SZ3.idx()];
        self.regs[SXY0.idx()] = self.regs[SXY1.idx()];
        self.regs[SXY1.idx()] = self.regs[SXY2.idx()];
        let ir1 = {
            let rt11 = self.get_control_i16_hi(RT11_12) as i64;
            let rt12 = self.get_control_i16_lo(RT11_12) as i64;
            let rt13 = self.get_control_i16_hi(RT13_21) as i64;
            let trx = self.get_control_i32(TRX) as i64;
            let mac1 = (trx << 12) + rt11 * vx + rt12 * vy + rt13 * vz;
            let mac1 = self.set_mac1(mac1, shift);
            self.set_ir1(mac1, ir_unsigned)
        };
        let ir2 = {
            let rt21 = self.get_control_i16_lo(RT13_21) as i64;
            let rt22 = self.get_control_i16_hi(RT22_23) as i64;
            let rt23 = self.get_control_i16_lo(RT22_23) as i64;
            let _try = self.get_control_i32(TRY) as i64;
            let mac2 = (_try << 12) + rt21 * vx + rt22 * vy + rt23 * vz;
            let mac2 = self.set_mac2(mac2, shift);
            self.set_ir2(mac2, ir_unsigned)
        };
        let sz3 = {
            let rt31 = self.get_control_i16_hi(RT31_32) as i64;
            let rt32 = self.get_control_i16_lo(RT31_32) as i64;
            let rt33 = self.get_control_i16_hi(RT33) as i64;
            let trz = self.get_control_i32(TRZ) as i64;
            let mac3 = (trz << 12) + rt31 * vx + rt32 * vy + rt33 * vz;
            let mac3 = self.set_mac3(mac3, shift);
            self.set_ir3(mac3, ir_unsigned);
            let sz3 = mac3.clamp(0, 0xFFFF);
            self.regs[SZ3.idx()] = sz3 as u32;
            self.set_flag(Flag::SZ3Sat, (mac3 as u32) > 0xFFFF);
            sz3 as i64
        };
        let h = self.get_control_u16_lo(H) as i64;
        // Unsigned divide
        let div = if sz3 == 0 {
            self.insert_flag(Flag::DivOvf);
            0x1_FFFF
        } else {
            let div = ((h << 17) / sz3 + 1) >> 1;
            if div > 0x1_FFFF {
                self.insert_flag(Flag::DivOvf);
                0x1_FFFF
            } else {
                div
            }
        };
        let screen_x = {
            let x_offset = self.get_control_i32(OFX) as i64;
            let mac0 = div * ir1 + x_offset;
            self.set_mac0(mac0);
            let screen_x = mac0 >> 16;
            if screen_x > 0x3FF {
                self.insert_flag(Flag::SXSat);
                0x3FF_i16
            } else if screen_x < -0x400 {
                self.insert_flag(Flag::SXSat);
                -0x400_i16
            } else {
                screen_x as i16
            }
        };
        let screen_y = {
            let y_offset = self.get_control_i32(OFY) as i64;
            let mac0 = div * ir2 + y_offset;
            self.set_mac0(mac0);
            let screen_y = mac0 >> 16;
            if screen_y > 0x3FF {
                self.insert_flag(Flag::SYSat);
                0x3FF_i16
            } else if screen_y < -0x400 {
                self.insert_flag(Flag::SYSat);
                -0x400_i16
            } else {
                screen_y as i16
            }
        };
        self.regs[SXY2.idx()] = (screen_x as u32) | ((screen_y as u32) << 16);
        div
    }

    fn depth_queue(&mut self, div: i64) {
        use Reg::*;
        use Control::*;
        let mac0 = {
            let depth_queue_a = self.get_control_i16_lo(DQA) as i64;
            let depth_queue_b = self.get_control_i32(DQB) as i64;
            let mac0 = div * depth_queue_a + depth_queue_b;
            self.set_mac0(mac0);
            mac0 as i32 // TODO: clip or shift by 12?
        };
        self.regs[MAC0.idx()] = mac0 as u32;
        let ir0 = {
            let ir0 = mac0 >> 12;
            if ir0 > 0x1000 {
                self.insert_flag(Flag::IR0Sat);
                0x1000_i16
            } else if ir0 < 0 {
                self.insert_flag(Flag::IR0Sat);
                0_i16
            } else {
                ir0 as i16
            }
        };
        self.regs[IR0.idx()] = ir0 as u32;
    }

    /// Multiply normal by light direction, then multiply the result by color matrix.
    /// 
    /// Returns mac[1,2,3]
    fn normal_color_mul(&mut self, shift: u8, ir_unsigned: bool, nx: i64, ny: i64, nz: i64) -> [i32; 3] {
        use Control::*;
        // Light direction calculation.
        let ir1 = {
            let l11 = self.get_control_i16_hi(L11_12) as i64;
            let l12 = self.get_control_i16_lo(L11_12) as i64;
            let l13 = self.get_control_i16_hi(L13_21) as i64;
            let mac1 = self.set_mac1(l11 * nx + l12 * ny + l13 * nz, shift);
            self.set_ir1(mac1, ir_unsigned)
        };
        let ir2 = {
            let l21 = self.get_control_i16_lo(L13_21) as i64;
            let l22 = self.get_control_i16_hi(L22_23) as i64;
            let l23 = self.get_control_i16_lo(L22_23) as i64;
            let mac2 = self.set_mac2(l21 * nx + l22 * ny + l23 * nz, shift);
            self.set_ir2(mac2, ir_unsigned)
        };
        let ir3 = {
            let l31 = self.get_control_i16_hi(L31_32) as i64;
            let l32 = self.get_control_i16_lo(L31_32) as i64;
            let l33 = self.get_control_i16_hi(L33) as i64;
            let mac3 = self.set_mac3(l31 * nx + l32 * ny + l33 * nz, shift);
            self.set_ir3(mac3, ir_unsigned)
        };
        // Color matrix calculation.
        let mac1 = {
            let lr1 = self.get_control_i16_hi(LR1R2) as i64;
            let lr2 = self.get_control_i16_lo(LR1R2) as i64;
            let lr3 = self.get_control_i16_hi(LR3G1) as i64;
            let rbk = self.get_control_i32(RBK) as i64;
            self.set_mac1(lr1 * ir1 + lr2 * ir2 + lr3 * ir3 + (rbk << 12), shift)
        };
        let mac2 = {
            let lg1 = self.get_control_i16_lo(LR3G1) as i64;
            let lg2 = self.get_control_i16_hi(LG2G3) as i64;
            let lg3 = self.get_control_i16_lo(LG2G3) as i64;
            let gbk = self.get_control_i32(GBK) as i64;
            self.set_mac2(lg1 * ir1 + lg2 * ir2 + lg3 * ir3 + (gbk << 12), shift)
        };
        let mac3 = {
            let lb1 = self.get_control_i16_hi(LB1B2) as i64;
            let lb2 = self.get_control_i16_lo(LB1B2) as i64;
            let lb3 = self.get_control_i16_hi(LB3) as i64;
            let bbk = self.get_control_i32(BBK) as i64;
            self.set_mac3(lb1 * ir1 + lb2 * ir2 + lb3 * ir3 + (bbk << 12), shift)
        };
        [mac1, mac2, mac3]
    }

    /// Normal * color calculation.
    /// Shifts color FIFO.
    fn normal_color(&mut self, shift: u8, ir_unsigned: bool, nx: i64, ny: i64, nz: i64) {
        use Reg::*;
        // Shift color FIFO.
        self.regs[RGB0.idx()] = self.regs[RGB1.idx()];
        self.regs[RGB1.idx()] = self.regs[RGB2.idx()];
        let [mac1, mac2, mac3] = self.normal_color_mul(shift, ir_unsigned, nx, ny, nz);
        let r = {
            self.set_ir1(mac1, ir_unsigned);
            let r = mac1 >> 4;
            self.set_flag(Flag::ColRSat, r > 0xFF);
            r.clamp(0, 0xFF) as u32
        };
        let g = {
            self.set_ir2(mac2, ir_unsigned);
            let g = mac2 >> 4;
            self.set_flag(Flag::ColGSat, g > 0xFF);
            g.clamp(0, 0xFF) as u32
        };
        let b = {
            self.set_ir3(mac3, ir_unsigned);
            let b = mac3 >> 4;
            self.set_flag(Flag::ColBSat, b > 0xFF);
            b.clamp(0, 0xFF) as u32
        };
        let code = self.regs[RGBC.idx()] & 0xFF00_0000;
        self.regs[RGB2.idx()] = code | (r << 16) | (g << 8) | b;
    }

    /// Normal * color calculation, with color factor.
    /// Shifts color FIFO.
    fn normal_color_color(&mut self, shift: u8, ir_unsigned: bool, nx: i64, ny: i64, nz: i64) {
        use Reg::*;
        // Shift color FIFO.
        self.regs[RGB0.idx()] = self.regs[RGB1.idx()];
        self.regs[RGB1.idx()] = self.regs[RGB2.idx()];
        let [mac1, mac2, mac3] = self.normal_color_mul(shift, ir_unsigned, nx, ny, nz);
        let rgbc = self.regs[RGBC.idx()];
        let r = {
            let r = ((rgbc >> 16) & 0xFF) as i64;
            let ir1 = self.set_ir1(mac1, ir_unsigned);
            let mac1 = self.set_mac1(ir1 * r, shift) >> 4;
            self.set_ir1(mac1, ir_unsigned);
            self.set_flag(Flag::ColRSat, mac1 > 0xFF);
            mac1.clamp(0, 0xFF) as u32
        };
        let g = {
            let g = ((rgbc >> 8) & 0xFF) as i64;
            let ir2 = self.set_ir2(mac2, ir_unsigned);
            let mac2 = self.set_mac2(ir2 * g, shift) >> 4;
            self.set_ir2(mac2, ir_unsigned);
            self.set_flag(Flag::ColGSat, mac2 > 0xFF);
            mac2.clamp(0, 0xFF) as u32
        };
        let b = {
            let b = (rgbc & 0xFF) as i64;
            let ir3 = self.set_ir3(mac3, ir_unsigned);
            let mac3 = self.set_mac3(ir3 * b, shift) >> 4;
            self.set_ir3(mac3, ir_unsigned);
            self.set_flag(Flag::ColBSat, mac3 > 0xFF);
            mac3.clamp(0, 0xFF) as u32
        };
        let code = rgbc & 0xFF00_0000;
        self.regs[RGB2.idx()] = code | (r << 16) | (g << 8) | b;
    }

    /// Normal * color calculation, with depth queue.
    /// Shifts color FIFO.
    fn normal_color_depth(&mut self, shift: u8, ir_unsigned: bool, nx: i64, ny: i64, nz: i64) {
        use Reg::*;
        use Control::*;
        // Shift color FIFO.
        self.regs[RGB0.idx()] = self.regs[RGB1.idx()];
        self.regs[RGB1.idx()] = self.regs[RGB2.idx()];
        let [mac1, mac2, mac3] = self.normal_color_mul(shift, ir_unsigned, nx, ny, nz);
        let rgbc = self.regs[RGBC.idx()];
        let ir0 = self.get_reg_i16_lo(IR0) as i64;
        let r = {
            let r = ((rgbc >> 16) & 0xFF) as i64;
            let ir1 = self.set_ir1(mac1, ir_unsigned);
            let mac1 = (self.set_mac1(ir1 * r, 0) >> 4) as i64;
            let rfc = self.get_control_i32(RFC) as i64;
            let ir1 = self.set_ir1((((rfc << 12) - mac1) >> shift) as i32, false);
            let mac1 = self.set_mac1(ir1 * ir0 + mac1, shift);
            self.set_ir1(mac1, ir_unsigned);
            self.set_flag(Flag::ColRSat, mac1 > 0xFF);
            mac1.clamp(0, 0xFF) as u32
        };
        let g = {
            let g = ((rgbc >> 8) & 0xFF) as i64;
            let ir2 = self.set_ir2(mac2, ir_unsigned);
            let mac2 = (self.set_mac2(ir2 * g, 0) >> 4) as i64;
            let gfc = self.get_control_i32(GFC) as i64;
            let ir2 = self.set_ir2((((gfc << 12) - mac2) >> shift) as i32, false);
            let mac2 = self.set_mac2(ir2 * ir0 + mac2, shift);
            self.set_ir2(mac2, ir_unsigned);
            self.set_flag(Flag::ColGSat, mac2 > 0xFF);
            mac2.clamp(0, 0xFF) as u32
        };
        let b = {
            let b = (rgbc & 0xFF) as i64;
            let ir3 = self.set_ir3(mac3, ir_unsigned);
            let mac3 = (self.set_mac3(ir3 * b, 0) >> 4) as i64;
            let bfc = self.get_control_i32(BFC) as i64;
            let ir3 = self.set_ir3((((bfc << 12) - mac3) >> shift) as i32, false);
            let mac3 = self.set_mac3(ir3 * ir0 + mac3, shift);
            self.set_ir3(mac3, ir_unsigned);
            self.set_flag(Flag::ColBSat, mac3 > 0xFF);
            mac3.clamp(0, 0xFF) as u32
        };
        let code = rgbc & 0xFF00_0000;
        self.regs[RGB2.idx()] = code | (r << 16) | (g << 8) | b;
    }
}

// Commands.
impl GTE {
    /// Rotate, translate, perspective transformation (single).
    fn rtps(&mut self, shift: u8, ir_unsigned: bool) {
        use Reg::*;
        let vx0 = self.get_reg_i16_lo(VXY0) as i64;
        let vy0 = self.get_reg_i16_hi(VXY0) as i64;
        let vz0 = self.get_reg_i16_lo(VZ0) as i64;
        let div = self.rtp(shift, ir_unsigned, vx0, vy0, vz0);
        self.depth_queue(div);
    }

    /// Rotate, translate, perspective transformation (triple).
    fn rtpt(&mut self, shift: u8, ir_unsigned: bool) {
        use Reg::*;
        let vx0 = self.get_reg_i16_lo(VXY0) as i64;
        let vy0 = self.get_reg_i16_hi(VXY0) as i64;
        let vz0 = self.get_reg_i16_lo(VZ0) as i64;
        self.rtp(shift, ir_unsigned, vx0, vy0, vz0);
        let vx1 = self.get_reg_i16_lo(VXY1) as i64;
        let vy1 = self.get_reg_i16_hi(VXY1) as i64;
        let vz1 = self.get_reg_i16_lo(VZ1) as i64;
        self.rtp(shift, ir_unsigned, vx1, vy1, vz1);
        let vx2 = self.get_reg_i16_lo(VXY2) as i64;
        let vy2 = self.get_reg_i16_hi(VXY2) as i64;
        let vz2 = self.get_reg_i16_lo(VZ2) as i64;
        let div = self.rtp(shift, ir_unsigned, vx2, vy2, vz2);
        self.depth_queue(div);
    }

    /// Normal clipping.
    fn nclip(&mut self) {
        use Reg::*;
        let sx0 = self.get_reg_i16_lo(SXY0) as i64;
        let sy0 = self.get_reg_i16_hi(SXY0) as i64;
        let sx1 = self.get_reg_i16_lo(SXY1) as i64;
        let sy1 = self.get_reg_i16_hi(SXY1) as i64;
        let sx2 = self.get_reg_i16_lo(SXY2) as i64;
        let sy2 = self.get_reg_i16_hi(SXY2) as i64;
        let mac0 = sx0 * sy1 + sx1 * sy2 + sx2 * sy0 - sx0 * sy2 - sx1 * sy0 - sx2 * sy1;
        self.set_mac0(mac0);
    }

    /// Outer product
    fn op(&mut self, shift: u8, ir_unsigned: bool) {
        let ir1 = self.get_reg_i16_lo(Reg::IR1) as i64;
        let ir2 = self.get_reg_i16_lo(Reg::IR2) as i64;
        let ir3 = self.get_reg_i16_lo(Reg::IR3) as i64;
        let d1 = self.get_control_i16_hi(Control::RT11_12) as i64;
        let d2 = self.get_control_i16_hi(Control::RT22_23) as i64;
        let d3 = self.get_control_i16_lo(Control::RT33) as i64;
        let mac1 = self.set_mac1(d2 * ir3 - d3 * ir2, shift);
        self.set_ir1(mac1, ir_unsigned);
        let mac2 = self.set_mac2(d3 * ir1 - d1 * ir3, shift);
        self.set_ir2(mac2, ir_unsigned);
        let mac3 = self.set_mac3(d1 * ir2 - d2 * ir1, shift);
        self.set_ir3(mac3, ir_unsigned);
    }

    /// Depth queueing (single).
    fn dpcs(&mut self) {

    }

    /// Depth queueing (triple).
    fn dpct(&mut self) {

    }

    /// Interpolation.
    fn intpl(&mut self) {

    }

    /// Multiply vector by matrix and add.
    fn mvmva(&mut self, shift: u8, ir_unsigned: bool, mul_mat: u8, mul_vec: u8, trans_vec: u8) {
        use Reg::*;
        use Control::*;
        let m_vec = match mul_vec {
            0 => [
                self.get_reg_i16_lo(VXY0) as i64,
                self.get_reg_i16_hi(VXY0) as i64,
                self.get_reg_i16_lo(VZ0) as i64,
            ],
            1 => [
                self.get_reg_i16_lo(VXY1) as i64,
                self.get_reg_i16_hi(VXY1) as i64,
                self.get_reg_i16_lo(VZ1) as i64,
            ],
            2 => [
                self.get_reg_i16_lo(VXY2) as i64,
                self.get_reg_i16_hi(VXY2) as i64,
                self.get_reg_i16_lo(VZ2) as i64,
            ],
            3 => [
                self.get_reg_i16_lo(IR1) as i64,
                self.get_reg_i16_hi(IR2) as i64,
                self.get_reg_i16_lo(IR3) as i64,
            ],
            _ => unreachable!()
        };
        let mat = match mul_mat {
            0 => [ // Rotation
                self.get_control_i16_hi(RT11_12) as i64,
                self.get_control_i16_lo(RT11_12) as i64,
                self.get_control_i16_hi(RT13_21) as i64,
                self.get_control_i16_lo(RT13_21) as i64,
                self.get_control_i16_hi(RT22_23) as i64,
                self.get_control_i16_lo(RT22_23) as i64,
                self.get_control_i16_hi(RT31_32) as i64,
                self.get_control_i16_lo(RT31_32) as i64,
                self.get_control_i16_hi(RT33) as i64
            ],
            1 => [ // Light
                self.get_control_i16_hi(L11_12) as i64,
                self.get_control_i16_lo(L11_12) as i64,
                self.get_control_i16_hi(L13_21) as i64,
                self.get_control_i16_lo(L13_21) as i64,
                self.get_control_i16_hi(L22_23) as i64,
                self.get_control_i16_lo(L22_23) as i64,
                self.get_control_i16_hi(L31_32) as i64,
                self.get_control_i16_lo(L31_32) as i64,
                self.get_control_i16_hi(L33) as i64
            ],
            2 => [ // Color
                self.get_control_i16_hi(LR1R2) as i64,
                self.get_control_i16_lo(LR1R2) as i64,
                self.get_control_i16_hi(LR3G1) as i64,
                self.get_control_i16_lo(LR3G1) as i64,
                self.get_control_i16_hi(LG2G3) as i64,
                self.get_control_i16_lo(LG2G3) as i64,
                self.get_control_i16_hi(LB1B2) as i64,
                self.get_control_i16_lo(LB1B2) as i64,
                self.get_control_i16_hi(LB3) as i64
            ],
            3 => panic!("can't use mat 3"),
            _ => unreachable!()
        };
        let t_vec = match trans_vec {
            0 => [
                self.get_control_i32(TRX) as i64,
                self.get_control_i32(TRY) as i64,
                self.get_control_i32(TRZ) as i64,
            ],
            1 => [
                self.get_control_i32(RBK) as i64,
                self.get_control_i32(GBK) as i64,
                self.get_control_i32(BBK) as i64,
            ],
            2 => panic!("can't use trans vec 2"),
            3 => [0, 0, 0],
            _ => unreachable!()
        };
        let mac1 = m_vec[0] * mat[0] + m_vec[1] * mat[3] + m_vec[2] * mat[6] + (t_vec[0] << 12);
        let mac1 = self.set_mac1(mac1, shift);
        self.set_ir1(mac1, ir_unsigned);
        let mac2 = m_vec[0] * mat[1] + m_vec[1] * mat[4] + m_vec[2] * mat[7] + (t_vec[1] << 12);
        let mac2 = self.set_mac2(mac2, shift);
        self.set_ir2(mac2, ir_unsigned);
        let mac3 = m_vec[0] * mat[2] + m_vec[1] * mat[5] + m_vec[2] * mat[8] + (t_vec[2] << 12);
        let mac3 = self.set_mac3(mac3, shift);
        self.set_ir3(mac3, ir_unsigned);
    }

    /// Color color.
    fn cc(&mut self) {

    }

    /// Color depth queue.
    fn cdp(&mut self) {

    }

    /// Normal color (single).
    fn ncs(&mut self, shift: u8, ir_unsigned: bool) {
        use Reg::*;
        let vx0 = self.get_reg_i16_lo(VXY0) as i64;
        let vy0 = self.get_reg_i16_hi(VXY0) as i64;
        let vz0 = self.get_reg_i16_lo(VZ0) as i64;
        self.normal_color(shift, ir_unsigned, vx0, vy0, vz0);
    }

    /// Normal color (triple).
    fn nct(&mut self, shift: u8, ir_unsigned: bool) {
        use Reg::*;
        let vx0 = self.get_reg_i16_lo(VXY0) as i64;
        let vy0 = self.get_reg_i16_hi(VXY0) as i64;
        let vz0 = self.get_reg_i16_lo(VZ0) as i64;
        self.normal_color(shift, ir_unsigned, vx0, vy0, vz0);
        let vx1 = self.get_reg_i16_lo(VXY1) as i64;
        let vy1 = self.get_reg_i16_hi(VXY1) as i64;
        let vz1 = self.get_reg_i16_lo(VZ1) as i64;
        self.normal_color(shift, ir_unsigned, vx1, vy1, vz1);
        let vx2 = self.get_reg_i16_lo(VXY2) as i64;
        let vy2 = self.get_reg_i16_hi(VXY2) as i64;
        let vz2 = self.get_reg_i16_lo(VZ2) as i64;
        self.normal_color(shift, ir_unsigned, vx2, vy2, vz2);
    }

    /// Normal color color (single).
    fn nccs(&mut self, shift: u8, ir_unsigned: bool) {
        use Reg::*;
        let vx0 = self.get_reg_i16_lo(VXY0) as i64;
        let vy0 = self.get_reg_i16_hi(VXY0) as i64;
        let vz0 = self.get_reg_i16_lo(VZ0) as i64;
        self.normal_color_color(shift, ir_unsigned, vx0, vy0, vz0);
    }

    /// Normal color color (triple).
    fn ncct(&mut self, shift: u8, ir_unsigned: bool) {
        use Reg::*;
        let vx0 = self.get_reg_i16_lo(VXY0) as i64;
        let vy0 = self.get_reg_i16_hi(VXY0) as i64;
        let vz0 = self.get_reg_i16_lo(VZ0) as i64;
        self.normal_color_color(shift, ir_unsigned, vx0, vy0, vz0);
        let vx1 = self.get_reg_i16_lo(VXY1) as i64;
        let vy1 = self.get_reg_i16_hi(VXY1) as i64;
        let vz1 = self.get_reg_i16_lo(VZ1) as i64;
        self.normal_color_color(shift, ir_unsigned, vx1, vy1, vz1);
        let vx2 = self.get_reg_i16_lo(VXY2) as i64;
        let vy2 = self.get_reg_i16_hi(VXY2) as i64;
        let vz2 = self.get_reg_i16_lo(VZ2) as i64;
        self.normal_color_color(shift, ir_unsigned, vx2, vy2, vz2);
    }

    /// Normal color depth queue (single).
    fn ncds(&mut self, shift: u8, ir_unsigned: bool) {
        use Reg::*;
        let vx0 = self.get_reg_i16_lo(VXY0) as i64;
        let vy0 = self.get_reg_i16_hi(VXY0) as i64;
        let vz0 = self.get_reg_i16_lo(VZ0) as i64;
        self.normal_color_depth(shift, ir_unsigned, vx0, vy0, vz0);
    }

    /// Normal color depth queue (triple).
    fn ncdt(&mut self, shift: u8, ir_unsigned: bool) {
        use Reg::*;
        let vx0 = self.get_reg_i16_lo(VXY0) as i64;
        let vy0 = self.get_reg_i16_hi(VXY0) as i64;
        let vz0 = self.get_reg_i16_lo(VZ0) as i64;
        self.normal_color_depth(shift, ir_unsigned, vx0, vy0, vz0);
        let vx1 = self.get_reg_i16_lo(VXY1) as i64;
        let vy1 = self.get_reg_i16_hi(VXY1) as i64;
        let vz1 = self.get_reg_i16_lo(VZ1) as i64;
        self.normal_color_depth(shift, ir_unsigned, vx1, vy1, vz1);
        let vx2 = self.get_reg_i16_lo(VXY2) as i64;
        let vy2 = self.get_reg_i16_hi(VXY2) as i64;
        let vz2 = self.get_reg_i16_lo(VZ2) as i64;
        self.normal_color_depth(shift, ir_unsigned, vx2, vy2, vz2);
    }

    /// Square IR.
    fn sqr(&mut self, shift: u8, ir_unsigned: bool) {
        let ir1 = self.get_reg_i16_lo(Reg::IR1) as i64;
        let ir2 = self.get_reg_i16_lo(Reg::IR2) as i64;
        let ir3 = self.get_reg_i16_lo(Reg::IR3) as i64;
        let mac1 = self.set_mac1(ir1 * ir1, shift);
        self.set_ir1(mac1, ir_unsigned);
        let mac2 = self.set_mac2(ir2 * ir2, shift);
        self.set_ir2(mac2, ir_unsigned);
        let mac3 = self.set_mac3(ir3 * ir3, shift);
        self.set_ir3(mac3, ir_unsigned);
    }

    /// Depth cue color light.
    fn dcpl(&mut self) {

    }

    /// Average 3 Z values.
    fn avsz3(&mut self) {
        let zsf3 = self.get_control_i16_lo(Control::ZSF3) as i64;
        let sz1 = self.get_reg_u16_lo(Reg::SZ1) as i64;
        let sz2 = self.get_reg_u16_lo(Reg::SZ2) as i64;
        let sz3 = self.get_reg_u16_lo(Reg::SZ3) as i64;
        let mac0 = zsf3 * (sz1 + sz2 + sz3);
        self.set_mac0(mac0);
        let otz = mac0 >> 12;
        self.regs[Reg::OTZ.idx()] = otz.clamp(0, 0xFFFF) as u32;
        self.set_flag(Flag::SZ3Sat, (otz as u32) > 0xFFFF);
    }

    /// Average 4 Z values.
    fn avsz4(&mut self) {
        let zsf4 = self.get_control_i16_lo(Control::ZSF4) as i64;
        let sz0 = self.get_reg_u16_lo(Reg::SZ0) as i64;
        let sz1 = self.get_reg_u16_lo(Reg::SZ1) as i64;
        let sz2 = self.get_reg_u16_lo(Reg::SZ2) as i64;
        let sz3 = self.get_reg_u16_lo(Reg::SZ3) as i64;
        let mac0 = zsf4 * (sz0 + sz1 + sz2 + sz3);
        self.set_mac0(mac0);
        let otz = mac0 >> 12;
        self.regs[Reg::OTZ.idx()] = otz.clamp(0, 0xFFFF) as u32;
        self.set_flag(Flag::SZ3Sat, (otz as u32) > 0xFFFF);
    }

    /// General purpose interpolation.
    fn gpf(&mut self) {

    }

    /// General purpose interpolation with base.
    fn gpl(&mut self) {

    }
}