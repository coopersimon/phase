use super::{
    RendererImpl, Coord, Size, Color, Vertex, TexInfo, TexMode, TexCoord, PaletteCoord
};

use crate::{
    Frame, gpu::InterlaceState
};

struct DrawingArea {
    top: u16,
    bottom: u16,
    left: u16,
    right: u16,
}

struct TextureWindow {
    mask_s: u8,
    mask_t: u8,
    offset_s: u8,
    offset_t: u8,
}

/// Software implementation of rendering functions
/// for the PlayStation GPU.
pub struct SoftwareRenderer {
    vram: Vec<u16>,

    // Settings
    enable_display: bool,
    resolution: Size,
    frame_pos: Coord,
    drawing_area: DrawingArea,
    draw_offset: (i16, i16),
    tex_window: TextureWindow,
}

impl SoftwareRenderer {
    pub fn new() -> Self {
        Self {
            vram: vec![0; super::VRAM_SIZE / 2],

            enable_display: false,
            resolution: Size { width: 320, height: 240 },
            frame_pos: Coord { x: 0, y: 0 },
            drawing_area: DrawingArea { top: 0, bottom: 0, left: 0, right: 0 },
            draw_offset: (0, 0),
            tex_window: TextureWindow { mask_s: 0, mask_t: 0, offset_s: 0, offset_t: 0 }
        }
    }
}

