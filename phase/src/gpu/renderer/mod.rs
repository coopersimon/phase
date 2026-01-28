mod software;

use std::sync::{
    Arc, Mutex,
    atomic::{Ordering, AtomicU32}
};

use crossbeam_channel::{
    Sender, Receiver
};

use crate::{
    Frame, gpu::InterlaceState, utils::bits::*
};

use software::SoftwareRenderer;

const VRAM_SIZE: usize = 1024 * 1024;

pub enum RendererCmd {
    /// A command word sent via GP0.
    /// This could be a full or partial command,
    /// or data to be written to VRAM.
    GP0(u32),
    /// A new frame has begun, and we want to capture
    /// the frame buffer.
    GetFrame(InterlaceState),

    // GP1 commands:
    AcknowledgeIRQ,
    DisplayEnable(bool),
    DataRequest(GPUStatus),
    DisplayVRAMOffset(u32),
    DisplayXRange(u32),
    DisplayYRange(u32),
    DisplayMode(GPUStatus),
    TexDisable(bool),
}

/// GPU renderer.
/// 
/// This lives on a different thread.
/// It receives GP0 commands and dispatches render calls.
/// 
/// It also manages VRAM.
pub struct Renderer {
    // Comms
    command_rx: Receiver<RendererCmd>,
    frame_tx: Sender<()>,
    vram_tx: Sender<u32>,
    atomic_status: Arc<AtomicU32>,

    // Internal state
    status: GPUStatus,
    frame: Arc<Mutex<Frame>>,
    staging_buffer: Vec<u16>,

    renderer: Box<dyn RendererImpl>,
}

impl Renderer {
    pub fn new(command_rx: Receiver<RendererCmd>, frame_tx: Sender<()>, vram_tx: Sender<u32>, status: Arc<AtomicU32>, frame: Arc<Mutex<Frame>>) -> Self {
        let init_status = GPUStatus::CommandReady | GPUStatus::DMARecvReady;
        status.store(init_status.bits(), Ordering::Release);
        let renderer = Box::new(SoftwareRenderer::new());
        frame.lock().unwrap().resize((320, 240));
        Self {
            command_rx,
            frame_tx,
            vram_tx,
            atomic_status: status,

            status: init_status,
            frame,
            staging_buffer: Vec::new(),

            renderer,
        }
    }

    /// Run in a separate thread.
    pub fn run(&mut self) {
        while let Ok(cmd) = self.command_rx.recv() {
            if let Some(gp0_data) = self.handle_command(cmd) {
                self.exec_gp0_command(gp0_data);
            }
        }
    }

    /// Handle a command. If it was a GP0 command, return the data word.
    fn handle_command(&mut self, command: RendererCmd) -> Option<u32> {
        use RendererCmd::*;
        match command {
            GP0(data)                   => return Some(data),
            GetFrame(interlace)         => self.send_frame(interlace),
            AcknowledgeIRQ              => self.acknowledge_irq(),
            DisplayEnable(enable)       => self.display_enable(enable),
            DataRequest(data_req_stat)  => self.data_request(data_req_stat),
            DisplayVRAMOffset(offset)   => self.display_vram_offset(offset),
            DisplayXRange(range)        => self.display_range_x(range),
            DisplayYRange(range)        => self.display_range_y(range),
            DisplayMode(disp_mode_stat) => self.display_mode(disp_mode_stat),
            TexDisable(disable)         => self.tex_disable(disable),
        }
        None
    }

    fn send_frame(&mut self, interlace_state: InterlaceState) {
        {
            let mut frame = self.frame.lock().unwrap();
            self.renderer.get_frame(&mut frame, interlace_state);
        }
        let _ = self.frame_tx.send(());
    }

    fn get_parameter(&mut self) -> u32 {
        while let Ok(cmd) = self.command_rx.recv() {
            if let Some(gp0_data) = self.handle_command(cmd) {
                return gp0_data;
            }
        }
        // TODO: handle this more gracefully.
        panic!("command receiver failed");
    }
}

