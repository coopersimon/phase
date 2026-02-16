mod software;

use std::sync::{
    Arc, Mutex
};

use crossbeam_channel::{
    Sender, Receiver
};

use crate::{
    Frame
};
use super::{
    InterlaceState, GPUStatus
};

use software::SoftwareRenderer;

const VRAM_SIZE: usize = 1024 * 1024;

// TODO: make this configurable.
const DEBUG_MODE: bool = false;

#[derive(Debug)]
pub enum RendererCmd {
    /// A new frame has begun, and we want to capture
    /// the frame buffer.
    GetFrame(InterlaceState),

    // GP0 commands:
    GP0(GP0Command),
    GP0Data(u32),

    // GP1 commands:
    DisplayEnable(bool),
    DisplayVRAMOffset(u32),
    DisplayXRange(u32),
    DisplayYRange(u32),
    DisplayMode {
        h_res: usize,
        v_res: usize,
        interlace: bool,
        rgb24: bool,
    },
    TexDisable(bool),
}

#[derive(Debug)]
pub enum GP0Command {
    ClearCache,
    FillRectangle([u32; 3]),

    DrawTri{params: [u32; 4], transparent: bool},
    DrawTexTri{params: [u32; 6], transparent: bool},
    DrawTexBlendTri{params: [u32; 7], transparent: bool},
    DrawShadedTri{params: [u32; 6], transparent: bool},
    DrawTexShadedTri{params: [u32; 9], transparent: bool},

    DrawQuad{params: [u32; 5], transparent: bool},
    DrawTexQuad{params: [u32; 8], transparent: bool},
    DrawTexBlendQuad{params: [u32; 9], transparent: bool},
    DrawShadedQuad{params: [u32; 8], transparent: bool},
    DrawTexShadedQuad{params: [u32; 12], transparent: bool},

    DrawLine{params: [u32; 3], transparent: bool},
    DrawShadedLine{params: [u32; 4], transparent: bool},

    DrawRect{params: [u32; 3], transparent: bool},
    DrawTexRect{params: [u32; 3], transparent: bool},
    DrawTexBlendedRect{params: [u32; 4], transparent: bool},
    DrawFixedRect{params: [u32; 2], size: u16, transparent: bool},
    DrawTexFixedRect{params: [u32; 2], size: u16, transparent: bool},
    DrawTexBlendedFixedRect{params: [u32; 3], size: u16, transparent: bool},

    BlitVRAMtoVRAM{params: [u32; 3]},
    BlitCPUtoVRAM{params: [u32; 2]},
    BlitVRAMtoCPU{params: [u32; 2]},

    DrawMode(GPUStatus),
    TexWindow(u32),
    DrawAreaTopLeft(u32),
    DrawAreaBottomRight(u32),
    DrawOffset(u32),
    MaskBit{set_mask: bool, check_mask: bool}
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

    // Internal state
    frame: Arc<Mutex<Frame>>,
    staging_buffer: Vec<u16>,
    tex_mode: u16,

    renderer: Box<dyn RendererImpl>,
}

impl Renderer {
    pub fn new(command_rx: Receiver<RendererCmd>, frame_tx: Sender<()>, vram_tx: Sender<u32>, frame: Arc<Mutex<Frame>>) -> Self {
        let renderer = Box::new(SoftwareRenderer::new());
        frame.lock().unwrap().resize((320, 240));
        Self {
            command_rx,
            frame_tx,
            vram_tx,

            frame,
            staging_buffer: Vec::new(),
            tex_mode: 0,

            renderer,
        }
    }

    /// Run in a separate thread.
    pub fn run(&mut self) {
        while let Ok(cmd) = self.command_rx.recv() {
            self.handle_command(cmd);
        }
    }

