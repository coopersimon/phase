use crate::utils::{bits::*, interface::MemInterface};

bitflags::bitflags! {
    #[derive(Clone, Copy, Default)]
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
    pub fn trigger_irq(&mut self, int: Interrupt) {
        if !int.is_empty() {
            //println!("trigger int: {:X}", int.bits());
        }
        self.status.insert(int);
    }

    /// Returns true if any pending interrupts overlap with the interrupt mask.
    pub fn check_irq(&self) -> bool {
        self.status.intersects(self.mask)
    }

    fn acknowledge_irq(&mut self, data: u32) {
        let ack = Interrupt::from_bits_truncate(!data);
        self.status.remove(ack);
    }

    fn set_mask(&mut self, data: u32) {
        //println!("set i mask: {:X}", data);
        self.mask = Interrupt::from_bits_truncate(data);
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
            0x1F801070 => self.acknowledge_irq(data),
            0x1F801074 => self.set_mask(data),
            _ => unreachable!()
        }
    }
}