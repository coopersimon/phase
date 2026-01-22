mod ram;
mod bios;
mod control;
mod dma;

use mips::mem::Mem32;
use ram::RAM;
use bios::BIOS;
use control::MemControl;
use dma::DMA;

use crate::PlayStationConfig;
use crate::utils::interface::MemInterface;
use crate::interrupt::InterruptControl;
use crate::timer::Timers;
use crate::cdrom::CDROM;

pub struct MemBus {
    control: MemControl,
    main_ram: RAM,
    scratchpad: RAM,
    bios: BIOS,
    interrupts: InterruptControl,

    timers: Timers,
    dma: DMA,
    cdrom: CDROM,
}

impl MemBus {
    pub fn new(config: &PlayStationConfig) -> Self {
        let bios = BIOS::new(Some(&config.bios_path)).expect("error loading BIOS"); // TODO: handle error.
        Self {
            control: MemControl::new(),
            main_ram: RAM::new(2048 * 1024), // 2MB
            scratchpad: RAM::new(1024),
            bios,
            interrupts: InterruptControl::new(),

            timers: Timers::new(),
            dma: DMA::new(),
            cdrom: CDROM::new(),
        }
    }

    /// Clock internally, and set interrupt bits.
    fn do_clock(&mut self, cycles: usize) {
        let hblank = false;
        let vblank = false;

        let dma_irq = self.dma.check_irq();

        let timer_irq = self.timers.clock(cycles, hblank, vblank);

        let cd_irq = self.cdrom.clock(cycles);

        self.interrupts.trigger_irq(
            dma_irq |
            timer_irq |
            cd_irq
        );
    }

    /// Do DMA transfers, if any are ready.
    /// 
    /// This will take control from the CPU and clock until the
    /// DMA transfers are complete.
    fn do_dma(&mut self) {
        while let Some(transfer) = self.dma.get_transfer() {
            let data = self.read_word(transfer.src_addr);
            self.write_word(transfer.dst_addr, data);
            // TODO: do_clock?
        }
    }
}

impl Mem32 for MemBus {
    type Addr = u32;
    const LITTLE_ENDIAN: bool = true;

    fn clock(&mut self, cycles: usize) -> u8 {
        self.do_clock(cycles);

        self.do_dma();

        if self.interrupts.check_irq() {
            0x04 // Interrupt bit 2 is used for all external hardware IRQs.
        } else {
            0x00
        }
    }

    fn read_byte(&mut self, addr: Self::Addr) -> u8 {
        match addr {
            0x0000_0000..=0x007F_FFFF => self.main_ram.read_byte(addr & 0x1F_FFFF),
            0x1F80_0000..=0x1F80_03FF => self.scratchpad.read_byte(addr & 0x3FF),
            0x1F80_1000..=0x1F80_1FFF => self.mut_io_device(addr).map(|d| d.read_byte(addr)).unwrap_or_default(),
            0x1FC0_0000..=0x1FC7_FFFF => self.bios.read_byte(addr & 0x7_FFFF),
            _ => panic!("read invalid address {:X}", addr),
        }
    }

    fn write_byte(&mut self, addr: Self::Addr, data: u8) {
        match addr {
            0x0000_0000..=0x007F_FFFF => self.main_ram.write_byte(addr & 0x1F_FFFF, data),
            0x1F80_0000..=0x1F80_03FF => self.scratchpad.write_byte(addr & 0x3FF, data),
            0x1F80_1000..=0x1F80_1FFF => self.mut_io_device(addr).map(|d| d.write_byte(addr, data)).unwrap_or_default(),
            0x1FC0_0000..=0x1FC7_FFFF => {}, // BIOS
            _ => panic!("write invalid address {:X}", addr),
        }
    }

    fn read_halfword(&mut self, addr: Self::Addr) -> u16 {
        match addr {
            0x0000_0000..=0x007F_FFFF => self.main_ram.read_halfword(addr & 0x1F_FFFF),
            0x1F80_0000..=0x1F80_03FF => self.scratchpad.read_halfword(addr & 0x3FF),
            0x1F80_1000..=0x1F80_1FFF => self.mut_io_device(addr).map(|d| d.read_halfword(addr)).unwrap_or_default(),
            0x1FC0_0000..=0x1FC7_FFFF => self.bios.read_halfword(addr & 0x7_FFFF),
            _ => panic!("read invalid address {:X}", addr),
        }
    }

    fn write_halfword(&mut self, addr: Self::Addr, data: u16) {
        match addr {
            0x0000_0000..=0x007F_FFFF => self.main_ram.write_halfword(addr & 0x1F_FFFF, data),
            0x1F80_0000..=0x1F80_03FF => self.scratchpad.write_halfword(addr & 0x3FF, data),
            0x1F80_1000..=0x1F80_1FFF => self.mut_io_device(addr).map(|d| d.write_halfword(addr, data)).unwrap_or_default(),
            0x1FC0_0000..=0x1FC7_FFFF => {}, // BIOS
            _ => panic!("write invalid address {:X}", addr),
        }
    }

    fn read_word(&mut self, addr: Self::Addr) -> u32 {
        match addr {
            0x0000_0000..=0x007F_FFFF => self.main_ram.read_word(addr & 0x1F_FFFF),
            0x1F80_0000..=0x1F80_03FF => self.scratchpad.read_word(addr & 0x3FF),
            0x1F80_1000..=0x1F80_1FFF => self.mut_io_device(addr).map(|d| d.read_word(addr)).unwrap_or_default(),
            0x1FC0_0000..=0x1FC7_FFFF => self.bios.read_word(addr & 0x7_FFFF),
            _ => panic!("read invalid address {:X}", addr),
        }
    }

    fn write_word(&mut self, addr: Self::Addr, data: u32) {
        match addr {
            0x0000_0000..=0x007F_FFFF => self.main_ram.write_word(addr & 0x1F_FFFF, data),
            0x1F80_0000..=0x1F80_03FF => self.scratchpad.write_word(addr & 0x3FF, data),
            0x1F80_1000..=0x1F80_1FFF => self.mut_io_device(addr).map(|d| d.write_word(addr, data)).unwrap_or_default(),
            0x1FC0_0000..=0x1FC7_FFFF => {}, // BIOS
            _ => panic!("write invalid address {:X}", addr),
        }
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
            0x1F80_1080..=0x1F80_10FF => Some(&mut self.dma),
            0x1F80_1100..=0x1F80_1129 => Some(&mut self.timers),
            0x1F80_1800..=0x1F80_1807 => Some(&mut self.cdrom),
            0x1F80_1810..=0x1F80_1817 => None, // GPU
            0x1F80_1820..=0x1F80_1827 => None, // MDEC
            0x1F80_1C00..=0x1F80_1C0F => None, // SPU
            0x1F80_1D80..=0x1F80_1FFF => None, // SPU
            _ => panic!("no such I/O device at {:X}", addr),
        }
    }
}