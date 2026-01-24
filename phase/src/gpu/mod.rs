use std::collections::VecDeque;

use mips::mem::Data;

use crate::{
    interrupt::Interrupt,
    mem::{DMADevice, ram::RAM},
    utils::{bits::*, interface::MemInterface}
};


const VRAM_SIZE: usize = 1024 * 1024;

/// Returned when clocking the GPU.
/// 
/// Indicates state of interrupt / blanking.
pub struct GPUClockRes {
    pub irq: Interrupt,
    pub v_blank: bool,
    pub h_blank: bool,
}

/// Graphics processing unit
pub struct GPU {
    vram: RAM,
    status: GPUStatus,

    gp0_fifo: VecDeque<u32>,
    read_reg: u32,
}

impl GPU {
    pub fn new() -> Self {
        Self {
            vram: RAM::new(VRAM_SIZE),
            status: GPUStatus::CommandReady | GPUStatus::DMARecvReady,

            gp0_fifo: VecDeque::new(),
            read_reg: 0,
        }
    }

    pub fn clock(&mut self, cycles: usize) -> GPUClockRes {
        // TODO: clock.

        while let Some(data) = self.gp0_fifo.pop_front() {
            self.exec_gp0_command(data);
        }
        self.status.insert(GPUStatus::CommandReady | GPUStatus::DMARecvReady);

        GPUClockRes {
            irq: Interrupt::empty(),
            v_blank: false,
            h_blank: false
        }
    }
}

impl MemInterface for GPU {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x1F80_1810 => self.recv_response(),
            0x1F80_1814 => self.read_status(),
            _ => panic!("invalid GPU address {:X}", addr),
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x1F80_1810 => self.send_gp0_command(data),
            0x1F80_1814 => self.send_gp1_command(data),
            _ => panic!("invalid GPU address {:X}", addr),
        }
    }
}

impl DMADevice for GPU {
    fn dma_read_word(&mut self) -> Data<u32> {
        // TODO.
        Data { data: 0, cycles: 1 }
    }

    fn dma_write_word(&mut self, data: u32) -> usize {
        // TODO.
        1
    }
}

// Internal
impl GPU {
    fn send_gp0_command(&mut self, data: u32) {
        self.gp0_fifo.push_back(data);
    }

    fn send_gp1_command(&mut self, data: u32) {
        println!("GP1 command: {:X}", data);
        let command = (data >> 24) as u8;
        match command {
            0x00 => self.reset(),
            0x01 => self.reset_command_buf(),
            0x02 => self.acknowledge_irq(),
            0x03 => self.display_enable(data),
            0x04 => self.data_request(data),
            0x05 => self.display_origin(data),
            0x06 => self.display_width(data),
            0x07 => self.display_height(data),
            0x08 => self.display_mode(data),
            0x09 => self.tex_disable(data),
            0x10 => self.get_gpu_info(data),
            _ => {}, // Invalid command.
        }
    }

    fn recv_response(&mut self) -> u32 {
        self.read_reg
    }

    fn read_status(&self) -> u32 {
        let stat = self.status.bits();
        println!("Read stat: {:X}", stat);
        stat
    }

    fn exec_gp0_command(&mut self, data: u32) {
        println!("GP0 command: {:X}", data);
        let command = (data >> 24) as u8;
        match command {
            0xE1 => self.draw_mode_setting(data),
            _ => {}, // Invalid command.
        }
    }
}

// GP0 commands
impl GPU {

    // Rendering attribute commands

    fn draw_mode_setting(&mut self, param: u32) {
        let low_bits = param & 0x7FF;
        self.status.remove(GPUStatus::DrawModeFlags);
        self.status.insert(GPUStatus::from_bits_truncate(low_bits));
        self.status.set(GPUStatus::TexDisable, test_bit!(param, 11));

        // TODO: x-flip and y-flip
    }
}

// GP1 commands
impl GPU {
    fn reset(&mut self) {
        self.reset_command_buf();
        self.acknowledge_irq();
        self.display_enable(1);
        self.data_request(0);
        self.display_origin(0);
        let reset_x = 0x200 | ((0x200 + 256 * 10) << 12);
        self.display_width(reset_x);
        let reset_y = 0x010 | ((0x010 + 240) << 12);
        self.display_height(reset_y);
        self.display_mode(0);
    }

    fn reset_command_buf(&mut self) {

    }

    fn acknowledge_irq(&mut self) {
        self.status.remove(GPUStatus::IRQ);
    }

    fn display_enable(&mut self, param: u32) {
        self.status.set(GPUStatus::DisplayEnable, !test_bit!(param, 0));
    }

    fn data_request(&mut self, param: u32) {
        let dma_mode = param & 0x3;
        self.status.remove(GPUStatus::DMAMode);
        self.status.insert(GPUStatus::from_bits_truncate(dma_mode << 29));
    }

    fn display_origin(&mut self, param: u32) {

    }

    fn display_width(&mut self, param: u32) {
        
    }

    fn display_height(&mut self, param: u32) {
        
    }

    fn display_mode(&mut self, param: u32) {
        self.status.remove(GPUStatus::DispModeFlags);
        self.status.insert(GPUStatus::from_bits_truncate((param & 0x3F) << 17));
        self.status.insert(GPUStatus::from_bits_truncate((param & 0x40) << 10)); // HRes low bit
        self.status.insert(GPUStatus::from_bits_truncate((param & 0x80) << 7)); // Reverseflag
    }

    fn get_gpu_info(&mut self, param: u32) {
        match param & 0x7 {
            2 => self.read_reg = 0, // Tex window
            3 => self.read_reg = 0, // Draw area top-left
            4 => self.read_reg = 0, // Draw area bottom-right
            5 => self.read_reg = 0, // Draw offset
            _ => {}, // NOP.
        }
    }

    fn tex_disable(&mut self, param: u32) {
        self.status.set(GPUStatus::TexDisable, test_bit!(param, 0));
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct GPUStatus: u32 {
        const InterlaceOdd  = bit!(31);
        const DMAMode       = bits![29, 30];
        const DMARecvReady  = bit!(28);
        const VRAMSendReady = bit!(27);
        const CommandReady  = bit!(26);
        const DMARequest    = bit!(25);
        const IRQ           = bit!(24);
        const DisplayEnable = bit!(23);
        const Interlace     = bit!(22);
        const ColorDepth    = bit!(21);
        const PALMode       = bit!(20);
        const YResolution   = bit!(19);
        const XResolution   = bits![16, 17, 18];
        const TexDisable    = bit!(15);
        const Reverse       = bit!(14);
        const InterlaceField = bit!(13);
        const MaskDrawing   = bit!(12);
        const SetDrawMask   = bit!(11);
        const DrawDisplay   = bit!(10);
        const Dither        = bit!(9);
        const TexPageCol    = bits![7, 8];
        const SemiTrans     = bits![5, 6];
        const TexPageYBase  = bit!(4);
        const TexPageXBase  = bits![0, 1, 2, 3];

        const DispModeFlags = bits![14, 16, 17, 18, 19, 20, 21, 22];
        const DrawModeFlags = bits![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15];
    }
}