// GP1.
impl Renderer {
    fn acknowledge_irq(&mut self) {
        self.status.remove(GPUStatus::IRQ);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
    }

    fn display_enable(&mut self, enable: bool) {
        self.status.set(GPUStatus::DisplayEnable, enable);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
        self.renderer.enable_display(enable);
    }

    fn data_request(&mut self, data_req_stat: GPUStatus) {
        self.status.remove(GPUStatus::DMAMode);
        self.status.insert(data_req_stat);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
    }

    fn display_vram_offset(&mut self, offset: u32) {
        let coord = Coord {
            x: (offset & 0x3FF) as u16,
            y: ((offset >> 10) & 0x1FF) as u16
        };
        self.renderer.set_display_offset(coord);
    }

    fn display_range_x(&mut self, range: u32) {
        let begin = range & 0xFFF;
        let end = (range >> 12) & 0xFFF;
        self.renderer.set_display_range_x(begin, end);
    }

    fn display_range_y(&mut self, range: u32) {
        let begin = range & 0x7FF;
        let end = (range >> 10) & 0x7FF;
        self.renderer.set_display_range_y(begin, end);
    }

    fn display_mode(&mut self, disp_mode_stat: GPUStatus) {
        self.status.remove(GPUStatus::DispModeFlags);
        self.status.insert(disp_mode_stat);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
        let h_res = self.status.h_res();
        let v_res = self.status.v_res();
        self.frame.lock().unwrap().resize((h_res, v_res));
        self.renderer.set_display_resolution(Size { width: h_res as u16, height: v_res as u16 });
    }

    fn tex_disable(&mut self, disable: bool) {
        self.status.set(GPUStatus::TexDisable, disable);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
    }
}

const POLY_LINE_TERM: u32 = 0x5555_5555;

// GP0.
impl Renderer {
    fn exec_gp0_command(&mut self, data: u32) {
        println!("GP0 command: {:X}", data);
        self.status.remove(GPUStatus::CommandReady);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
        let command = (data >> 24) as u8;
        match command {
            0x00 => {}, // NOP
            0x01 => self.clear_cache(),
            0x02 => self.fill_rectangle(data),

            0x1F => self.irq(),

            0x20 => self.draw_tri(data, false),
            0x22 => self.draw_tri(data, true),
            0x24 => self.draw_textured_blended_tri(data, false),
            0x25 => self.draw_textured_tri(false),
            0x26 => self.draw_textured_blended_tri(data, true),
            0x27 => self.draw_textured_tri(true),
            0x28 => self.draw_quad(data, false),
            0x2A => self.draw_quad(data, true),
            0x2C => self.draw_textured_blended_quad(data, false),
            0x2D => self.draw_textured_quad(false),
            0x2E => self.draw_textured_blended_quad(data, true),
            0x2F => self.draw_textured_quad(true),
            0x30 => self.draw_shaded_tri(data, false),
            0x32 => self.draw_shaded_tri(data, true),
            0x34 => self.draw_textured_shaded_tri(data, false),
            0x36 => self.draw_textured_shaded_tri(data, true),
            0x38 => self.draw_shaded_quad(data, false),
            0x3A => self.draw_shaded_quad(data, true),
            0x3C => self.draw_textured_shaded_quad(data, false),
            0x3E => self.draw_textured_shaded_quad(data, true),

            0x40 => self.draw_line(data, false),
            0x42 => self.draw_line(data, true),
            0x48 => self.draw_poly_line(data, false),
            0x4A => self.draw_poly_line(data, true),
            0x50 => self.draw_shaded_line(data, false),
            0x52 => self.draw_shaded_line(data, true),
            0x58 => self.draw_shaded_poly_line(data, false),
            0x5A => self.draw_shaded_poly_line(data, true),

            0x60 => self.draw_rectangle(data, false),
            0x62 => self.draw_rectangle(data, true),
            0x64 => self.draw_tex_rectangle(Some(data), false),
            0x65 => self.draw_tex_rectangle(None, false),
            0x66 => self.draw_tex_rectangle(Some(data), true),
            0x67 => self.draw_tex_rectangle(None, true),
            0x68 => self.draw_fixed_rectangle(data, false, 1),
            0x6A => self.draw_fixed_rectangle(data, true, 1),
            0x6C => self.draw_tex_fixed_rectangle(Some(data), false, 1),
            0x6D => self.draw_tex_fixed_rectangle(None, false, 1),
            0x6E => self.draw_tex_fixed_rectangle(Some(data), true, 1),
            0x6F => self.draw_tex_fixed_rectangle(None, true, 1),
            0x70 => self.draw_fixed_rectangle(data, false, 8),
            0x72 => self.draw_fixed_rectangle(data, true, 8),
            0x74 => self.draw_tex_fixed_rectangle(Some(data), false, 8),
            0x75 => self.draw_tex_fixed_rectangle(None, false, 8),
            0x76 => self.draw_tex_fixed_rectangle(Some(data), true, 8),
            0x77 => self.draw_tex_fixed_rectangle(None, true, 8),
            0x78 => self.draw_fixed_rectangle(data, false, 16),
            0x7A => self.draw_fixed_rectangle(data, true, 16),
            0x7C => self.draw_tex_fixed_rectangle(Some(data), false, 16),
            0x7D => self.draw_tex_fixed_rectangle(None, false, 16),
            0x7E => self.draw_tex_fixed_rectangle(Some(data), true, 16),
            0x7F => self.draw_tex_fixed_rectangle(None, true, 16),

            0x80 => self.blit_vram_to_vram(),
            0xA0 => self.blit_cpu_to_vram(),
            0xC0 => self.blit_vram_to_cpu(),

            0xE1 => self.draw_mode_setting(data),
            0xE2 => self.texture_window_setting(data),
            0xE3 => self.set_draw_area_top_left(data),
            0xE4 => self.set_draw_area_bottom_right(data),
            0xE5 => self.set_draw_offset(data),
            0xE6 => self.mask_bit_setting(data),

            _ => panic!("Invalid GP0 command: {:X}", data),
        }
        self.status.insert(GPUStatus::CommandReady);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
    }