    /// Handle a command.
    fn handle_command(&mut self, command: RendererCmd) -> Option<u32> {
        use RendererCmd::*;
        use GP0Command::*;
        //println!("renderer exec: {:?}", command);
        match command {
            GetFrame(interlace)         => self.send_frame(interlace),

            GP0Data(data)               => return Some(data),
            GP0(ClearCache)             => self.clear_cache(),
            GP0(FillRectangle(params))  => self.fill_rectangle(&params),

            GP0(DrawTri{params, transparent})           => self.draw_tri(&params, transparent),
            GP0(DrawTexTri{params, transparent})        => self.draw_textured_tri(&params, transparent),
            GP0(DrawTexBlendTri{params, transparent})   => self.draw_textured_blended_tri(&params, transparent),
            GP0(DrawShadedTri{params, transparent})     => self.draw_shaded_tri(&params, transparent),
            GP0(DrawTexShadedTri{params, transparent})  => self.draw_textured_shaded_tri(&params, transparent),

            GP0(DrawQuad{params, transparent})          => self.draw_quad(&params, transparent),
            GP0(DrawTexQuad{params, transparent})       => self.draw_textured_quad(&params, transparent),
            GP0(DrawTexBlendQuad{params, transparent})  => self.draw_textured_blended_quad(&params, transparent),
            GP0(DrawShadedQuad{params, transparent})    => self.draw_shaded_quad(&params, transparent),
            GP0(DrawTexShadedQuad{params, transparent}) => self.draw_textured_shaded_quad(&params, transparent),

            GP0(DrawLine{params, transparent})          => self.draw_line(&params, transparent),
            GP0(DrawShadedLine{params, transparent})    => self.draw_shaded_line(&params, transparent),

            GP0(DrawRect{params, transparent})                      => self.draw_rectangle(&params, transparent),
            GP0(DrawTexRect{params, transparent})                   => self.draw_tex_rectangle(&params, transparent),
            GP0(DrawTexBlendedRect{params, transparent})            => self.draw_tex_blended_rectangle(&params, transparent),
            GP0(DrawFixedRect{params, size, transparent})           => self.draw_fixed_rectangle(&params, transparent, size),
            GP0(DrawTexFixedRect{params, size, transparent})        => self.draw_tex_fixed_rectangle(&params, transparent, size),
            GP0(DrawTexBlendedFixedRect{params, size, transparent}) => self.draw_tex_blended_fixed_rectangle(&params, transparent, size),

            GP0(BlitVRAMtoVRAM{params}) => self.blit_vram_to_vram(&params),
            GP0(BlitCPUtoVRAM{params})  => self.blit_cpu_to_vram(&params),
            GP0(BlitVRAMtoCPU{params})  => self.blit_vram_to_cpu(&params),

            GP0(DrawMode(status))               => self.set_draw_mode(status),
            GP0(TexWindow(param))               => self.texture_window_setting(param),
            GP0(DrawAreaTopLeft(param))         => self.set_draw_area_top_left(param),
            GP0(DrawAreaBottomRight(param))     => self.set_draw_area_bottom_right(param),
            GP0(DrawOffset(param))              => self.set_draw_offset(param),
            GP0(MaskBit{set_mask, check_mask})  => self.renderer.set_mask_settings(set_mask, check_mask),

            DisplayEnable(enable)       => self.display_enable(enable),
            DisplayVRAMOffset(offset)   => self.display_vram_offset(offset),
            DisplayXRange(range)        => self.display_range_x(range),
            DisplayYRange(range)        => self.display_range_y(range),
            DisplayMode{h_res, v_res, interlace, rgb24}  => self.display_mode(h_res, v_res, interlace, rgb24),
            TexDisable(disable)         => self.tex_disable(disable),
        }
        None
    }

    fn get_data(&mut self) -> u32 {
        while let Ok(cmd) = self.command_rx.recv() {
            if let Some(data) = self.handle_command(cmd) {
                return data;
            }
        }
        // TODO: more graceful.
        panic!("command rx failed.");
    }

    fn send_frame(&mut self, interlace_state: InterlaceState) {
        {
            let mut frame = self.frame.lock().unwrap();
            self.renderer.get_frame(&mut frame, interlace_state, DEBUG_MODE);
        }
        let _ = self.frame_tx.send(());
    }
}

// GP1.
impl Renderer {
    fn display_enable(&mut self, enable: bool) {
        self.renderer.enable_display(enable);
    }

    fn display_vram_offset(&mut self, offset: u32) {
        let coord = Coord {
            x: (offset & 0x3FF) as i16,
            y: ((offset >> 10) & 0x1FF) as i16
        };
        self.renderer.set_display_offset(coord);
    }

    fn display_range_x(&mut self, range: u32) {
        let begin = range & 0xFFF;
        let end = (range >> 12) & 0xFFF;
        self.renderer.set_display_range_x(begin, end);
    }

    fn display_range_y(&mut self, range: u32) {
        let begin = range & 0x3FF;
        let end = (range >> 10) & 0x3FF;
        self.renderer.set_display_range_y(begin, end);
    }

