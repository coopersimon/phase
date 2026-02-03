pub mod ram;
mod bios;
mod control;
mod dma;

use mips::mem::{Data, Mem32};
use ram::RAM;
use bios::BIOS;
use control::MemControl;
use dma::DMA;
pub use dma::DMADevice;

use crate::PlayStationConfig;
use crate::gpu::GPU;
use crate::io::{BusIO, InputMessage};
use crate::spu::SPU;
use crate::utils::interface::MemInterface;
use crate::interrupt::InterruptControl;
use crate::timer::Timers;
use crate::cdrom::CDROM;
use crate::expansion::{ExpansionPort1, ExpansionPort2};
use crate::peripheral::PeripheralPort;
use crate::mdec::{MDEC, MDECStatus};

pub struct MemBus {
    control: MemControl,
    main_ram: RAM,
    scratchpad: RAM,
    bios: BIOS,
    interrupts: InterruptControl,

    timers: Timers,
    dma:    DMA,
    cdrom:  CDROM,
    spu:    SPU,
    gpu:    GPU,
    peripheral: PeripheralPort,
    mdec:   MDEC,

    expansion_port_1: ExpansionPort1,
    expansion_port_2: ExpansionPort2,

    io: BusIO,
}

impl MemBus {
    pub fn new(config: &PlayStationConfig, io: BusIO) -> Self {
        let bios = BIOS::new(Some(&config.bios_path)).expect("error loading BIOS"); // TODO: handle error.
        Self {
            control: MemControl::new(),
            main_ram: RAM::new(2048 * 1024), // 2MB
            scratchpad: RAM::new(1024),
            bios,
            interrupts: InterruptControl::new(),

            timers: Timers::new(),
            dma:    DMA::new(),
            cdrom:  CDROM::new(),
            spu:    SPU::new(),
            gpu:    GPU::new(io.clone_frame_arc()),
            peripheral: PeripheralPort::new(),
            mdec:   MDEC::new(),

            expansion_port_1: ExpansionPort1::new(),
            expansion_port_2: ExpansionPort2::new(),

            io: io,
        }
    }

    /// Clock internally, and set interrupt bits.
    /// 
    /// Returns false if a frame is about to begin,
    /// and therefore we are syncing with the real world.
    fn do_clock(&mut self, cycles: usize) -> bool {
        let gpu_stat = self.gpu.clock(cycles);
        if self.gpu.dma_recv_ready() {
            self.dma.gpu_recv_req();
        }
        if self.gpu.dma_send_ready() {
            self.dma.gpu_send_req();
        }

        let mdec_status = self.mdec.clock(cycles);
        match mdec_status {
            MDECStatus::DataInReady => self.dma.mdec_recv_req(),
            MDECStatus::DataOutReady => self.dma.mdec_send_req(),
            _ => {}
        }

        let dma_irq = self.dma.check_irq();

        let timer_irq = self.timers.clock(cycles, &gpu_stat);

        let cd_irq = self.cdrom.clock(cycles);

        let spu_irq = self.spu.clock(cycles);

        let peripheral_irq = self.peripheral.clock(cycles);

        self.interrupts.trigger_irq(
            gpu_stat.irq |
            dma_irq |
            timer_irq |
            cd_irq |
            spu_irq |
            peripheral_irq
        );
        gpu_stat.new_frame
    }

    /// Do DMA transfers, if any are ready.
    /// 
    /// This will take control from the CPU and clock until the
    /// DMA transfers are complete.
    fn do_dma(&mut self) {
        while let Some(transfer) = self.dma.get_transfer() {
            let ram_addr = transfer.addr & 0x1F_FFFF;
            let cycles = if transfer.from_ram {
                let data = self.main_ram.read_word(ram_addr);
                if transfer.list_data {
                    self.dma.write_list_data(transfer.device, data);
                    1
                } else {
                    self.mut_dma_device(transfer.device).dma_write_word(data)
                }
            } else {
                let Data { data, cycles } = self.mut_dma_device(transfer.device).dma_read_word();
                self.main_ram.write_word(ram_addr, data);
                cycles
            };
            if self.do_clock(cycles) {
                self.begin_frame();
            }
        }
    }