    fn clear_cache(&mut self) {
        // TODO.
    }

    /// Fill a rectangle.
    fn fill_rectangle(&mut self, rgb: u32) {
        let top_left = self.get_parameter();
        let size = self.get_parameter();

    }

    /// Draw a triangle.
    fn draw_tri(&mut self, rgb: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let vertex_2 = self.get_parameter();
        let vertex_3 = self.get_parameter();
        let vertices = [
            Vertex::from_xy(vertex_1).set_col(rgb),
            Vertex::from_xy(vertex_2).set_col(rgb),
            Vertex::from_xy(vertex_3).set_col(rgb),
        ];
        self.renderer.draw_triangle(&vertices, transparent);
    }

    fn draw_textured_tri(&mut self, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let texcoord_1 = self.get_parameter();
        let vertex_2 = self.get_parameter();
        let texcoord_2 = self.get_parameter();
        let vertex_3 = self.get_parameter();
        let texcoord_3 = self.get_parameter();
    }

    fn draw_textured_blended_tri(&mut self, rgb: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let texcoord_1 = self.get_parameter();
        let vertex_2 = self.get_parameter();
        let texcoord_2 = self.get_parameter();
        let vertex_3 = self.get_parameter();
        let texcoord_3 = self.get_parameter();
    }

    fn draw_shaded_tri(&mut self, rgb_1: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let rgb_2 = self.get_parameter();
        let vertex_2 = self.get_parameter();
        let rgb_3 = self.get_parameter();
        let vertex_3 = self.get_parameter();
    }

    fn draw_textured_shaded_tri(&mut self, rgb_1: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let texcoord_1 = self.get_parameter();
        let rgb_2 = self.get_parameter();
        let vertex_2 = self.get_parameter();
        let texcoord_2 = self.get_parameter();
        let rgb_3 = self.get_parameter();
        let vertex_3 = self.get_parameter();
        let texcoord_3 = self.get_parameter();
    }