    fn display_mode(&mut self, h_res: usize, v_res: usize, interlace: bool, rgb24: bool) {
        if DEBUG_MODE {
            self.frame.lock().unwrap().resize((1024, 512));
        } else {
            self.frame.lock().unwrap().resize((h_res, v_res));
        }
        self.renderer.set_display_resolution(Size { width: h_res as u16, height: v_res as u16 }, interlace);
        self.renderer.set_color_depth(rgb24);
    }

    fn tex_disable(&mut self, _disable: bool) {
        // TODO.
    }
}

// GP0.
impl Renderer {
    fn clear_cache(&mut self) {
        // TODO.
    }

    /// Fill a rectangle.
    fn fill_rectangle(&mut self, params: &[u32; 3]) {
        let color = Color::from_rgb24(params[0]);
        let top_left = Coord::from_xy(params[1]);
        let size = Size::from_xy(params[2]);
        self.renderer.fill_rectangle(color, top_left, size);
    }

    /// Draw a triangle.
    fn draw_tri(&mut self, params: &[u32; 4], transparent: bool) {
        let color = Color::from_rgb24(params[0]);
        let vertex_1 = params[1];
        let vertex_2 = params[2];
        let vertex_3 = params[3];
        let vertices = [
            Vertex::from_xy(vertex_1),
            Vertex::from_xy(vertex_2),
            Vertex::from_xy(vertex_3),
        ];
        self.renderer.draw_triangle_flat(&vertices, color, transparent);
    }

    fn draw_textured_tri(&mut self, params: &[u32; 6], transparent: bool) {
        let vertex_1 = params[0];
        let texcoord_1 = params[1];
        let vertex_2 = params[2];
        let texcoord_2 = params[3];
        let vertex_3 = params[4];
        let texcoord_3 = params[5];
        let vertices = [
            Vertex::from_xy(vertex_1).set_tex(texcoord_1),
            Vertex::from_xy(vertex_2).set_tex(texcoord_2),
            Vertex::from_xy(vertex_3).set_tex(texcoord_3),
        ];
        let tex_info = TexInfo::from_data(texcoord_1, texcoord_2);
        self.renderer.draw_triangle_tex(&vertices, &tex_info, transparent);
    }

    fn draw_textured_blended_tri(&mut self, params: &[u32; 7], transparent: bool) {
        let rgb = params[0];
        let vertex_1 = params[1];
        let texcoord_1 = params[2];
        let vertex_2 = params[3];
        let texcoord_2 = params[4];
        let vertex_3 = params[5];
        let texcoord_3 = params[6];
        let vertices = [
            Vertex::from_xy(vertex_1).set_col(rgb).set_tex(texcoord_1),
            Vertex::from_xy(vertex_2).set_col(rgb).set_tex(texcoord_2),
            Vertex::from_xy(vertex_3).set_col(rgb).set_tex(texcoord_3),
        ];
        let tex_info = TexInfo::from_data(texcoord_1, texcoord_2);
        self.renderer.draw_triangle_tex_blended(&vertices, &tex_info, transparent);
    }

    fn draw_shaded_tri(&mut self, params: &[u32; 6], transparent: bool) {
        let rgb_1 = params[0];
        let vertex_1 = params[1];
        let rgb_2 = params[2];
        let vertex_2 = params[3];
        let rgb_3 = params[4];
        let vertex_3 = params[5];
        let vertices = [
            Vertex::from_xy(vertex_1).set_col(rgb_1),
            Vertex::from_xy(vertex_2).set_col(rgb_2),
            Vertex::from_xy(vertex_3).set_col(rgb_3),
        ];
        self.renderer.draw_triangle_shaded(&vertices, transparent);
    }

    fn draw_textured_shaded_tri(&mut self, params: &[u32; 9], transparent: bool) {
        let rgb_1 = params[0];
        let vertex_1 = params[1];
        let texcoord_1 = params[2];
        let rgb_2 = params[3];
        let vertex_2 = params[4];
        let texcoord_2 = params[5];
        let rgb_3 = params[6];
        let vertex_3 = params[7];
        let texcoord_3 = params[8];
        let vertices = [
            Vertex::from_xy(vertex_1).set_col(rgb_1).set_tex(texcoord_1),
            Vertex::from_xy(vertex_2).set_col(rgb_2).set_tex(texcoord_2),
            Vertex::from_xy(vertex_3).set_col(rgb_3).set_tex(texcoord_3),
        ];
        let tex_info = TexInfo::from_data(texcoord_1, texcoord_2);
        self.renderer.draw_triangle_tex_blended(&vertices, &tex_info, transparent);
    }

