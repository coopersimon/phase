use super::{
    RendererImpl, Coord, Size, Color, Vertex
};

use crate::{
    Frame, gpu::InterlaceState, mem::ram::RAM
};

struct DrawingArea {
    top: u16,
    bottom: u16,
    left: u16,
    right: u16,
}

/// Software implementation of rendering functions
/// for the PlayStation GPU.
pub struct SoftwareRenderer {
    vram: RAM,

    // Settings
    enable_display: bool,
    resolution: Size,
    frame_pos: Coord,
    drawing_area: DrawingArea,
    draw_offset: (i16, i16),
}

impl SoftwareRenderer {
    pub fn new() -> Self {
        Self {
            vram: RAM::new(super::VRAM_SIZE),

            enable_display: false,
            resolution: Size { width: 320, height: 240 },
            frame_pos: Coord { x: 0, y: 0 },
            drawing_area: DrawingArea { top: 0, bottom: 0, left: 0, right: 0 },
            draw_offset: (0, 0),
        }
    }
}

impl RendererImpl for SoftwareRenderer {
    fn get_frame(&mut self, frame: &mut Frame, interlace_state: InterlaceState) {
        if self.enable_display {
            println!("draw {:?} offset {}, {}", interlace_state, self.frame_pos.x, self.frame_pos.y);
            let line_size = (self.resolution.width as usize) * 4;
            for y in 0..240 {
                let mut frame_idx = match interlace_state {
                    InterlaceState::Off => y * line_size,
                    InterlaceState::Even => (y * 2) * line_size,
                    InterlaceState::Odd => (y * 2 + 1) * line_size,
                };
                let mut vram_addr = Coord {x: self.frame_pos.x, y: self.frame_pos.y + (y as u16)}.get_vram_idx() as u32;
                for _ in 0..self.resolution.width {
                    let pixel = self.vram.read_halfword(vram_addr);
                    let col = Color::from_rgb15(pixel);
                    frame.frame_buffer[frame_idx + 0] = col.r;
                    frame.frame_buffer[frame_idx + 1] = col.g;
                    frame.frame_buffer[frame_idx + 2] = col.b;
                    frame_idx += 4;
                    vram_addr += 2;
                }
            }
        } else {
            frame.frame_buffer.fill(0);
        }
    }
    fn write_vram_block(&mut self, data_in: &[u16], to: Coord, size: Size) {
        let mut data_idx = 0;
        for y in 0..size.height {
            // TODO: block copy.
            let mut addr = Coord {x: to.x, y: to.y + y}.get_vram_idx() as u32;
            for _ in 0..size.width {
                self.vram.write_halfword(addr, data_in[data_idx]);
                data_idx += 1;
                addr += 2;
            }
        }
    }
    fn read_vram_block(&mut self, data_out: &mut [u16], from: Coord, size: Size) {
        let mut data_idx = 0;
        for y in 0..size.height {
            // TODO: block copy.
            let mut addr = Coord {x: from.x, y: from.y + y}.get_vram_idx() as u32;
            for _ in 0..size.width {
                data_out[data_idx] = self.vram.read_halfword(addr);
                data_idx += 1;
                addr += 2;
            }
        }
    }
    fn copy_vram_block(&mut self, from: Coord, to: Coord, size: Size) {
        for line in 0..size.height {
            let mut src_addr = Coord {x: from.x, y: from.y + line}.get_vram_idx() as u32;
            let mut dst_addr = Coord {x: to.x, y: to.y + line}.get_vram_idx() as u32;
            //let copy_size = (size.width as usize) * 2;
            // TODO: use copy_within for block copy.
            for _ in 0..size.width {
                let pixel = self.vram.read_halfword(src_addr);
                self.vram.write_halfword(dst_addr, pixel);
                // TODO: wraparound...
                src_addr += 2;
                dst_addr += 2;
            }
        }
    }

    fn enable_display(&mut self, enable: bool) {
        self.enable_display = enable;
    }
    fn set_display_offset(&mut self, offset: Coord) {
        self.frame_pos = offset;
    }
    fn set_display_range_x(&mut self, begin: u32, end: u32) {
        
    }
    fn set_display_range_y(&mut self, begin: u32, end: u32) {
        
    }
    fn set_display_resolution(&mut self, res: Size) {
        self.resolution = res;
    }

    fn set_draw_area_top_left(&mut self, left: u16, top: u16) {
        self.drawing_area.left = left;
        self.drawing_area.top = top;
    }

    fn set_draw_area_bottom_right(&mut self, right: u16, bottom: u16) {
        self.drawing_area.right = right;
        self.drawing_area.bottom = bottom;
    }

    fn set_draw_area_offset(&mut self, x: i16, y: i16) {
        self.draw_offset.0 = x;
        self.draw_offset.1 = y;
    }

    fn set_mask_settings(&mut self, set_mask_bit: bool, check_mask_bit: bool) {
        // TODO: set
    }

    fn draw_triangle(&mut self, vertices: &[Vertex], transparent: bool) {
        let mut min_y = std::i16::MAX;
        let mut max_y = std::i16::MIN;
        let mut min_x = std::i16::MAX;
        let mut max_x = std::i16::MIN;
        println!("Draw triangle:");
        for v in vertices {
            println!("  {}, {}", v.x, v.y);
            min_y = min_y.min(v.y);
            max_y = max_y.max(v.y);
            min_x = min_x.min(v.x);
            max_x = max_x.max(v.x);
        }
        // For now just draw rectangle...
        let rgb = vertices[0].col.to_rgb15();
        for y in min_y..max_y {
            for x in min_x..max_x {
                let addr = Coord {x: x as u16, y: y as u16}.get_vram_idx() as u32;
                self.vram.write_halfword(addr, rgb);
            }
        }
    }
}