    /// Draw a quad.
    fn draw_quad(&mut self, rgb: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let vertex_2 = self.get_parameter();
        let vertex_3 = self.get_parameter();
        let vertex_4 = self.get_parameter();
        let vertices = [
            Vertex::from_xy(vertex_1).set_col(rgb),
            Vertex::from_xy(vertex_2).set_col(rgb),
            Vertex::from_xy(vertex_3).set_col(rgb),
            Vertex::from_xy(vertex_4).set_col(rgb),
        ];
        self.renderer.draw_triangle(&vertices[0..3], transparent);
        self.renderer.draw_triangle(&vertices[1..4], transparent);
    }

    fn draw_textured_quad(&mut self, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let texcoord_1 = self.get_parameter();
        let vertex_2 = self.get_parameter();
        let texcoord_2 = self.get_parameter();
        let vertex_3 = self.get_parameter();
        let texcoord_3 = self.get_parameter();
        let vertex_4 = self.get_parameter();
        let texcoord_4 = self.get_parameter();
    }

    fn draw_textured_blended_quad(&mut self, rgb: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let texcoord_1 = self.get_parameter();
        let vertex_2 = self.get_parameter();
        let texcoord_2 = self.get_parameter();
        let vertex_3 = self.get_parameter();
        let texcoord_3 = self.get_parameter();
        let vertex_4 = self.get_parameter();
        let texcoord_4 = self.get_parameter();
    }

    fn draw_shaded_quad(&mut self, rgb_1: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let rgb_2 = self.get_parameter();
        let vertex_2 = self.get_parameter();
        let rgb_3 = self.get_parameter();
        let vertex_3 = self.get_parameter();
        let rgb_4 = self.get_parameter();
        let vertex_4 = self.get_parameter();
    }

    fn draw_textured_shaded_quad(&mut self, rgb_1: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let texcoord_1 = self.get_parameter();
        let rgb_2 = self.get_parameter();
        let vertex_2 = self.get_parameter();
        let texcoord_2 = self.get_parameter();
        let rgb_3 = self.get_parameter();
        let vertex_3 = self.get_parameter();
        let texcoord_3 = self.get_parameter();
        let rgb_4 = self.get_parameter();
        let vertex_4 = self.get_parameter();
        let texcoord_4 = self.get_parameter();
    }

    // Lines

    fn draw_line(&mut self, rgb: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let vertex_2 = self.get_parameter();
    }

    fn draw_poly_line(&mut self, rgb: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        loop {
            let vertex_n = self.get_parameter();
            if vertex_n == POLY_LINE_TERM {
                break;
            }
        }
    }

    fn draw_shaded_line(&mut self, rgb_1: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        let rgb_2 = self.get_parameter();
        let vertex_2 = self.get_parameter();
    }

    fn draw_shaded_poly_line(&mut self, rgb_1: u32, transparent: bool) {
        let vertex_1 = self.get_parameter();
        loop {
            let rgb_n = self.get_parameter();
            if rgb_n == POLY_LINE_TERM {
                break;
            }
            let vertex_n = self.get_parameter();
        }
    }

    fn draw_rectangle(&mut self, rgb: u32, transparent: bool) {
        let top_left = self.get_parameter();
        let size = self.get_parameter();

    }

    fn draw_fixed_rectangle(&mut self, rgb: u32, transparent: bool, size: usize) {
        let top_left = self.get_parameter();
    }

    fn draw_tex_rectangle(&mut self, rgb: Option<u32>, transparent: bool) {
        let top_left = self.get_parameter();
        let tex_info = self.get_parameter();
        let size = self.get_parameter();
    }

    fn draw_tex_fixed_rectangle(&mut self, rgb: Option<u32>, transparent: bool, size: usize) {
        let top_left = self.get_parameter();
        let tex_info = self.get_parameter();
    }

    // Data copy

    fn blit_vram_to_vram(&mut self) {
        let source = self.get_parameter();
        let dest = self.get_parameter();
        let size = self.get_parameter();
        self.renderer.copy_vram_block(Coord::from_xy(source), Coord::from_xy(dest), Size::from_xy(size));
    }

