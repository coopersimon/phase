mod ram;
mod bios;
mod control;

use mips::mem::Mem32;
use ram::RAM;
use bios::BIOS;
use control::MemControl;

use crate::interrupt::InterruptControl;
use crate::utils::interface::MemInterface;


pub struct MemBus {
    control: MemControl,
    main_ram: RAM,
    scratchpad: RAM,
    bios: BIOS,
    interrupts: InterruptControl,
}

impl MemBus {
    pub fn new() -> Self {
        let bios = BIOS::new();
        Self {
            control: MemControl::new(),
            main_ram: RAM::new(2048 * 1024), // 2MB
            scratchpad: RAM::new(1024),
            bios,
            interrupts: InterruptControl::new(),
        }
    }
}

impl Mem32 for MemBus {
    type Addr = u32;
    const LITTLE_ENDIAN: bool = true;

    fn clock(&mut self, _cycles: usize) -> u8 {
        if self.interrupts.check_interrupt() {
            0x04
        } else {
            0x00
        }
    }

    fn read_byte(&mut self, addr: Self::Addr) -> u8 {
        match addr {
            0x0000_0000..=0x007F_FFFF => self.main_ram.read_byte(addr & 0x1F_FFFF),
            0x1F80_0000..=0x1F80_03FF => self.scratchpad.read_byte(addr & 0x3FF),
            0x1F80_1000..=0x1F80_1FFF => self.mut_io_device(addr).map(|d| d.read_byte(addr)).unwrap_or_default(), // I/O
            0x1FC0_0000..=0x1FC7_FFFF => self.bios.read_byte(addr & 0x7_FFFF), // BIOS
            _ => panic!("read invalid address {:X}", addr),
        }
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

impl MemBus {
    /// Mutably reference an I/O device.
    fn mut_io_device<'a>(&'a mut self, addr: u32) -> Option<&'a mut dyn MemInterface> {
        match addr {
            0x1F80_1000..=0x1F80_1023 => Some(&mut self.control),
            0x1F80_1040..=0x1F80_105F => None, // Peripheral
            0x1F80_1060..=0x1F80_1063 => Some(&mut self.control),
            0x1F80_1070..=0x1F80_1077 => Some(&mut self.interrupts),
            0x1F80_1080..=0x1F80_10FF => None, // DMA
            0x1F80_1100..=0x1F80_1129 => None, // Timers
            0x1F80_1800..=0x1F80_1803 => None, // CD-ROM
            0x1F80_1810..=0x1F80_1817 => None, // GPU
            0x1F80_1820..=0x1F80_1827 => None, // MDEC
            0x1F80_1C00..=0x1F80_1C0F => None, // SPU
            0x1F80_1D80..=0x1F80_1FFF => None, // SPU
            _ => panic!("no such I/O device at {:X}", addr),
        }
    }
}