use crate::utils::{bits::*, interface::MemInterface};

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    pub struct Interrupt: u32 {
        const ControllerLightpen    = bit!(10);
        const SPU                   = bit!(9);
        const SIO                   = bit!(8);
        const Peripheral            = bit!(7);
        const Timer2                = bit!(6);
        const Timer1                = bit!(5);
        const Timer0                = bit!(4);
        const DMA                   = bit!(3);
        const CDROM                 = bit!(2);
        const GPU                   = bit!(1);
        const VBLANK                = bit!(0);
    }
}

pub struct InterruptControl {
    status: Interrupt,
    mask: Interrupt
}

impl InterruptControl {
    pub fn new() -> Self {
        Self {
            status: Interrupt::empty(),
            mask: Interrupt::empty()
        }
    }

    /// Trigger newly occurring interrupts.
    /// 
    /// Returns true if any overlap with the interrupt mask.
    pub fn trigger_interrupt(&mut self, int: Interrupt) -> bool {
        self.status.insert(int);
        self.status.intersects(self.mask)
    }
}

impl MemInterface for InterruptControl {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x1F801070 => self.status.bits(),
            0x1F801074 => self.mask.bits(),
            _ => unreachable!()
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x1F801070 => self.status.remove(!Interrupt::from_bits_truncate(data)),
            0x1F801074 => self.mask = Interrupt::from_bits_truncate(data),
            _ => unreachable!()
        }
    }
}