    fn blit_cpu_to_vram(&mut self) {
        let dest = Coord::from_xy(self.get_parameter());
        let size = Size::from_xy(self.get_parameter());
        let data_words = size.word_count();
        self.staging_buffer.clear();
        for _ in 0..data_words {
            let data = self.get_parameter();
            self.staging_buffer.push((data & 0xFFFF) as u16);
            self.staging_buffer.push(((data >> 16) & 0xFFFF) as u16);
        }
        self.renderer.write_vram_block(&self.staging_buffer, dest, size);
    }

    fn blit_vram_to_cpu(&mut self) {
        let source = Coord::from_xy(self.get_parameter());
        let size = Size::from_xy(self.get_parameter());
        let data_words = size.word_count() as usize;
        self.staging_buffer.resize(data_words * 2, 0);
        self.renderer.read_vram_block(&mut self.staging_buffer, source, size);
        // Send.
        for i in 0..data_words {
            let idx = i * 2;
            let data_lo = self.staging_buffer[idx] as u32;
            let data_hi = self.staging_buffer[idx + 1] as u32;
            let data = (data_hi << 16) | data_lo;
            let _ = self.vram_tx.send(data);
        }
    }

    // Settings

    fn irq(&mut self) {
        self.status.insert(GPUStatus::IRQ);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
    }

    fn draw_mode_setting(&mut self, param: u32) {
        let low_bits = param & 0x7FF;
        self.status.remove(GPUStatus::DrawModeFlags);
        self.status.insert(GPUStatus::from_bits_truncate(low_bits));
        self.status.set(GPUStatus::TexDisable, test_bit!(param, 11));
        self.atomic_status.store(self.status.bits(), Ordering::Release);

        // TODO: x-flip and y-flip

        // TODO: inform renderer.
    }

    fn texture_window_setting(&mut self, param: u32) {
        // TODO.
    }

    fn set_draw_area_top_left(&mut self, param: u32) {
        let left = (param & 0x3FF) as u16;
        let top = ((param >> 10) & 0x1FF) as u16;
        self.renderer.set_draw_area_top_left(left, top);
    }

    fn set_draw_area_bottom_right(&mut self, param: u32) {
        let right = (param & 0x3FF) as u16;
        let bottom = ((param >> 10) & 0x1FF) as u16;
        self.renderer.set_draw_area_bottom_right(right, bottom);
    }

    fn set_draw_offset(&mut self, param: u32) {
        let x = (param & 0x7FF) as i16;
        let y = ((param >> 10) & 0x7FF) as i16;
        let signed_x = (x << 5) >> 5;
        let signed_y = (y << 5) >> 5;
        self.renderer.set_draw_area_offset(signed_x, signed_y);
    }

    fn mask_bit_setting(&mut self, param: u32) {
        let set_mask_bit = test_bit!(param, 0);
        let check_mask_bit = test_bit!(param, 1);
        self.status.set(GPUStatus::SetDrawMask, set_mask_bit);
        self.status.set(GPUStatus::MaskDrawing, check_mask_bit);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
        self.renderer.set_mask_settings(set_mask_bit, check_mask_bit);
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
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

        const TransferReady = bits![28, 30];
        //const CommandTransferReady = bits![26, 30]; //?
    }
}

impl GPUStatus {
    pub fn h_res(&self) -> usize {
        match (*self & GPUStatus::XResolution).bits() >> 16 {
            0b000 => 256,
            0b010 => 320,
            0b100 => 512,
            0b110 => 640,
            _ => 368,
        }
    }

    pub fn v_res(&self) -> usize {
        let interlace_bits = GPUStatus::YResolution | GPUStatus::Interlace;
        if self.contains(interlace_bits) {
            480
        } else {
            240
        }
    }
}

/// The code responsible for doing actual drawing
/// should implement this trait.
trait RendererImpl {
    /// The frame provided should be of the correct resolution.
    /// It is of format BGRA U8.
    fn get_frame(&mut self, frame: &mut Frame, interlace: InterlaceState);

