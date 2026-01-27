mod videostate;
mod renderer;

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering}
};

use mips::mem::Data;

use crossbeam_channel::{
    bounded, Sender, Receiver
};

use crate::{
    mem::DMADevice,
    utils::{bits::*, interface::MemInterface}
};

use renderer::*;
use videostate::StateMachine;
pub use videostate::GPUClockRes;

/// Graphics processing unit
pub struct GPU {
    state: StateMachine,

    status: Arc<AtomicU32>,
    read_reg: u32,

    renderer_tx: Sender<RendererCmd>,
}

impl GPU {
    pub fn new() -> Self {
        let (tx, rx) = bounded(32); // TODO: should this even be bounded..?
        let status = Arc::new(AtomicU32::new(0));
        let thread_status = status.clone();
        // Start render thread.
        std::thread::spawn(|| {
            let mut renderer = Renderer::new(rx, thread_status);
            renderer.run();
        });
        Self {
            state: StateMachine::new(),

            status: status,
            read_reg: 0,

            renderer_tx: tx,
        }
    }

    pub fn clock(&mut self, cycles: usize) -> GPUClockRes {
        let res = self.state.clock(cycles);

        /*while let Some(data) = self.gp0_fifo.pop_front() {
            self.exec_gp0_command(data);
        }
        self.status.insert(GPUStatus::CommandReady | GPUStatus::DMARecvReady);*/

        res
    }

    /// Check if the GPU is ready to transfer via DMA,
    /// using a command list.
    pub fn dma_command_ready(&self) -> bool {
        let mask = (GPUStatus::DMAMode | GPUStatus::DMARecvReady).bits();
        (self.status.load(Ordering::Acquire) & mask) == GPUStatus::CommandTransferReady.bits()
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
        let dma_mode = (self.status.load(Ordering::Acquire) & GPUStatus::DMAMode.bits()) >> 29;
        if dma_mode == 2 {
            self.send_gp0_command(data);
        } else {
            // ???
        }
        1
    }
}

// Internal
impl GPU {
    fn send_gp0_command(&mut self, data: u32) {
        let _ = self.renderer_tx.send(RendererCmd::GP0(data));
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
            0x05 => self.display_vram_offset(data),
            0x06 => self.display_range_x(data),
            0x07 => self.display_range_y(data),
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
        let status = self.status.load(Ordering::Acquire);
        if status & GPUStatus::Interlace.bits() != 0 {
            if self.state.get_interlace_bit() {
                status | GPUStatus::InterlaceOdd.bits()
            } else {
                status
            }
        } else {
            status
        }
    }
}

// GP1 commands
impl GPU {
    fn reset(&mut self) {
        self.reset_command_buf();
        self.acknowledge_irq();
        self.display_enable(1);
        self.data_request(0);
        self.display_vram_offset(0);
        let reset_x = 0x200 | ((0x200 + 256 * 10) << 12);
        self.display_range_x(reset_x);
        let reset_y = 0x010 | ((0x010 + 240) << 12);
        self.display_range_y(reset_y);
        self.display_mode(0);
    }

    fn reset_command_buf(&mut self) {
        // TODO: flush tx.
    }

    fn acknowledge_irq(&mut self) {
        let _ = self.renderer_tx.send(RendererCmd::AcknowledgeIRQ);
    }

    fn display_enable(&mut self, param: u32) {
        let enable = !test_bit!(param, 0);
        let _ = self.renderer_tx.send(RendererCmd::DisplayEnable(enable));
    }

    fn data_request(&mut self, param: u32) {
        let dma_mode = param & 0x3;
        let _ = self.renderer_tx.send(RendererCmd::DataRequest(GPUStatus::from_bits_truncate(dma_mode << 29)));
    }

    fn display_vram_offset(&mut self, param: u32) {
        let _ = self.renderer_tx.send(RendererCmd::DisplayVRAMOffset(param));
    }

    fn display_range_x(&mut self, param: u32) {
        // TODO: send to video state.
        let _x_left = param & 0xFFF;
        let _x_right = (param >> 12) & 0xFFF;
    }

    fn display_range_y(&mut self, param: u32) {
        // TODO: send to video state
        let _y_top = param & 0x3FF;
        let _y_bottom = (param >> 10) & 0x3FF;
    }

    fn display_mode(&mut self, param: u32) {
        let mut display_status = GPUStatus::empty();
        display_status.insert(GPUStatus::from_bits_truncate((param & 0x3F) << 17));
        display_status.insert(GPUStatus::from_bits_truncate((param & 0x40) << 10)); // HRes low bit
        display_status.insert(GPUStatus::from_bits_truncate((param & 0x80) << 7)); // Reverseflag
        let h_res = match (display_status & GPUStatus::XResolution).bits() >> 16 {
            0b000 => 256,
            0b010 => 320,
            0b100 => 512,
            0b110 => 640,
            _ => 368,
        };
        if display_status.contains(GPUStatus::PALMode) {
            panic!("PAL mode unsupported!");
        } else {
            self.state.set_h_res_ntsc(h_res);
        }
        self.state.set_interlace(display_status.contains(GPUStatus::Interlace));
        let _ = self.renderer_tx.send(RendererCmd::DisplayMode(display_status));
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
        let disable = test_bit!(param, 0);
        let _ = self.renderer_tx.send(RendererCmd::TexDisable(disable));
    }
}