    /// Draw a quad.
    fn draw_quad(&mut self, params: &[u32; 5], transparent: bool) {
        let color = Color::from_rgb24(params[0]);
        let vertex_1 = params[1];
        let vertex_2 = params[2];
        let vertex_3 = params[3];
        let vertex_4 = params[4];
        let vertices = [
            Vertex::from_xy(vertex_1),
            Vertex::from_xy(vertex_2),
            Vertex::from_xy(vertex_3),
            Vertex::from_xy(vertex_4),
        ];
        self.renderer.draw_triangle_flat(&vertices[0..3], color, transparent);
        self.renderer.draw_triangle_flat(&vertices[1..4], color, transparent);
    }

    fn draw_textured_quad(&mut self, params: &[u32; 8], transparent: bool) {
        let vertex_1 = params[0];
        let texcoord_1 = params[1];
        let vertex_2 = params[2];
        let texcoord_2 = params[3];
        let vertex_3 = params[4];
        let texcoord_3 = params[5];
        let vertex_4 = params[6];
        let texcoord_4 = params[7];
        let vertices = [
            Vertex::from_xy(vertex_1).set_tex(texcoord_1),
            Vertex::from_xy(vertex_2).set_tex(texcoord_2),
            Vertex::from_xy(vertex_3).set_tex(texcoord_3),
            Vertex::from_xy(vertex_4).set_tex(texcoord_4),
        ];
        let tex_info = TexInfo::from_data(texcoord_1, texcoord_2);
        self.renderer.draw_triangle_tex(&vertices[0..3], &tex_info, transparent);
        self.renderer.draw_triangle_tex(&vertices[1..4], &tex_info, transparent);
    }

    fn draw_textured_blended_quad(&mut self, params: &[u32; 9], transparent: bool) {
        let rgb = params[0];
        let vertex_1 = params[1];
        let texcoord_1 = params[2];
        let vertex_2 = params[3];
        let texcoord_2 = params[4];
        let vertex_3 = params[5];
        let texcoord_3 = params[6];
        let vertex_4 = params[7];
        let texcoord_4 = params[8];
        let vertices = [
            Vertex::from_xy(vertex_1).set_col(rgb).set_tex(texcoord_1),
            Vertex::from_xy(vertex_2).set_col(rgb).set_tex(texcoord_2),
            Vertex::from_xy(vertex_3).set_col(rgb).set_tex(texcoord_3),
            Vertex::from_xy(vertex_4).set_col(rgb).set_tex(texcoord_4),
        ];
        let tex_info = TexInfo::from_data(texcoord_1, texcoord_2);
        self.renderer.draw_triangle_tex_blended(&vertices[0..3], &tex_info, transparent);
        self.renderer.draw_triangle_tex_blended(&vertices[1..4], &tex_info, transparent);
    }

    fn draw_shaded_quad(&mut self, params: &[u32; 8], transparent: bool) {
        let rgb_1 = params[0];
        let vertex_1 = params[1];
        let rgb_2 = params[2];
        let vertex_2 = params[3];
        let rgb_3 = params[4];
        let vertex_3 = params[5];
        let rgb_4 = params[6];
        let vertex_4 = params[7];
        let vertices = [
            Vertex::from_xy(vertex_1).set_col(rgb_1),
            Vertex::from_xy(vertex_2).set_col(rgb_2),
            Vertex::from_xy(vertex_3).set_col(rgb_3),
            Vertex::from_xy(vertex_4).set_col(rgb_4),
        ];
        self.renderer.draw_triangle_shaded(&vertices[0..3], transparent);
        self.renderer.draw_triangle_shaded(&vertices[1..4], transparent);
    }

