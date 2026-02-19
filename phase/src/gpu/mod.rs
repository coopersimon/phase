mod videostate;
mod renderer;

use std::sync::{
    Arc, Mutex
};

use mips::mem::Data;

use crossbeam_channel::{
    Receiver, Sender, unbounded
};

use crate::{
    Frame,
    mem::DMADevice,
    utils::{bits::*, interface::MemInterface}
};

use renderer::*;
use videostate::StateMachine;
pub use videostate::GPUClockRes;

/// Graphics processing unit
pub struct GPU {
    state: StateMachine,

    status: GPUStatus,
    read_reg: u32,
    // Below are used only for returning via GPU Info command
    tex_window: u32,
    draw_area_top_left: u32,
    draw_area_bottom_right: u32,
    draw_offset: u32,

    renderer_tx: Sender<RendererCmd>,
    frame_rx: Receiver<()>,
    vram_rx: Receiver<u32>,

    // GP0 stuff
    pending_command: u8,
    pending_command_words: usize,
    command_data: Vec<u32>,
    poly_line_buf: Vec<u32>,

    // DMA
    data_words: usize,
    block_count: usize,
}

impl GPU {
    pub fn new(frame: Arc<Mutex<Frame>>) -> Self {
        let (renderer_tx, renderer_rx) = unbounded(); // TODO: technically FIFO should be bounded...
        let init_status = GPUStatus::CommandReady | GPUStatus::DMARecvReady;
        let (frame_tx, frame_rx) = unbounded();
        let (vram_tx, vram_rx) = unbounded();
        // Start render thread.
        std::thread::spawn(|| {
            let mut renderer = Renderer::new(renderer_rx, frame_tx, vram_tx, frame);
            renderer.run();
        });
        Self {
            state: StateMachine::new(),

            status: init_status,
            read_reg: 0,
            tex_window: 0,
            draw_area_top_left: 0,
            draw_area_bottom_right: 0,
            draw_offset: 0,

            renderer_tx,
            frame_rx,
            vram_rx,

            pending_command: 0,
            pending_command_words: 0,
            command_data: Vec::new(),
            poly_line_buf: Vec::new(),

            data_words: 0,
            block_count: 0,
        }
    }

    pub fn clock(&mut self, cycles: usize) -> GPUClockRes {
        let res = self.state.clock(cycles);
        res
    }

    /// Check if DMA is ready.
    pub fn dma_ready(&mut self) -> bool {
        if self.status.contains(GPUStatus::DMARequest) {
            true
        } else {
            false
        }
    }

    pub fn dma_cmd_ready(&self) -> bool {
        self.status.contains(GPUStatus::CommandReady)
    }

    /// This extracts a frame from the renderer. It needs to communicate across a thread.
    /// It should be called at the _start_ of each frame.
    pub fn get_frame(&mut self) {
        let interlace_state = self.state.get_interlace_state();
        if self.renderer_tx.send(RendererCmd::GetFrame(interlace_state)).is_ok() {
            let _ = self.frame_rx.recv();
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
            0x1F80_1810 => self.send_gp0(data),
            0x1F80_1814 => self.send_gp1_command(data),
            _ => panic!("invalid GPU address {:X}", addr),
        }
    }
}

impl DMADevice for GPU {
    fn dma_read_word(&mut self) -> Data<u32> {
        let data = self.recv_response();
        Data { data: data, cycles: 1 }
    }

    fn dma_write_word(&mut self, data: u32) -> usize {
        self.send_gp0(data);
        1
    }
}

// Internal
impl GPU {
    fn send_gp0(&mut self, data: u32) {
        if self.data_words > 0 {
            let _ = self.renderer_tx.send(RendererCmd::GP0Data(data));
            self.status.remove(GPUStatus::DMARequest);
            self.data_words -= 1;
            self.block_count -= 1;
            if self.data_words == 0 {
                self.status.insert(GPUStatus::CommandReady | GPUStatus::DMARequest); // ?
            } else {
                if self.block_count == 0 {
                    self.status.insert(GPUStatus::DMARequest);
                    self.block_count = 0x10;
                }
            }
            self.update_dma_status();
        } else {
            self.send_gp0_command(data);
        }
    }