impl RendererImpl for SoftwareRenderer {
    fn get_frame(&mut self, frame: &mut Frame, interlace_state: InterlaceState) {
        if self.enable_display {
            let line_size = (self.resolution.width as usize) * 4;
            for y in 0..240 {
                let y = match interlace_state {
                    InterlaceState::Off => y,
                    InterlaceState::Even => y * 2,
                    InterlaceState::Odd => y * 2 + 1,
                };
                let mut frame_idx = y * line_size;
                let mut vram_addr = Coord {x: self.frame_pos.x, y: self.frame_pos.y + (y as u16)}.get_vram_idx();
                for _ in 0..self.resolution.width {
                    let pixel = self.vram[vram_addr];
                    let col = Color::from_rgb15(pixel);
                    frame.frame_buffer[frame_idx + 0] = col.r;
                    frame.frame_buffer[frame_idx + 1] = col.g;
                    frame.frame_buffer[frame_idx + 2] = col.b;
                    frame_idx += 4;
                    vram_addr += 1;
                }
            }
        } else {
            frame.frame_buffer.fill(0);
        }
    }
    fn write_vram_block(&mut self, data_in: &[u16], to: Coord, size: Size) {
        for y in 0..size.height {
            let src_begin = (y * size.width) as usize;
            let src_end = src_begin + (size.width as usize);
            let dst_begin = Coord {x: to.x, y: to.y + y}.get_vram_idx();
            let dst_end = dst_begin + (size.width as usize);
            let dest = &mut self.vram[dst_begin..dst_end];
            dest.copy_from_slice(&data_in[src_begin..src_end]);
        }
    }
    fn read_vram_block(&mut self, data_out: &mut [u16], from: Coord, size: Size) {
        for y in 0..size.height {
            let dst_begin = (y * size.width) as usize;
            let dst_end = dst_begin + (size.width as usize);
            let src_begin = Coord {x: from.x, y: from.y + y}.get_vram_idx();
            let src_end = src_begin + (size.width as usize);
            let dest = &mut data_out[dst_begin..dst_end];
            dest.copy_from_slice(&self.vram[src_begin..src_end]);
        }
    }
    fn copy_vram_block(&mut self, from: Coord, to: Coord, size: Size) {
        for line in 0..size.height {
            let src_begin = Coord {x: from.x, y: from.y + line}.get_vram_idx();
            let src_end = src_begin + (size.width as usize);
            let dest = Coord {x: to.x, y: to.y + line}.get_vram_idx();
            self.vram.copy_within(src_begin..src_end, dest);
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

    fn set_texture_window(&mut self, mask_s: u8, mask_t: u8, offset_s: u8, offset_t: u8) {
        self.tex_window.mask_s = mask_s;
        self.tex_window.mask_t = mask_t;
        self.tex_window.offset_s = offset_s;
        self.tex_window.offset_t = offset_t;
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

    fn fill_rectangle(&mut self, color: Color, top_left: Coord, size: Size) {
        let rgb15 = color.to_rgb15();
        for y in 0..size.height {
            let dst_begin = Coord {x: top_left.x, y: top_left.y + y}.get_vram_idx();
            let dst_end = dst_begin + (size.width as usize);
            let dest = &mut self.vram[dst_begin..dst_end];
            dest.fill(rgb15);
        }
    }

    fn draw_triangle(&mut self, vertices: &[Vertex], transparent: bool) {
        self.rasterize_triangle(vertices, |_: &Self, line: &Line| {
            Some(line.get_color())
        });
    }

    fn draw_triangle_tex(&mut self, vertices: &[Vertex], tex_info: &TexInfo, transparent: bool) {
        self.rasterize_triangle(vertices, |renderer: &Self, line: &Line| {
            let tex_color = renderer.tex_lookup(&line.get_tex_coords(), tex_info);
            if tex_color == 0 {
                None
            } else {
                let frag_color = line.get_color();
                Some(frag_color.blend(&Color::from_rgb15(tex_color)))
            }
        });
    }

    fn draw_rectangle(&mut self, color: Color, top_left: Coord, size: Size, transparent: bool) {
        let top = top_left.y;
        let bottom = top + size.height;
        let left = top_left.x;
        let right = left + size.width;
        for y in top..bottom {
            let line_addr = (y as usize) * 1024;
            for x in left..right {
                let addr = line_addr + (x as usize);
                self.vram[addr] = color.to_rgb15();
            }
        }
    }

    fn draw_rectangle_tex(&mut self, color: Color, tex_coord: TexCoord, tex_info: &TexInfo, top_left: Coord, size: Size, transparent: bool) {
        let top = top_left.y;
        let bottom = top + size.height;
        let left = top_left.x;
        let right = left + size.width;
        let mut current_tex_coord = tex_coord;
        for y in top..bottom {
            let line_addr = (y as usize) * 1024;
            for x in left..right {
                let tex_color = self.tex_lookup(&current_tex_coord, tex_info);
                if tex_color != 0 {
                    let addr = line_addr + (x as usize);
                    self.vram[addr] = color.blend(&Color::from_rgb15(tex_color)).to_rgb15();
                }
                current_tex_coord.s += 1;
            }
            current_tex_coord.t += 1;
            current_tex_coord.s = tex_coord.s;
        }
    }
}

// Internal
impl SoftwareRenderer {
    fn rasterize_triangle<F: Fn(&Self, &Line) -> Option<Color>>(&mut self, vertices: &[Vertex], raster_f: F) {
        let mut min_y = std::u16::MAX;
        let mut max_y = std::u16::MIN;
        //println!("Draw triangle:");
        for v in vertices {
            //println!("  {}, {}", v.coord.x, v.coord.y);
            min_y = min_y.min(v.coord.y);
            max_y = max_y.max(v.coord.y);
        }
        let Some(mut lines) = Self::get_intersection_points(vertices, min_y) else {
            panic!("no intersection points found"); // TODO: just return?
            return;
        };
        //println!("Line 0 (tex) {:X},{:X} => {:X},{:X}", lines.left.tex_s, lines.left.tex_t, lines.right.tex_s, lines.right.tex_t);
        //println!("Line 0 (rgb) {:X},{:X},{:X}", lines.left.r_gradient, lines.left.b_gradient, lines.left.g_gradient);
        // TODO: clip at view bounds
        self.draw_lines(min_y, &mut lines, &raster_f);
        min_y = lines.max_y;
        if min_y > self.drawing_area.bottom {
            return;
        }
        // TODO: validate that we get more if min_y < max_y
        // Also: we kind of want to _replace_ one of our lines (either left or right?)
        if let Some(mut lines) = Self::get_intersection_points(vertices, min_y) {
            // Continue drawing.
            self.draw_lines(min_y, &mut lines, &raster_f);
        };
    }

    fn draw_lines<F: Fn(&Self, &Line) -> Option<Color>>(&mut self, min_y: u16, lines: &mut Lines, raster_f: &F) {
        let max_y = lines.max_y.min(self.drawing_area.bottom);
        for y in min_y..max_y {
            let left = lines.left.get_x();
            let right = lines.right.get_x();
            if left != right {
                let line_addr = (y as usize) * 1024;
                let mut line = Line::from_lines(&lines.left, &lines.right);
                for x in left..right {
                    if let Some(col) = raster_f(self, &line) {
                        let addr = line_addr + (x as usize);
                        self.vram[addr] = col.to_rgb15();
                    }
                    line.inc();
                }
            }
            lines.left.inc();
            lines.right.inc();
        }
    }

    fn get_intersection_points(vertices: &[Vertex], line: u16) -> Option<Lines> {
        let mut left: Option<Line> = None;
        let mut right: Option<Line> = None;
        let mut max_y = u16::MAX;
        for i in 0..3 {
            let vertex_a = &vertices[i];
            let vertex_b = &vertices[(i + 1) % 3];
            // (0,0) is TOP-LEFT.
            let (top, bottom) = if vertex_a.coord.y > vertex_b.coord.y {
                (vertex_b, vertex_a)
            } else {
                (vertex_a, vertex_b)
            };
            if top.coord.y == bottom.coord.y || top.coord.y > line || bottom.coord.y <= line {
                continue;
            }
            
            max_y = max_y.min(bottom.coord.y);
            let line = Line::from_vertices(top, bottom);
            if let Some(other_line) = left.take() {
                if line.x < other_line.x {
                    right = Some(other_line);
                    left = Some(line);
                } else if line.x > other_line.x {
                    left = Some(other_line);
                    right = Some(line);
                } else { // equal
                    if line.x_gradient < other_line.x_gradient {
                        right = Some(other_line);
                        left = Some(line);
                    } else {
                        left = Some(other_line);
                        right = Some(line);
                    }
                }
            } else {
                left = Some(line);
            }
        }
        if left.is_some() && right.is_some() {
            Some(Lines {
                left: left.unwrap(),
                right: right.unwrap(),
                max_y,
            })
        } else {
            None
        }
    }

    fn tex_lookup(&self, tex_coords: &TexCoord, tex_info: &TexInfo) -> u16 {
        let tex_s = (tex_coords.s & !self.tex_window.mask_s) | (self.tex_window.mask_s & self.tex_window.offset_s);
        let tex_t = (tex_coords.t & !self.tex_window.mask_t) | (self.tex_window.mask_t & self.tex_window.offset_t);
        let t = tex_t as usize + tex_info.t_base;
        match tex_info.tex_mode {
            TexMode::Palette4 => {
                let s = (tex_s as usize / 4) + tex_info.s_base;
                let tex_addr = t * 1024 + s;
                let data = self.vram[tex_addr];
                let palette_shift = (tex_s & 0x3) * 4;
                let palette_idx = ((data >> palette_shift) & 0xF) as usize;
                //println!("s: {:X} t: {:X} addr: {:X} idx: {:X}", line.tex_s, line.tex_t, tex_addr, palette_idx);
                let palette_addr = tex_info.palette_coord.y * 1024 + tex_info.palette_coord.x + palette_idx;
                self.vram[palette_addr]
            },
            TexMode::Palette8 => {
                let s = (tex_s as usize / 2) + tex_info.s_base;
                let tex_addr = t * 1024 + s;
                let data = self.vram[tex_addr];
                let palette_shift = (tex_s & 0x1) * 8;
                let palette_idx = ((data >> palette_shift) & 0xFF) as usize;
                let palette_addr = tex_info.palette_coord.y * 1024 + tex_info.palette_coord.x + palette_idx;
                self.vram[palette_addr]
            },
            TexMode::Direct => {
                let s = tex_s as usize + tex_info.s_base;
                let tex_addr = t * 1024 + s;
                self.vram[tex_addr]
            }
        }
    }
}

struct Line {
    // Gradient is a fixed-point factor.
    // 16 i bits and 16 f bits.
    x_gradient: i32,
    x: i32,
    r_gradient: i32,
    r: i32,
    g_gradient: i32,
    g: i32,
    b_gradient: i32,
    b: i32,
    tex_s_gradient: i32,
    tex_s: i32,
    tex_t_gradient: i32,
    tex_t: i32,
}

impl Line {
    fn from_vertices(top: &Vertex, bottom: &Vertex) -> Self {
        let gradient = (1 << 16) / ((bottom.coord.y - top.coord.y) as i32);
        let tex_s_diff = bottom.tex.s as i32 - top.tex.s as i32;
        let tex_t_diff = bottom.tex.t as i32 - top.tex.t as i32;
        Self {
            x_gradient: gradient * (bottom.coord.x as i32 - top.coord.x as i32),
            x: (top.coord.x as i32) << 16,
            r_gradient: gradient * (bottom.col.r as i32 - top.col.r as i32),
            r: (top.col.r as i32) << 16,
            g_gradient: gradient * (bottom.col.g as i32 - top.col.g as i32),
            g: (top.col.g as i32) << 16,
            b_gradient: gradient * (bottom.col.b as i32 - top.col.b as i32),
            b: (top.col.b as i32) << 16,
            tex_s_gradient: gradient * tex_s_diff,
            tex_s: (top.tex.s as i32) << 16,
            tex_t_gradient: gradient * tex_t_diff,
            tex_t: (top.tex.t as i32) << 16,
        }
    }

    fn from_lines(left: &Line, right: &Line) -> Self {
        let gradient = (1 << 16) / ((right.get_x() - left.get_x()) as i32);
        let tex_s_diff = (right.tex_s - left.tex_s) >> 16;
        let tex_t_diff = (right.tex_t - left.tex_t) >> 16;
        Self {
            x_gradient: 0,
            x: 0,
            r_gradient: gradient * ((right.r - left.r) >> 16),
            r: left.r,
            g_gradient: gradient * ((right.g - left.g) >> 16),
            g: left.g,
            b_gradient: gradient * ((right.b - left.b) >> 16),
            b: left.b,
            tex_s_gradient: gradient * tex_s_diff,
            tex_s: left.tex_s,
            tex_t_gradient: gradient * tex_t_diff,
            tex_t: left.tex_t,
        }
    }

    fn get_x(&self) -> u16 {
        ((self.x + 0x8000) >> 16) as u16
    }
    fn get_color(&self) -> Color {
        Color {
            r: (self.r >> 16) as u8,
            g: (self.g >> 16) as u8,
            b: (self.b >> 16) as u8,
        }
    }
    fn get_tex_coords(&self) -> TexCoord {
        let s = ((self.tex_s + 0x8000) >> 16) as u8;
        let t = ((self.tex_t + 0x8000) >> 16) as u8;
        TexCoord { s, t }
    }

    /// Advance internal state.
    fn inc(&mut self) {
        self.x += self.x_gradient;
        self.r += self.r_gradient;
        self.g += self.g_gradient;
        self.b += self.b_gradient;
        self.tex_s += self.tex_s_gradient;
        self.tex_t += self.tex_t_gradient;
    }
}

struct Lines {
    left: Line,
    right: Line,
    max_y: u16,
}