    fn draw_textured_shaded_quad(&mut self, params: &[u32; 12], transparent: bool) {
        let rgb_1 = params[0];
        let vertex_1 = params[1];
        let texcoord_1 = params[2];
        let rgb_2 = params[3];
        let vertex_2 = params[4];
        let texcoord_2 = params[5];
        let rgb_3 = params[6];
        let vertex_3 = params[7];
        let texcoord_3 = params[8];
        let rgb_4 = params[9];
        let vertex_4 = params[10];
        let texcoord_4 = params[11];
        let vertices = [
            Vertex::from_xy(vertex_1).set_col(rgb_1).set_tex(texcoord_1),
            Vertex::from_xy(vertex_2).set_col(rgb_2).set_tex(texcoord_2),
            Vertex::from_xy(vertex_3).set_col(rgb_3).set_tex(texcoord_3),
            Vertex::from_xy(vertex_4).set_col(rgb_4).set_tex(texcoord_4),
        ];
        let tex_info = TexInfo::from_data(texcoord_1, texcoord_2);
        self.renderer.draw_triangle_tex_blended(&vertices[0..3], &tex_info, transparent);
        self.renderer.draw_triangle_tex_blended(&vertices[1..4], &tex_info, transparent);
    }

    // Lines

    fn draw_line(&mut self, params: &[u32; 3], transparent: bool) {
        let rgb = params[0];
        let vertex_1 = params[1];
        let vertex_2 = params[2];
        let vertex_a = Vertex::from_xy(vertex_1).set_col(rgb);
        let vertex_b = Vertex::from_xy(vertex_2).set_col(rgb);
        self.renderer.draw_line(&vertex_a, &vertex_b, transparent);
    }

    fn draw_shaded_line(&mut self, params: &[u32; 4], transparent: bool) {
        let rgb_1 = params[0];
        let vertex_1 = params[1];
        let rgb_2 = params[2];
        let vertex_2 = params[3];
        let vertex_a = Vertex::from_xy(vertex_1).set_col(rgb_1);
        let vertex_b = Vertex::from_xy(vertex_2).set_col(rgb_2);
        self.renderer.draw_line(&vertex_a, &vertex_b, transparent);
    }

    fn draw_rectangle(&mut self, params: &[u32; 3], transparent: bool) {
        let color = Color::from_rgb24(params[0]);
        let top_left = Coord::from_xy(params[1]);
        let size = Size::from_xy(params[2]);
        self.renderer.draw_rectangle(color, top_left, size, transparent);
    }

    fn draw_fixed_rectangle(&mut self, params: &[u32; 2], transparent: bool, size: u16) {
        let color = Color::from_rgb24(params[0]);
        let top_left = Coord::from_xy(params[1]);
        let size = Size { width: size, height: size };
        self.renderer.draw_rectangle(color, top_left, size, transparent);
    }

    fn draw_tex_rectangle(&mut self, params: &[u32; 3], transparent: bool) {
        let color = Color::default();
        let top_left = Coord::from_xy(params[0]);
        let tex_data = params[1];
        let size = Size::from_xy(params[2]);
        let palette = PaletteCoord::from_data(tex_data);
        let tex_info = TexInfo::from_draw_mode(self.tex_mode, palette);
        let tex_coord = TexCoord::from_16(tex_data as u16);
        self.renderer.draw_rectangle_tex(color, tex_coord, &tex_info, top_left, size, transparent);
    }

    fn draw_tex_blended_rectangle(&mut self, params: &[u32; 4], transparent: bool) {
        let color = Color::from_rgb24(params[0]);
        let top_left = Coord::from_xy(params[1]);
        let tex_data = params[2];
        let size = Size::from_xy(params[3]);
        let palette = PaletteCoord::from_data(tex_data);
        let tex_info = TexInfo::from_draw_mode(self.tex_mode, palette);
        let tex_coord = TexCoord::from_16(tex_data as u16);
        self.renderer.draw_rectangle_tex(color, tex_coord, &tex_info, top_left, size, transparent);
    }

    fn draw_tex_fixed_rectangle(&mut self, params: &[u32; 2], transparent: bool, size: u16) {
        let color = Color::default();
        let top_left = Coord::from_xy(params[0]);
        let tex_data = params[1];
        let size = Size { width: size, height: size };
        let palette = PaletteCoord::from_data(tex_data);
        let tex_info = TexInfo::from_draw_mode(self.tex_mode, palette);
        let tex_coord = TexCoord::from_16(tex_data as u16);
        self.renderer.draw_rectangle_tex(color, tex_coord, &tex_info, top_left, size, transparent);
    }

    fn draw_tex_blended_fixed_rectangle(&mut self, params: &[u32; 3], transparent: bool, size: u16) {
        let color = Color::from_rgb24(params[0]);
        let top_left = Coord::from_xy(params[1]);
        let tex_data = params[2];
        let size = Size { width: size, height: size };
        let palette = PaletteCoord::from_data(tex_data);
        let tex_info = TexInfo::from_draw_mode(self.tex_mode, palette);
        let tex_coord = TexCoord::from_16(tex_data as u16);
        self.renderer.draw_rectangle_tex(color, tex_coord, &tex_info, top_left, size, transparent);
    }