    fn write_vram_block(&mut self, data_in: &[u16], to: Coord, size: Size);
    fn read_vram_block(&mut self, data_out: &mut [u16], from: Coord, size: Size);
    fn copy_vram_block(&mut self, from: Coord, to: Coord, size: Size);

    fn enable_display(&mut self, enable: bool);
    fn set_display_offset(&mut self, offset: Coord);
    fn set_display_range_x(&mut self, begin: u32, end: u32);
    fn set_display_range_y(&mut self, begin: u32, end: u32);
    fn set_display_resolution(&mut self, res: Size);

    fn set_draw_area_top_left(&mut self, left: u16, top: u16);
    fn set_draw_area_bottom_right(&mut self, right: u16, bottom: u16);
    fn set_draw_area_offset(&mut self, x: i16, y: i16);
    fn set_mask_settings(&mut self, set_mask_bit: bool, check_mask_bit: bool);

    fn draw_triangle(&mut self, vertices: &[Vertex], transparent: bool);
    //fn draw_triangle_tex(&mut self, vertices: &[Vertex], tex_info: TexInfo, transparent: bool);
}

struct Coord {
    x: u16,
    y: u16,
}

impl Coord {
    #[inline(always)]
    fn from_xy(xy: u32) -> Self {
        Self {
            x: (xy & 0xFFFF) as u16,
            y: ((xy >> 16) & 0xFFFF) as u16,
        }
    }

    /// Get byte index into VRAM.
    fn get_vram_idx(&self) -> usize {
        (self.x as usize) * 2 + (self.y as usize) * 2048
    }
}

struct Size {
    width: u16,
    height: u16,
}

impl Size {
    #[inline(always)]
    fn from_xy(xy: u32) -> Self {
        Self {
            width: (xy & 0xFFFF) as u16,
            height: ((xy >> 16) & 0xFFFF) as u16,
        }
    }

    /// Get number of 32-bit words needed to hold this data.
    /// Each pixel is 2 bytes.
    #[inline(always)]
    fn word_count(&self) -> u32 {
        ((self.width as u32) * (self.height as u32) + 1) / 2
    }
}

struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    fn white() -> Self {
        Self {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
        }
    }

    fn from_rgb15(rgb: u16) -> Self {
        let r = (rgb & 0x1F) as u8;
        let g = ((rgb >> 5) & 0x1F) as u8;
        let b = ((rgb >> 10) & 0x1F) as u8;
        Self {
            r: (r << 3) | (r >> 2),
            g: (g << 3) | (g >> 2),
            b: (b << 3) | (b >> 2),
        }
    }

    fn from_rgb24(rgb: u32) -> Self {
        Self {
            r: (rgb & 0xFF) as u8,
            g: ((rgb >> 8) & 0xFF) as u8,
            b: ((rgb >> 16) & 0xFF) as u8,
        }
    }

    fn to_rgb15(&self) -> u16 {
        let r = (self.r >> 3) as u16;
        let g = (self.g >> 3) as u16;
        let b = (self.b >> 3) as u16;
        r | (g << 5) | (b << 10)
    }
}

struct Vertex {
    coord: Coord,
    col: Color,
    tex_s: u8,
    tex_t: u8,
}

impl Vertex {
    #[inline(always)]
    fn from_xy(xy: u32) -> Self {
        Self {
            coord: Coord::from_xy(xy),
            col: Color::white(),
            tex_s: 0,
            tex_t: 0,
        }
    }

    #[inline(always)]
    fn set_col(mut self, rgb: u32) -> Self {
        self.col = Color::from_rgb24(rgb);
        self
    }

    #[inline(always)]
    fn set_tex(mut self, tex: u32) -> Self {
        self.tex_s = (tex & 0xFF) as u8;
        self.tex_t = ((tex >> 8) & 0xFF) as u8;
        self
    }
}

struct TexInfo {
    palette: u16,
    page: u16,
}