    /// Upon frame completion, send a frame to the outside world.
    fn begin_frame(&mut self) {
        // Sync up with the GPU.
        self.gpu.get_frame();
        let input = self.io.send_frame();
        for message in input {
            use InputMessage::*;
            match message {
                CDInserted { path } => {
                    self.cdrom.insert_disc(Some(&path)).expect("error inserting CD");
                    println!("CD inserted: {:?}", path.to_str());
                },
                CDRemoved => {
                    self.cdrom.insert_disc(None).expect("error removing CD");
                    println!("CD removed");
                },
                ControllerConnected { port, state } => {
                    println!("Connected controller to port {:?}", port);
                    self.peripheral.set_controller_state(port, state);
                },
                ControllerDisconnected { port } => {
                    println!("Disconnected controller at port {:?}", port);
                    self.peripheral.clear_controller_state(port);
                },
                ControllerInput { port, state } => {
                    self.peripheral.set_controller_state(port, state);
                },
            }
        }
    }
}

impl Mem32 for MemBus {
    type Addr = u32;
    const LITTLE_ENDIAN: bool = true;

    fn clock(&mut self, cycles: usize) -> u8 {
        if self.do_clock(cycles) {
            self.begin_frame();
        }

        self.do_dma();

        if self.interrupts.check_irq() {
            0x04 // Interrupt bit 2 is used for all external hardware IRQs.
        } else {
            0x00
        }
    }

    fn read_byte(&mut self, addr: Self::Addr) -> Data<u8> {
        let (data, cycles) = match addr {
            0x0000_0000..=0x007F_FFFF => (self.main_ram.read_byte(addr & 0x1F_FFFF), 3),
            0x1F00_0000..=0x1F7F_FFFF => (self.expansion_port_1.read_byte(addr), 1),
            0x1F80_0000..=0x1F80_03FF => (self.scratchpad.read_byte(addr & 0x3FF), 1),
            0x1F80_1000..=0x1F80_1FFF => (self.mut_io_device(addr).read_byte(addr), 1),
            0x1F80_2000..=0x1F80_2FFF => (self.expansion_port_2.read_byte(addr), 1),
            0x1FC0_0000..=0x1FC7_FFFF => (self.bios.read_byte(addr & 0x7_FFFF), 1),
            _ => panic!("read invalid address {:X}", addr),
        };
        Data { data, cycles }
    }

    fn write_byte(&mut self, addr: Self::Addr, data: u8) -> usize {
        match addr {
            0x0000_0000..=0x007F_FFFF => {self.main_ram.write_byte(addr & 0x1F_FFFF, data); 3},
            0x1F00_0000..=0x1F7F_FFFF => {self.expansion_port_1.write_byte(addr, data); 1},
            0x1F80_0000..=0x1F80_03FF => {self.scratchpad.write_byte(addr & 0x3FF, data); 1},
            0x1F80_1000..=0x1F80_1FFF => {self.mut_io_device(addr).write_byte(addr, data); 1},
            0x1F80_2000..=0x1F80_2FFF => {self.expansion_port_2.write_byte(addr, data); 1},
            0x1FC0_0000..=0x1FC7_FFFF => 1, // BIOS
            _ => panic!("write invalid address {:X}", addr),
        }
    }

    fn read_halfword(&mut self, addr: Self::Addr) -> Data<u16> {
        let (data, cycles) = match addr {
            0x0000_0000..=0x007F_FFFF => (self.main_ram.read_halfword(addr & 0x1F_FFFF), 3),
            0x1F00_0000..=0x1F7F_FFFF => (self.expansion_port_1.read_halfword(addr), 1),
            0x1F80_0000..=0x1F80_03FF => (self.scratchpad.read_halfword(addr & 0x3FF), 1),
            0x1F80_1000..=0x1F80_1FFF => (self.mut_io_device(addr).read_halfword(addr), 1),
            0x1F80_2000..=0x1F80_2FFF => (self.expansion_port_2.read_halfword(addr), 1),
            0x1FC0_0000..=0x1FC7_FFFF => (self.bios.read_halfword(addr & 0x7_FFFF), 1),
            _ => panic!("read invalid address {:X}", addr),
        };
        Data { data, cycles }
    }