    // Data copy

    fn blit_vram_to_vram(&mut self, params: &[u32; 3]) {
        let source = Coord::from_xy(params[0]).copy_clip();
        let dest = Coord::from_xy(params[1]).copy_clip();
        let size = Size::from_xy(params[2]).copy_clip();
        self.renderer.copy_vram_block(source, dest, size);
    }

    fn blit_cpu_to_vram(&mut self, params: &[u32; 2]) {
        // Mask to ensure within bounds
        let dest = Coord::from_xy(params[0]).copy_clip();
        let size = Size::from_xy(params[1]).copy_clip();
        let data_words = size.word_count();
        self.staging_buffer.clear();
        for _ in 0..data_words {
            let data = self.get_data();
            self.staging_buffer.push((data & 0xFFFF) as u16);
            self.staging_buffer.push(((data >> 16) & 0xFFFF) as u16);
        }
        self.renderer.write_vram_block(&self.staging_buffer, dest, size);
    }

    fn blit_vram_to_cpu(&mut self, params: &[u32; 2]) {
        let source = Coord::from_xy(params[0]).copy_clip();
        let size = Size::from_xy(params[1]).copy_clip();
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

    fn set_draw_mode(&mut self, status: GPUStatus) {
        self.tex_mode = status.intersection(GPUStatus::DrawModeFlags).bits() as u16;
        let trans_mode = match (status.intersection(GPUStatus::SemiTrans)).bits() >> 5 {
            0b00 => TransparencyMode::Average,
            0b01 => TransparencyMode::Add,
            0b10 => TransparencyMode::Subtract,
            0b11 => TransparencyMode::Combine,
            _ => unreachable!()
        };
        let dither = status.contains(GPUStatus::Dither);
        self.renderer.set_draw_mode(trans_mode, dither);
    }

    fn texture_window_setting(&mut self, param: u32) {
        let mask_s = ((param & 0x1F) as u8) << 3;
        let mask_t = (((param >> 5) & 0x1F) as u8) << 3;
        let offset_s = (((param >> 10) & 0x1F) as u8) << 3;
        let offset_t = (((param >> 15) & 0x1F) as u8) << 3;
        self.renderer.set_texture_window(mask_s, mask_t, offset_s, offset_t);
    }

    fn set_draw_area_top_left(&mut self, param: u32) {
        let left = (param & 0x3FF) as i16;
        let top = ((param >> 10) & 0x1FF) as i16;
        self.renderer.set_draw_area_top_left(left, top);
    }

    fn set_draw_area_bottom_right(&mut self, param: u32) {
        let right = (param & 0x3FF) as i16;
        let bottom = ((param >> 10) & 0x1FF) as i16;
        self.renderer.set_draw_area_bottom_right(right, bottom);
    }

    fn set_draw_offset(&mut self, param: u32) {
        let x = (param & 0x7FF) as i16;
        let y = ((param >> 11) & 0x7FF) as i16;
        let signed_x = (x << 5) >> 5;
        let signed_y = (y << 5) >> 5;
        self.renderer.set_draw_area_offset(signed_x, signed_y);
    }
}

/// The code responsible for doing actual drawing
/// should implement this trait.
trait RendererImpl {
    /// The frame provided should be of the correct resolution.
    /// It is of format BGRA U8.
    /// 
    /// "Debug" setting will draw the entire VRAM on-screen.
    fn get_frame(&mut self, frame: &mut Frame, interlace: InterlaceState, debug: bool);

    fn write_vram_block(&mut self, data_in: &[u16], to: Coord, size: Size);
    fn read_vram_block(&mut self, data_out: &mut [u16], from: Coord, size: Size);
    fn copy_vram_block(&mut self, from: Coord, to: Coord, size: Size);

    fn enable_display(&mut self, enable: bool);
    fn set_display_offset(&mut self, offset: Coord);
    fn set_display_range_x(&mut self, begin: u32, end: u32);
    fn set_display_range_y(&mut self, begin: u32, end: u32);
    fn set_display_resolution(&mut self, res: Size, interlace: bool);
    fn set_color_depth(&mut self, rgb24: bool);

