use super::*;

use Reg::*;
use Control::*;

fn make_u32(lo: i16, hi: i16) -> u32 {
    (lo as u32) | ((hi as u32) << 16)
}

#[test]
fn rtps() {
    let mut gte = GTE::new();
    // In
    gte.regs[VXY0.idx()] = make_u32(0x1_800, 0x0_800);
    gte.regs[VZ0.idx()] = 0x1_000;
    gte.control_regs[RT11_12.idx()] = make_u32(0x1_000, 0x0_000);
    gte.control_regs[RT13_21.idx()] = make_u32(0x0_000, 0x0_000);
    gte.control_regs[RT22_23.idx()] = make_u32(0x1_000, 0x0_000);
    gte.control_regs[RT31_32.idx()] = make_u32(0x0_000, 0x0_000);
    gte.control_regs[RT33.idx()] = 0x1_000;
    gte.control_regs[TRX.idx()] = 0x2;
    gte.control_regs[TRY.idx()] = 0x2;
    gte.control_regs[TRZ.idx()] = 0x2;
    gte.control_regs[H.idx()] = 0x100; // ?
    gte.control_regs[OFX.idx()] = 0x0;
    gte.control_regs[OFY.idx()] = 0x0;
    gte.control_regs[DQA.idx()] = 0x1; // ?
    gte.control_regs[DQB.idx()] = 0x0;

    gte.rtps(12, false);

    // Out
    assert_eq!(gte.regs[IR0.idx()], 0x0);
    assert_eq!(gte.regs[IR1.idx()], 0x1802);
    assert_eq!(gte.regs[IR2.idx()], 0x0802);
    assert_eq!(gte.regs[IR3.idx()], 0x1002);
    assert_eq!(gte.regs[SXY2.idx()], 0x0080_017F);
    assert_eq!(gte.regs[SZ3.idx()], 0x1002);
    assert_eq!(gte.regs[MAC0.idx()], 0x00000FFE);
    assert_eq!(gte.regs[MAC1.idx()], 0x01802000);
    assert_eq!(gte.regs[MAC2.idx()], 0x00802000);
    assert_eq!(gte.regs[MAC3.idx()], 0x01002000);
    assert_eq!(gte.regs[FLAG.idx()], 0x0);
}