    fn write_halfword(&mut self, addr: Self::Addr, data: u16) -> usize {
        match addr {
            0x0000_0000..=0x007F_FFFF => {self.main_ram.write_halfword(addr & 0x1F_FFFF, data); 3},
            0x1F00_0000..=0x1F7F_FFFF => {self.expansion_port_1.write_halfword(addr, data); 1},
            0x1F80_0000..=0x1F80_03FF => {self.scratchpad.write_halfword(addr & 0x3FF, data); 1},
            0x1F80_1000..=0x1F80_1FFF => {self.mut_io_device(addr).write_halfword(addr, data); 1},
            0x1F80_2000..=0x1F80_2FFF => {self.expansion_port_2.write_halfword(addr, data); 1},
            0x1FC0_0000..=0x1FC7_FFFF => 1, // BIOS
            _ => panic!("write invalid address {:X}", addr),
        }
    }

    fn read_word(&mut self, addr: Self::Addr) -> Data<u32> {
        let (data, cycles) = match addr {
            0x0000_0000..=0x007F_FFFF => (self.main_ram.read_word(addr & 0x1F_FFFF), 3),
            0x1F00_0000..=0x1F7F_FFFF => (self.expansion_port_1.read_word(addr), 1),
            0x1F80_0000..=0x1F80_03FF => (self.scratchpad.read_word(addr & 0x3FF), 1),
            0x1F80_1000..=0x1F80_1FFF => (self.mut_io_device(addr).read_word(addr), 1),
            0x1F80_2000..=0x1F80_2FFF => (self.expansion_port_2.read_word(addr), 1),
            0x1FC0_0000..=0x1FC7_FFFF => (self.bios.read_word(addr & 0x7_FFFF), 1),
            _ => panic!("read invalid address {:X}", addr),
        };
        Data { data, cycles }
    }

    fn write_word(&mut self, addr: Self::Addr, data: u32) -> usize {
        match addr {
            0x0000_0000..=0x007F_FFFF => {self.main_ram.write_word(addr & 0x1F_FFFF, data); 3},
            0x1F00_0000..=0x1F7F_FFFF => {self.expansion_port_1.write_word(addr, data); 1},
            0x1F80_0000..=0x1F80_03FF => {self.scratchpad.write_word(addr & 0x3FF, data); 1},
            0x1F80_1000..=0x1F80_1FFF => {self.mut_io_device(addr).write_word(addr, data); 1},
            0x1F80_2000..=0x1F80_2FFF => {self.expansion_port_2.write_word(addr, data); 1},
            0x1FC0_0000..=0x1FC7_FFFF => 1, // BIOS
            _ => panic!("write invalid address {:X}", addr),
        }
    }
}

impl MemBus {
    /// Mutably reference an I/O device.
    fn mut_io_device<'a>(&'a mut self, addr: u32) -> &'a mut dyn MemInterface {
        if addr != 0x1F801814 {
            //println!("access I/O {:X}", addr);
        }
        match addr {
            0x1F80_1000..=0x1F80_1023 => &mut self.control,
            0x1F80_1040..=0x1F80_104F => &mut self.peripheral,
            //0x1F80_1050..=0x1F80_105F => None, // Serial
            0x1F80_1060..=0x1F80_1063 => &mut self.control,
            0x1F80_1070..=0x1F80_1077 => &mut self.interrupts,
            0x1F80_1080..=0x1F80_10FF => &mut self.dma,
            0x1F80_1100..=0x1F80_1129 => &mut self.timers,
            0x1F80_1800..=0x1F80_1807 => &mut self.cdrom,
            0x1F80_1810..=0x1F80_1817 => &mut self.gpu,
            0x1F80_1820..=0x1F80_1827 => &mut self.mdec,
            0x1F80_1C00..=0x1F80_1FFF => &mut self.spu,
            _ => panic!("no such I/O device at {:X}", addr),
        }
    }

    /// Mutably reference a DMA device.
    fn mut_dma_device<'a>(&'a mut self, device: usize) -> &'a mut dyn DMADevice {
        match device {
            0 => &mut self.mdec,
            1 => &mut self.mdec,
            2 => &mut self.gpu,
            3 => &mut self.cdrom,
            4 => &mut self.spu,
            5 => unimplemented!("expansion port DMA"),
            6 => self.dma.mut_table_gen(),
            _ => unreachable!()
        }
    }
}