    fn recv_response(&mut self) -> u32 {
        if self.data_words > 0 {
            self.read_reg = self.vram_rx.recv().unwrap_or_default();
            self.status.remove(GPUStatus::DMARequest);
            self.data_words -= 1;
            self.block_count -= 1;
            if self.data_words == 0 {
                self.status.remove(GPUStatus::VRAMSendReady);
                self.status.insert(GPUStatus::CommandReady | GPUStatus::DMARequest); // ?
            } else {
                if self.block_count == 0 {
                    self.status.insert(GPUStatus::DMARequest);
                    self.block_count = 0x10;
                }
            }
            self.update_dma_status();
        }
        self.read_reg
    }

    fn send_gp0_command(&mut self, data: u32) {
        //println!("GP0 send: {:X}", data);
        if self.pending_command_words == 0 {
            //self.status.remove(GPUStatus::CommandReady);
            self.pending_command = (data >> 24) as u8;
            self.pending_command_words = match self.pending_command {
                0x01 => 1,
                0x02 => 3,

                0x1F => 1,

                0x20..=0x23 => 4,
                0x24..=0x27 => 7,
                0x28..=0x2B => 5,
                0x2C..=0x2F => 9,
                0x30..=0x33 => 6,
                0x34..=0x37 => 9,
                0x38..=0x3B => 8,
                0x3C..=0x3F => 12,

                0x40..=0x4F => 3,
                0x50..=0x5F => 4,

                0x60..=0x63 => 3,
                0x64..=0x67 => 4,

                0x68..=0x6B => 2,
                0x6C..=0x6F => 3,
                0x70..=0x73 => 2,
                0x74..=0x77 => 3,
                0x78..=0x7B => 2,
                0x7C..=0x7F => 3,

                0x80 => 4,
                0xA0 => 3,
                0xC0 => 3,

                0xE1..=0xE6 => 1,
                _ => 0
            };
        }
        if self.pending_command_words > 0 {
            self.command_data.push(data);
            self.pending_command_words -= 1;
        }
        // Ready to send.
        if self.pending_command_words == 0 {
            //self.status.insert(GPUStatus::CommandReady);
            use GP0Command::*;
            let gp0_command = match self.pending_command {
                0x00 => None,
                0x01 => Some(ClearCache),
                0x02 => Some(FillRectangle(std::array::from_fn(|n| self.command_data[n]))),

                0x1F => {self.irq(); None},

                0x20 | 0x21 => Some(DrawTri{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x22 | 0x23 => Some(DrawTri{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),
                0x24 => Some(DrawTexBlendTri{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x25 => Some(DrawTexTri{params: std::array::from_fn(|n| self.command_data[n+1]), transparent: false}),
                0x26 => Some(DrawTexBlendTri{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),
                0x27 => Some(DrawTexTri{params: std::array::from_fn(|n| self.command_data[n+1]), transparent: true}),
                0x28 | 0x29 => Some(DrawQuad{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x2A | 0x2B => Some(DrawQuad{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),
                0x2C => Some(DrawTexBlendQuad{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x2D => Some(DrawTexQuad{params: std::array::from_fn(|n| self.command_data[n+1]), transparent: false}),
                0x2E => Some(DrawTexBlendQuad{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),
                0x2F => Some(DrawTexQuad{params: std::array::from_fn(|n| self.command_data[n+1]), transparent: true}),

                0x30 | 0x31 => Some(DrawShadedTri{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x32 | 0x33 => Some(DrawShadedTri{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),
                0x34 | 0x35 => Some(DrawTexShadedTri{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x36 | 0x37 => Some(DrawTexShadedTri{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),

                0x38 | 0x39 => Some(DrawShadedQuad{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x3A | 0x3B => Some(DrawShadedQuad{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),
                0x3C | 0x3D => Some(DrawTexShadedQuad{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x3E | 0x3F => Some(DrawTexShadedQuad{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),

                0x40 | 0x41 => Some(DrawLine{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x42 => Some(DrawLine{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),
                0x48 | 0x4C => self.poly_line(false),
                0x4A => self.poly_line(true),
                0x50 | 0x51 => Some(DrawShadedLine{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x52 | 0x53 => Some(DrawShadedLine{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),
                0x58 => self.shaded_poly_line(false),
                0x5A | 0x5E => self.shaded_poly_line(true),

                0x60 | 0x61 => Some(DrawRect{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x62 | 0x63 => Some(DrawRect{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),
                0x64 => Some(DrawTexBlendedRect{params: std::array::from_fn(|n| self.command_data[n]), transparent: false}),
                0x65 => Some(DrawTexRect{params: std::array::from_fn(|n| self.command_data[n+1]), transparent: false}),
                0x66 => Some(DrawTexBlendedRect{params: std::array::from_fn(|n| self.command_data[n]), transparent: true}),
                0x67 => Some(DrawTexRect{params: std::array::from_fn(|n| self.command_data[n+1]), transparent: true}),
                0x68 => Some(DrawFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 1, transparent: false}),
                0x6A => Some(DrawFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 1, transparent: true}),
                0x6C => Some(DrawTexBlendedFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 1, transparent: false}),
                0x6D => Some(DrawTexFixedRect{params: std::array::from_fn(|n| self.command_data[n+1]), size: 1, transparent: false}),
                0x6E => Some(DrawTexBlendedFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 1, transparent: true}),
                0x6F => Some(DrawTexFixedRect{params: std::array::from_fn(|n| self.command_data[n+1]), size: 1, transparent: true}),
                0x70 => Some(DrawFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 8, transparent: false}),
                0x72 => Some(DrawFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 8, transparent: true}),
                0x74 => Some(DrawTexBlendedFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 8, transparent: false}),
                0x75 => Some(DrawTexFixedRect{params: std::array::from_fn(|n| self.command_data[n+1]), size: 8, transparent: false}),
                0x76 => Some(DrawTexBlendedFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 8, transparent: true}),
                0x77 => Some(DrawTexFixedRect{params: std::array::from_fn(|n| self.command_data[n+1]), size: 8, transparent: true}),
                0x78 => Some(DrawFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 16, transparent: false}),
                0x7A => Some(DrawFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 16, transparent: true}),
                0x7C => Some(DrawTexBlendedFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 16, transparent: false}),
                0x7D => Some(DrawTexFixedRect{params: std::array::from_fn(|n| self.command_data[n+1]), size: 16, transparent: false}),
                0x7E => Some(DrawTexBlendedFixedRect{params: std::array::from_fn(|n| self.command_data[n]), size: 16, transparent: true}),
                0x7F => Some(DrawTexFixedRect{params: std::array::from_fn(|n| self.command_data[n+1]), size: 16, transparent: true}),

                0x80 => Some(BlitVRAMtoVRAM{params: std::array::from_fn(|n| self.command_data[n+1])}),
                0xA0 => Some(self.blit_cpu_to_vram()),
                0xC0 => Some(self.blit_vram_to_cpu()),

                0xE1 => Some(self.draw_mode_setting(data)),
                0xE2 => Some(self.texture_window_setting(data)),
                0xE3 => Some(self.set_draw_area_top_left(data)),
                0xE4 => Some(self.set_draw_area_bottom_right(data)),
                0xE5 => Some(self.set_draw_offset(data)),
                0xE6 => Some(self.mask_bit_setting(data)),

                _ => panic!("unknown GP0 command: {:X}", self.pending_command),
            };
            self.command_data.clear();
            if let Some(command) = gp0_command {
                let _ = self.renderer_tx.send(RendererCmd::GP0(command));
            }
        }
    }

    fn send_gp1_command(&mut self, data: u32) {
        //println!("GP1 command: {:X}", data);
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
            _ => panic!("invalid GP1 command {:X}", command),
        }
    }

    fn read_status(&self) -> u32 {
        let mut status = self.status;
        // TODO: deal with this elsewhere...
        if status.contains(GPUStatus::Interlace) {
            if self.state.get_interlace_bit() {
                status |= GPUStatus::InterlaceOdd;
            }
        }
        status.bits()
    }

    fn update_dma_status(&mut self) {
        let dma_mode = self.status.intersection(GPUStatus::DMAMode).bits() >> 29;
        match dma_mode {
            0b00 => self.status.remove(GPUStatus::DMARequest),
            0b01 => self.status.insert(GPUStatus::DMARequest), // Just assume FIFO is never empty...
            0b10 => self.status.set(GPUStatus::DMARequest, self.status.contains(GPUStatus::DMARecvReady)),
            0b11 => self.status.set(GPUStatus::DMARequest, self.status.contains(GPUStatus::VRAMSendReady)),
            _ => unreachable!("invalid DMA mode")
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
        self.status.remove(GPUStatus::IRQ);
    }

    fn display_enable(&mut self, param: u32) {
        let enable = !test_bit!(param, 0);
        self.status.set(GPUStatus::DisplayEnable, enable);
        let _ = self.renderer_tx.send(RendererCmd::DisplayEnable(enable));
    }

    fn data_request(&mut self, param: u32) {
        self.status.remove(GPUStatus::DMAMode);
        let dma_mode = param & 0x3;
        self.status.insert(GPUStatus::from_bits_retain(dma_mode << 29));
        self.update_dma_status();
    }

    fn display_vram_offset(&mut self, param: u32) {
        let _ = self.renderer_tx.send(RendererCmd::DisplayVRAMOffset(param));
    }

    fn display_range_x(&mut self, param: u32) {
        let _ = self.renderer_tx.send(RendererCmd::DisplayXRange(param));
    }

    fn display_range_y(&mut self, param: u32) {
        let _ = self.renderer_tx.send(RendererCmd::DisplayYRange(param));
    }

    fn display_mode(&mut self, param: u32) {
        self.status.remove(GPUStatus::DispModeFlags);
        self.status.insert(GPUStatus::from_bits_truncate((param & 0x3F) << 17));
        self.status.insert(GPUStatus::from_bits_truncate((param & 0x40) << 10)); // HRes low bit
        self.status.insert(GPUStatus::from_bits_truncate((param & 0x80) << 7)); // Reverseflag
        let h_res = self.status.h_res();
        let v_res = self.status.v_res();
        if self.status.contains(GPUStatus::PALMode) {
            panic!("PAL mode unsupported!");
        } else {
            self.state.set_h_res_ntsc(h_res);
        }
        let interlace = self.status.contains(GPUStatus::Interlace);
        self.state.set_interlace(interlace);
        let rgb24 = self.status.contains(GPUStatus::ColorDepth);
        let _ = self.renderer_tx.send(RendererCmd::DisplayMode{h_res, v_res, interlace, rgb24});
    }

    fn get_gpu_info(&mut self, param: u32) {
        match param & 0x7 {
            2 => self.read_reg = self.tex_window,
            3 => self.read_reg = self.draw_area_top_left,
            4 => self.read_reg = self.draw_area_bottom_right,
            5 => self.read_reg = self.draw_offset,
            7 => self.read_reg = 0, // GPU type.
            _ => panic!("Get GPU INFO {:X}", param & 0x7), // NOP.
        }
    }

    fn tex_disable(&mut self, param: u32) {
        let disable = test_bit!(param, 0);
        self.status.set(GPUStatus::TexDisable, disable);
        let _ = self.renderer_tx.send(RendererCmd::TexDisable(disable));
    }
}

const POLY_LINE_TERM: u32 = 0x5555_5555;

// GP0 commands
// Mostly as these are drawing commands, they are just dispatched to the
// render thread. However some modify status.
impl GPU {
    fn irq(&mut self) {
        self.status.insert(GPUStatus::IRQ);
    }

    fn draw_mode_setting(&mut self, param: u32) -> GP0Command {
        let low_bits = param & 0x7FF;
        self.status.remove(GPUStatus::DrawModeFlags);
        self.status.insert(GPUStatus::from_bits_truncate(low_bits));
        self.status.set(GPUStatus::TexDisable, test_bit!(param, 11));

        // TODO: x-flip and y-flip

        GP0Command::DrawMode(self.status)
    }

    fn texture_window_setting(&mut self, param: u32) -> GP0Command {
        self.tex_window = param & 0xFF_FFFF;
        GP0Command::TexWindow(self.tex_window)
    }

    fn set_draw_area_top_left(&mut self, param: u32) -> GP0Command {
        self.draw_area_top_left = param & 0xFF_FFFF;
        GP0Command::DrawAreaTopLeft(self.draw_area_top_left)
    }

    fn set_draw_area_bottom_right(&mut self, param: u32) -> GP0Command {
        self.draw_area_bottom_right = param & 0xFF_FFFF;
        GP0Command::DrawAreaBottomRight(self.draw_area_bottom_right)
    }

    fn set_draw_offset(&mut self, param: u32) -> GP0Command {
        self.draw_offset = param & 0xFF_FFFF;
        GP0Command::DrawOffset(self.draw_offset)
    }

    fn mask_bit_setting(&mut self, param: u32) -> GP0Command {
        let set_mask = test_bit!(param, 0);
        let check_mask = test_bit!(param, 1);
        self.status.set(GPUStatus::SetDrawMask, set_mask);
        self.status.set(GPUStatus::MaskDrawing, check_mask);
        GP0Command::MaskBit { set_mask, check_mask }
    }

    fn poly_line(&mut self, transparent: bool) -> Option<GP0Command> {
        if self.poly_line_buf.is_empty() {
            //self.status.remove(GPUStatus::CommandReady);
            self.poly_line_buf.push(self.command_data[0]);
            self.poly_line_buf.push(self.command_data[2]);
            self.pending_command_words = 1;
            Some(GP0Command::DrawLine { params: std::array::from_fn(|n| self.command_data[n]), transparent })
        } else {
            if self.command_data[0] == POLY_LINE_TERM {
                self.poly_line_buf.clear();
                None
            } else {
                //self.status.remove(GPUStatus::CommandReady);
                let params = [
                    self.poly_line_buf[0],
                    self.poly_line_buf[1],
                    self.command_data[0]
                ];
                self.poly_line_buf[1] = self.command_data[0];
                self.pending_command_words = 1;
                Some(GP0Command::DrawLine { params, transparent })
            }
        }
    }

    fn shaded_poly_line(&mut self, transparent: bool) -> Option<GP0Command> {
        if self.poly_line_buf.is_empty() {
            //self.status.remove(GPUStatus::CommandReady);
            self.poly_line_buf.push(self.command_data[2]);
            self.poly_line_buf.push(self.command_data[3]);
            self.pending_command_words = 1;
            Some(GP0Command::DrawShadedLine { params: std::array::from_fn(|n| self.command_data[n]), transparent })
        } else {
            //self.status.remove(GPUStatus::CommandReady);
            if self.poly_line_buf.len() == 2 {
                if self.command_data[0] == POLY_LINE_TERM {
                    self.poly_line_buf.clear();
                } else {
                    self.poly_line_buf.push(self.command_data[0]);
                    self.pending_command_words = 1;
                }
                None
            } else {
                let params = [
                    self.poly_line_buf[0],
                    self.poly_line_buf[1],
                    self.poly_line_buf[2],
                    self.command_data[0]
                ];
                self.poly_line_buf[0] = self.poly_line_buf[2];
                self.poly_line_buf[1] = self.command_data[0];
                self.poly_line_buf.resize(2, 0);
                self.pending_command_words = 1;
                Some(GP0Command::DrawShadedLine { params, transparent })
            }
        }
    }

    fn blit_cpu_to_vram(&mut self) -> GP0Command {
        self.data_words = Size::from_xy(self.command_data[2]).copy_clip().word_count() as usize;
        self.block_count = 0x10;
        self.status.remove(GPUStatus::CommandReady);
        self.status.insert(GPUStatus::DMARecvReady);
        self.update_dma_status();
        GP0Command::BlitCPUtoVRAM{params: std::array::from_fn(|n| self.command_data[n+1])}
    }

    fn blit_vram_to_cpu(&mut self) -> GP0Command {
        self.data_words = Size::from_xy(self.command_data[2]).copy_clip().word_count() as usize;
        self.block_count = 0x10;
        self.status.remove(GPUStatus::CommandReady);
        self.status.insert(GPUStatus::VRAMSendReady);
        self.update_dma_status();
        GP0Command::BlitVRAMtoCPU{params: std::array::from_fn(|n| self.command_data[n+1])}
    }
}

/// State of interlace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterlaceState {
    Off,    // Not drawing in interlace mode.
    Even,
    Odd,
}

impl InterlaceState {
    pub fn toggle(self) -> Self {
        use InterlaceState::*;
        match self {
            Off => Off,
            Even => Odd,
            Odd => Even,
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct GPUStatus: u32 {
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

        //const TransferReady = bits![28, 30];
        //const CommandTransferReady = bits![26, 30]; //?
    }
}

impl GPUStatus {
    pub fn h_res(&self) -> usize {
        match (self.intersection(GPUStatus::XResolution)).bits() >> 16 {
            0b000 => 256,
            0b010 => 320,
            0b100 => 512,
            0b110 => 640,
            _ => 368,
        }
    }

    pub fn v_res(&self) -> usize {
        const INTERLACE_BITS: GPUStatus = GPUStatus::YResolution.union(GPUStatus::Interlace);
        if self.contains(INTERLACE_BITS) {
            480
        } else {
            240
        }
    }
}