    fn set_draw_mode(&mut self, trans_mode: TransparencyMode, dither: bool);
    fn set_texture_window(&mut self, mask_s: u8, mask_t: u8, offset_s: u8, offset_t: u8);
    fn set_draw_area_top_left(&mut self, left: i16, top: i16);
    fn set_draw_area_bottom_right(&mut self, right: i16, bottom: i16);
    fn set_draw_area_offset(&mut self, x: i16, y: i16);
    fn set_mask_settings(&mut self, set_mask_bit: bool, check_mask_bit: bool);

    fn fill_rectangle(&mut self, color: Color, top_left: Coord, size: Size);
    fn draw_triangle_flat(&mut self, vertices: &[Vertex], color: Color, transparent: bool);
    fn draw_triangle_shaded(&mut self, vertices: &[Vertex], transparent: bool);
    fn draw_triangle_tex(&mut self, vertices: &[Vertex], tex_info: &TexInfo, transparent: bool);
    fn draw_triangle_tex_blended(&mut self, vertices: &[Vertex], tex_info: &TexInfo, transparent: bool);
    fn draw_rectangle(&mut self, color: Color, top_left: Coord, size: Size, transparent: bool);
    fn draw_rectangle_tex(&mut self, color: Color, tex_coord: TexCoord, tex_info: &TexInfo, top_left: Coord, size: Size, transparent: bool);
    fn draw_line(&mut self, vertex_a: &Vertex, vertex_b: &Vertex, transparent: bool);
}

#[derive(Clone, Copy)]
struct Coord {
    x: i16,
    y: i16,
}

impl Coord {
    #[inline(always)]
    fn from_xy(xy: u32) -> Self {
        Self {
            x: (xy & 0xFFFF) as i16,
            y: ((xy >> 16) & 0xFFFF) as i16,
        }
    }

    #[inline(always)]
    fn copy_clip(self) -> Self {
        Self {
            x: self.x & 0x3FF,
            y: self.y & 0x1FF
        }
    }

    /// Get halfword index into VRAM.
    fn get_vram_idx(&self) -> usize {
        (self.x as usize) + (self.y as usize) * 1024
    }
}

#[derive(Clone, Copy)]
pub struct Size {
    width: u16,
    height: u16,
}

impl Size {
    #[inline(always)]
    pub fn from_xy(xy: u32) -> Self {
        Self {
            width: (xy & 0xFFFF) as u16,
            height: ((xy >> 16) & 0xFFFF) as u16,
        }
    }

    #[inline(always)]
    pub fn copy_clip(self) -> Self {
        Self {
            width: (self.width.wrapping_sub(1) & 0x3FF) + 1,
            height: (self.height.wrapping_sub(1) & 0x1FF) + 1,
        }
    }

    /// Get number of 32-bit words needed to hold this data.
    /// Each pixel is 2 bytes.
    #[inline(always)]
    pub fn word_count(&self) -> u32 {
        ((self.width as u32) * (self.height as u32) + 1) / 2
    }
}

#[derive(Clone, Copy)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
    mask: u16,
}

impl Default for Color {
    fn default() -> Self {
        Self {
            r: 0x80,
            g: 0x80,
            b: 0x80,
            mask: 0,
        }
    }
}

impl Color {
    fn from_rgb15(rgb: u16) -> Self {
        let r = (rgb & 0x1F) as u8;
        let g = ((rgb >> 5) & 0x1F) as u8;
        let b = ((rgb >> 10) & 0x1F) as u8;
        let mask = rgb & 0x8000;
        Self {
            r: (r << 3) | (r >> 2),
            g: (g << 3) | (g >> 2),
            b: (b << 3) | (b >> 2),
            mask,
        }
    }

    fn from_rgb24(rgb: u32) -> Self {
        Self {
            r: (rgb & 0xFF) as u8,
            g: ((rgb >> 8) & 0xFF) as u8,
            b: ((rgb >> 16) & 0xFF) as u8,
            mask: 0,
        }
    }

    fn to_rgb15(&self) -> u16 {
        let r = (self.r >> 3) as u16;
        let g = (self.g >> 3) as u16;
        let b = (self.b >> 3) as u16;
        self.mask | r | (g << 5) | (b << 10)
    }

    /// Blend a color with another. The other color is usually a texture color.
    /// Takes the mask of "other" (i.e. the texture) if specified.
    fn blend(&self, other: &Color, use_other_mask: bool) -> Color {
        Color {
            r: (((self.r as u16) * (other.r as u16)) >> 7).try_into().unwrap_or(0xFF),
            g: (((self.g as u16) * (other.g as u16)) >> 7).try_into().unwrap_or(0xFF),
            b: (((self.b as u16) * (other.b as u16)) >> 7).try_into().unwrap_or(0xFF),
            mask: if use_other_mask {other.mask} else {0x8000},
        }
    }

    fn dither(&self, dither_val: i8) -> Color {
        Color {
            r: (self.r as i16 + dither_val as i16).clamp(0, 0xFF) as u8,
            g: (self.g as i16 + dither_val as i16).clamp(0, 0xFF) as u8,
            b: (self.b as i16 + dither_val as i16).clamp(0, 0xFF) as u8,
            mask: self.mask,
        }
    }
}

#[derive(Clone, Copy)]
struct TexCoord {
    s: u8,
    t: u8,
}

impl TexCoord {
    #[inline(always)]
    fn from_16(coords: u16) -> Self {
        Self {
            s: (coords & 0xFF) as u8,
            t: ((coords >> 8) & 0xFF) as u8,
        }
    }
}

#[derive(Clone)]
struct Vertex {
    coord: Coord,
    col: Color,
    tex: TexCoord,
}

impl Vertex {
    #[inline(always)]
    fn from_xy(xy: u32) -> Self {
        Self {
            coord: Coord::from_xy(xy),
            col: Color::default(),
            tex: TexCoord { s: 0, t: 0 },
        }
    }

    #[inline(always)]
    fn set_col(mut self, rgb: u32) -> Self {
        self.col = Color::from_rgb24(rgb);
        self
    }

    #[inline(always)]
    fn set_tex(mut self, tex: u32) -> Self {
        self.tex = TexCoord::from_16(tex as u16);
        self
    }
}

struct PaletteCoord {
    x: usize,
    y: usize,
}

impl PaletteCoord {
    fn from_data(data: u32) -> Self {
        let palette = (data >> 16) as usize;
        Self {
            x: (palette & 0x3F) * 16,
            y: (palette >> 6) & 0x1FF,
        }
    }
}

struct TexInfo {
    s_base: usize,
    t_base: usize,
    tex_mode: TexMode,
    palette_coord: PaletteCoord,
}

impl TexInfo {
    fn from_data(texcoord_1: u32, texcoord_2: u32) -> Self {
        let palette = PaletteCoord::from_data(texcoord_1);
        let draw_mode = (texcoord_2 >> 16) as u16;
        Self::from_draw_mode(draw_mode, palette)
    }

    fn from_draw_mode(draw_mode: u16, palette: PaletteCoord) -> Self {
        let tex_mode = match (draw_mode >> 7) & 0x3 {
            0 => TexMode::Palette4,
            1 => TexMode::Palette8,
            _ => TexMode::Direct,
        };
        Self {
            s_base: ((draw_mode & 0x0F) as usize) << 6,
            t_base: ((draw_mode & 0x10) as usize) << 4,
            tex_mode,
            palette_coord: palette,
        }
    }
}

enum TexMode {
    /// 4 bpp with palette
    Palette4,
    /// 8 bpp with palette
    Palette8,
    /// RGB-15
    Direct,
}

#[derive(Clone, Copy)]
enum TransparencyMode {
    /// Base / 2 + Frag / 2
    Average,
    /// Base + Frag
    Add,
    /// Base - Frag
    Subtract,
    /// Base + (Frag / 4)
    Combine,
}

impl TransparencyMode {
    fn blend(&self, a: &Color, b: &Color) -> Color {
        use TransparencyMode::*;
        match self {
            Average => Color {
                r: (a.r >> 1) + (b.r >> 1),
                g: (a.g >> 1) + (b.g >> 1),
                b: (a.b >> 1) + (b.b >> 1),
                mask: a.mask, // ?
            },
            Add => Color {
                r: a.r.saturating_add(b.r),
                g: a.g.saturating_add(b.g),
                b: a.b.saturating_add(b.b),
                mask: a.mask, // ?
            },
            Subtract => Color {
                r: a.r.saturating_sub(b.r),
                g: a.g.saturating_sub(b.g),
                b: a.b.saturating_sub(b.b),
                mask: a.mask, // ?
            },
            Combine => Color {
                r: a.r.saturating_add(b.r >> 2),
                g: a.g.saturating_add(b.g >> 2),
                b: a.b.saturating_add(b.b >> 2),
                mask: a.mask, // ?
            }
        }
    }
}