use super::{
    RendererImpl, Coord, Size, Color, Vertex, TexInfo, TexMode, TexCoord, TransparencyMode
};

use crate::{
    Frame, gpu::InterlaceState, utils::bits::*
};

struct DrawingArea {
    top: i16,
    bottom: i16,
    left: i16,
    right: i16,
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
    draw_offset: Coord,
    tex_window: TextureWindow,
    interlace: bool,
    display_height: u16,

    trans_mode: TransparencyMode,
    dither: bool,
    set_mask_bit: bool,
    check_mask_bit: bool,
}

impl SoftwareRenderer {
    pub fn new() -> Self {
        Self {
            vram: vec![0; super::VRAM_SIZE / 2],

            enable_display: false,
            resolution: Size { width: 320, height: 240 },
            frame_pos: Coord { x: 0, y: 0 },
            drawing_area: DrawingArea { top: 0, bottom: 0, left: 0, right: 0 },
            draw_offset: Coord { x: 0, y: 0 },
            tex_window: TextureWindow { mask_s: 0, mask_t: 0, offset_s: 0, offset_t: 0 },
            interlace: false,
            display_height: 240,

            trans_mode: TransparencyMode::Average,
            dither: false,
            set_mask_bit: false,
            check_mask_bit: false,
        }
    }
}

impl RendererImpl for SoftwareRenderer {
    fn get_frame(&mut self, frame: &mut Frame, _interlace_state: InterlaceState, rgb24: bool, debug: bool) {
        let width = if debug {1024} else {self.resolution.width as usize};
        let height = if debug {512} else {
            let display_height = if self.interlace {self.display_height * 2} else {self.display_height};
            self.resolution.height.min(display_height) as usize
        };
        if self.enable_display {
            let line_size = width * 4;
            for y in 0..height {
                // TODO: interlaced rendering makes a cool effect, but overflows memory sometimes
                // for as of yet unknown reasons.
                /*let y = match interlace_state {
                    InterlaceState::Off => y,
                    InterlaceState::Even => y * 2,
                    InterlaceState::Odd => y * 2 + 1,
                };*/
                let mut frame_idx = y * line_size;
                let mut vram_addr = Coord {x: self.frame_pos.x, y: self.frame_pos.y + (y as i16)}.get_vram_idx();
                if rgb24 {
                    if debug {
                        vram_addr = y * (width / 2);
                    }
                    for _ in 0..(width / 2) {
                        let h0 = self.vram[vram_addr + 0];
                        let h1 = self.vram[vram_addr + 1];
                        let h2 = self.vram[vram_addr + 2];
                        frame.frame_buffer[frame_idx + 0] = h0 as u8;
                        frame.frame_buffer[frame_idx + 1] = (h0 >> 8) as u8;
                        frame.frame_buffer[frame_idx + 2] = h1 as u8;
                        frame.frame_buffer[frame_idx + 4] = (h1 >> 8) as u8;
                        frame.frame_buffer[frame_idx + 5] = h2 as u8;
                        frame.frame_buffer[frame_idx + 6] = (h2 >> 8) as u8;
                        vram_addr += 3;
                        frame_idx += 8;
                    }
                } else {
                    if debug {
                        vram_addr = y * width;
                    }
                    for _ in 0..width {
                        let pixel = self.vram[vram_addr];
                        let col = Color::from_rgb15(pixel);
                        frame.frame_buffer[frame_idx + 0] = col.r;
                        frame.frame_buffer[frame_idx + 1] = col.g;
                        frame.frame_buffer[frame_idx + 2] = col.b;
                        vram_addr += 1;
                        frame_idx += 4;
                    }
                }
            }
        } else {
            frame.frame_buffer.fill(0);
        }
    }
    fn write_vram_block(&mut self, data_in: &[u16], to: Coord, size: Size) {
        let mask = if self.set_mask_bit {0x8000} else {0};
        for y in 0..size.height {
            let src_begin = (y * size.width) as usize;
            let src_end = src_begin + (size.width as usize);
            let addr_base = (to.y + y as i16) as usize * 1024;
            for (x, pixel) in data_in[src_begin..src_end].iter().cloned().enumerate() {
                let x_addr = ((to.x as usize) + x) % 1024;
                let addr = addr_base + x_addr;
                if !self.check_mask_bit || !test_bit!(self.vram[addr], 15) {
                    self.vram[addr] = pixel | mask;
                }
            }
        }
    }
    fn read_vram_block(&mut self, data_out: &mut [u16], from: Coord, size: Size) {
        if from.x + size.width as i16 > 1024 {
            panic!("copy {:X} from {:X}", size.width, from.x);
        }
        for y in 0..size.height {
            let dst_begin = (y * size.width) as usize;
            let dst_end = dst_begin + (size.width as usize);
            let src_begin = Coord {x: from.x, y: from.y + y as i16}.get_vram_idx();
            let src_end = src_begin + (size.width as usize);
            let dest = &mut data_out[dst_begin..dst_end];
            dest.copy_from_slice(&self.vram[src_begin..src_end]);
        }
    }
    fn copy_vram_block(&mut self, from: Coord, to: Coord, size: Size) {
        let mask = if self.set_mask_bit {0x8000} else {0};
        for y in 0..size.height {
            let read_addr_base = (from.y + y as i16) as usize * 1024;
            let write_addr_base = (to.y + y as i16) as usize * 1024;
            for x in 0..size.width {
                let read_x_addr = ((from.x + x as i16) as usize) % 1024;
                let write_x_addr = ((to.x + x as i16) as usize) % 1024;
                let read_addr = read_addr_base + read_x_addr;
                let write_addr = write_addr_base + write_x_addr;
                let data = self.vram[read_addr];
                if !self.check_mask_bit || !test_bit!(data, 15) {
                    self.vram[write_addr] = data | mask;
                }
            }
        }
    }

    fn enable_display(&mut self, enable: bool) {
        self.enable_display = enable;
    }
    fn set_display_offset(&mut self, offset: Coord) {
        self.frame_pos = offset;
    }
    fn set_display_range_x(&mut self, _begin: u32, _end: u32) {
        //println!("Display X: {} => {}", begin, end);
    }
    fn set_display_range_y(&mut self, begin: u32, end: u32) {
        //println!("Display Y: {} => {}", begin, end);
        self.display_height = (end - begin) as u16;
    }
    fn set_display_resolution(&mut self, res: Size, interlace: bool) {
        self.resolution = res;
        self.interlace = interlace;
    }

    fn set_draw_mode(&mut self, trans_mode: super::TransparencyMode, dither: bool) {
        self.trans_mode = trans_mode;
        self.dither = dither;
    }

    fn set_texture_window(&mut self, mask_s: u8, mask_t: u8, offset_s: u8, offset_t: u8) {
        self.tex_window.mask_s = mask_s;
        self.tex_window.mask_t = mask_t;
        self.tex_window.offset_s = offset_s;
        self.tex_window.offset_t = offset_t;
    }

    fn set_draw_area_top_left(&mut self, left: i16, top: i16) {
        self.drawing_area.left = left;
        self.drawing_area.top = top;
    }

    fn set_draw_area_bottom_right(&mut self, right: i16, bottom: i16) {
        self.drawing_area.right = right + 1;
        self.drawing_area.bottom = bottom + 1;
    }

    fn set_draw_area_offset(&mut self, x: i16, y: i16) {
        self.draw_offset.x = x;
        self.draw_offset.y = y;
    }

    fn set_mask_settings(&mut self, set_mask_bit: bool, check_mask_bit: bool) {
        self.set_mask_bit = set_mask_bit;
        self.check_mask_bit = check_mask_bit;
    }

    fn fill_rectangle(&mut self, color: Color, top_left: Coord, size: Size) {
        let rgb15 = color.to_rgb15();
        for y in 0..size.height {
            let dst_begin = Coord {x: top_left.x, y: top_left.y + y as i16}.get_vram_idx();
            let dst_end = dst_begin + (size.width as usize);
            let dest = &mut self.vram[dst_begin..dst_end];
            dest.fill(rgb15);
        }
    }

    fn draw_triangle(&mut self, vertices: &[Vertex], transparent: bool) {
        self.rasterize_triangle(vertices, |renderer: &mut Self, line: &Line, addr: usize| {
            let color = line.get_color();
            renderer.write_pixel(addr, &color, transparent);
        });
    }

    fn draw_triangle_tex(&mut self, vertices: &[Vertex], tex_info: &TexInfo, transparent: bool) {
        self.rasterize_triangle(vertices, |renderer: &mut Self, line: &Line, addr: usize| {
            let tex_color = renderer.tex_lookup(&line.get_tex_coords(), tex_info);
            if tex_color != 0 {
                let color = line.get_color();
                let frag_color = color.blend(&Color::from_rgb15(tex_color), !renderer.set_mask_bit);
                let transparent = transparent && test_bit!(tex_color, 15);
                renderer.write_pixel(addr, &frag_color, transparent);
            }
        });
    }

    fn draw_rectangle(&mut self, color: Color, top_left: Coord, size: Size, transparent: bool) {
        let top = top_left.y + self.draw_offset.y;
        let bottom = top + size.height as i16;
        let left = top_left.x + self.draw_offset.x;
        let right = left + size.width as i16;
        let y_min = top.max(self.drawing_area.top);
        let y_max = bottom.min(self.drawing_area.bottom);
        let x_min = left.max(self.drawing_area.left);
        let x_max = right.min(self.drawing_area.right);
        for y in y_min..y_max {
            let line_addr = (y as usize) * 1024;
            for x in x_min..x_max {
                let addr = line_addr + (x as usize);
                self.write_pixel(addr, &color, transparent);
            }
        }
    }

    fn draw_rectangle_tex(&mut self, color: Color, tex_coord: TexCoord, tex_info: &TexInfo, top_left: Coord, size: Size, transparent: bool) {
        let top = top_left.y + self.draw_offset.y;
        let bottom = top + size.height as i16;
        let left = top_left.x + self.draw_offset.x;
        let right = left + size.width as i16;
        let y_min = top.max(self.drawing_area.top);
        let y_max = bottom.min(self.drawing_area.bottom);
        let x_min = left.max(self.drawing_area.left);
        let x_max = right.min(self.drawing_area.right);
        let start_coord = TexCoord {
            s: if left < self.drawing_area.left {
                let offset = self.drawing_area.left - left;
                ((tex_coord.s as u16 as i16) + offset) as u8
            } else {
                tex_coord.s
            },
            t: if top < self.drawing_area.top {
                let offset = self.drawing_area.top - top;
                ((tex_coord.t as u16 as i16) + offset) as u8
            } else {
                tex_coord.t
            },
        };
        let mut current_tex_coord = start_coord;
        for y in y_min..y_max {
            let line_addr = (y as usize) * 1024;
            for x in x_min..x_max {
                let tex_color = self.tex_lookup(&current_tex_coord, tex_info);
                if tex_color != 0 {
                    let addr = line_addr + (x as usize);
                    let frag_color = color.blend(&Color::from_rgb15(tex_color), !self.set_mask_bit);
                    let transparent = transparent && test_bit!(tex_color, 15);
                    self.write_pixel(addr, &frag_color, transparent);
                }
                current_tex_coord.s += 1;
            }
            current_tex_coord.t += 1;
            current_tex_coord.s = start_coord.s;
        }
    }

    fn draw_line(&mut self, vertex_a: &Vertex, vertex_b: &Vertex, transparent: bool) {
        // X or Y major line:
        let dx = (vertex_a.coord.x - vertex_b.coord.x).abs();
        let dy = (vertex_a.coord.y - vertex_b.coord.y).abs();
        if dx > dy {
            let (left, right) = if vertex_a.coord.x < vertex_b.coord.x {
                (vertex_a, vertex_b)
            } else {
                (vertex_b, vertex_a)
            };
            let mut color = InterpolatedColor::from_x_major(left, right);
            let y_step = if left.coord.y < right.coord.y {1} else {-1};
            let x_begin = left.coord.x + self.draw_offset.x;
            let x_end = (right.coord.x + self.draw_offset.x).min(self.drawing_area.right - 1);
            let mut y = left.coord.y + self.draw_offset.y;
            let mut delta = 2 * dy - dx;
            for x in x_begin..=x_end {
                if x >= self.drawing_area.left &&
                    y >= self.drawing_area.top && y < self.drawing_area.bottom {
                    let addr = (y as usize) * 1024 + (x as usize);
                    self.write_pixel(addr, &color.get(), transparent);
                }
                if delta > 0 {
                    y += y_step;
                    delta += 2 * (dy - dx);
                } else {
                    delta += 2 * dy;
                }
                color.inc();
            }
        } else {
            let (top, bottom) = if vertex_a.coord.y < vertex_b.coord.y {
                (vertex_a, vertex_b)
            } else {
                (vertex_b, vertex_a)
            };
            let mut color = InterpolatedColor::from_y_major(top, bottom);
            let x_step = if top.coord.x < bottom.coord.x {1} else {-1};
            let y_begin = top.coord.y + self.draw_offset.y;
            let y_end = (bottom.coord.y + self.draw_offset.y).min(self.drawing_area.bottom - 1);
            let mut x = top.coord.x + self.draw_offset.x;
            let mut delta = 2 * dx - dy;
            for y in y_begin..=y_end {
                if y >= self.drawing_area.top &&
                    x >= self.drawing_area.left && x < self.drawing_area.right {
                    let addr = (y as usize) * 1024 + (x as usize);
                    self.write_pixel(addr, &color.get(), transparent);
                }
                if delta > 0 {
                    x += x_step;
                    delta += 2 * (dx - dy);
                } else {
                    delta += 2 * dx;
                }
                color.inc();
            }
        }
    }
}

// Internal
impl SoftwareRenderer {
    #[inline(always)]
    fn write_pixel(&mut self, addr: usize, color: &Color, transparent: bool) {
        if !self.check_mask_bit || !test_bit!(self.vram[addr], 15) {
            let color = if transparent {
                let base_color = Color::from_rgb15(self.vram[addr]);
                self.trans_mode.blend(&base_color, color)
            } else {
                *color
            };
            self.vram[addr] = color.to_rgb15();
        }
    }

    fn rasterize_triangle<F: Fn(&mut Self, &Line, usize)>(&mut self, vertices: &[Vertex], raster_f: F) {
        let mut min_y = std::i16::MAX;
        let mut max_y = std::i16::MIN;
        //println!("Draw triangle: offset: ({}, {}) bounds: ({},{}) => ({},{})", self.draw_offset.x, self.draw_offset.y, self.drawing_area.left, self.drawing_area.top, self.drawing_area.right, self.drawing_area.bottom);
        for v in vertices {
            //println!("  {}, {} TEX: {}, {}", v.coord.x, v.coord.y, v.tex.s, v.tex.t);
            min_y = min_y.min(v.coord.y);
            max_y = max_y.max(v.coord.y);
        }
        //min_y = min_y.max(self.drawing_area.top);
        //max_y = max_y.min(self.drawing_area.bottom);
        let Some(mut lines) = Self::get_intersection_points(vertices, min_y) else {
            //println!("No intersection points found");
            return;
        };
        //println!("Line 0 (tex) {:X},{:X} => {:X},{:X}", lines.left.tex_s, lines.left.tex_t, lines.right.tex_s, lines.right.tex_t);
        //println!("Line 0 (rgb) {:X},{:X},{:X}", lines.left.r_gradient, lines.left.b_gradient, lines.left.g_gradient);
        // TODO: clip at view bounds
        self.draw_lines(min_y, &mut lines, &raster_f);
        min_y = lines.max_y;
        /*if min_y > self.drawing_area.bottom {
            return;
        }*/
        // TODO: validate that we get more if min_y < max_y
        // Also: we kind of want to _replace_ one of our lines (either left or right?)
        if let Some(mut lines) = Self::get_intersection_points(vertices, min_y) {
            // Continue drawing.
            self.draw_lines(min_y, &mut lines, &raster_f);
        };
    }

    fn draw_lines<F: Fn(&mut Self, &Line, usize)>(&mut self, min_y: i16, lines: &mut Lines, raster_f: &F) {
        let base_y = min_y + self.draw_offset.y;
        if base_y < self.drawing_area.top {
            lines.left.mul((self.drawing_area.top - base_y) as i32);
            lines.right.mul((self.drawing_area.top - base_y) as i32);
        }
        let min_y = base_y.max(self.drawing_area.top);
        let max_y = (lines.max_y + self.draw_offset.y).min(self.drawing_area.bottom);
        for y in min_y..max_y {
            let left = lines.left.get_x() + self.draw_offset.x;
            let right = lines.right.get_x() + self.draw_offset.x;
            let min_x = left.max(self.drawing_area.left);
            let max_x = right.min(self.drawing_area.right);
            if lines.left.get_x() != lines.right.get_x() {
                let line_addr = (y as usize) * 1024;
                let mut line = Line::from_lines(&lines.left, &lines.right, lines.left.get_x());
                if left < self.drawing_area.left {
                    line.mul((self.drawing_area.left - left) as i32);
                }
                for x in min_x..max_x {
                    let addr = line_addr + (x as usize);
                    raster_f(self, &line, addr);
                    line.inc();
                }
            }
            lines.left.inc();
            lines.right.inc();
        }
    }

    fn get_intersection_points(vertices: &[Vertex], line: i16) -> Option<Lines> {
        let mut left: Option<Line> = None;
        let mut right: Option<Line> = None;
        let mut max_y = i16::MAX;
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
            let line = Line::from_vertices(top, bottom, line);
            if let Some(other_line) = left.take() {
                if line.x.val < other_line.x.val {
                    right = Some(other_line);
                    left = Some(line);
                } else if line.x.val > other_line.x.val {
                    left = Some(other_line);
                    right = Some(line);
                } else { // equal
                    if line.x.gradient < other_line.x.gradient {
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

#[derive(Default)]
struct InterpolatedValue {
    // Gradient is a fixed-point factor.
    // 16 i bits and 16 f bits.
    gradient: i32,
    val: i32,
}

impl InterpolatedValue {
    fn new(gradient: i32, a: i32, b: i32, i: i32) -> Self {
        let gradient = gradient * (b - a);
        Self {
            gradient,
            val: (a << 16) + i * gradient,
        }
    }

    fn new_shifted(gradient: i32, a: i32, b: i32, i: i32) -> Self {
        let gradient = gradient * ((b - a) >> 16);
        Self {
            gradient,
            val: a + i * gradient,
        }
    }

    fn inc(&mut self) {
        self.val += self.gradient;
    }

    /// Inc i times.
    fn mul(&mut self, i: i32) {
        self.val += i * self.gradient;
    }
}

struct Line {
    x: InterpolatedValue,
    r: InterpolatedValue,
    g: InterpolatedValue,
    b: InterpolatedValue,
    tex_s: InterpolatedValue,
    tex_t: InterpolatedValue,
}

impl Line {
    fn from_vertices(top: &Vertex, bottom: &Vertex, line: i16) -> Self {
        let gradient = (1 << 16) / ((bottom.coord.y - top.coord.y) as i32);
        let i = (line - top.coord.y) as i32;
        Self {
            x: InterpolatedValue::new(gradient, top.coord.x.into(), bottom.coord.x.into(), i),
            r: InterpolatedValue::new(gradient, top.col.r.into(), bottom.col.r.into(), i),
            g: InterpolatedValue::new(gradient, top.col.g.into(), bottom.col.g.into(), i),
            b: InterpolatedValue::new(gradient, top.col.b.into(), bottom.col.b.into(), i),
            tex_s: InterpolatedValue::new(gradient, top.tex.s.into(), bottom.tex.s.into(), i),
            tex_t: InterpolatedValue::new(gradient, top.tex.t.into(), bottom.tex.t.into(), i),
        }
    }

    fn from_lines(left: &Line, right: &Line, x: i16) -> Self {
        let left_x = left.get_x();
        let right_x = right.get_x();
        let gradient = (1 << 16) / ((right_x - left_x) as i32);
        let i = (x - left_x) as i32;
        Self {
            x: InterpolatedValue::default(),
            r: InterpolatedValue::new_shifted(gradient, left.r.val, right.r.val, i),
            g: InterpolatedValue::new_shifted(gradient, left.g.val, right.g.val, i),
            b: InterpolatedValue::new_shifted(gradient, left.b.val, right.b.val, i),
            tex_s: InterpolatedValue::new_shifted(gradient, left.tex_s.val, right.tex_s.val, i),
            tex_t: InterpolatedValue::new_shifted(gradient, left.tex_t.val, right.tex_t.val, i),
        }
    }

    fn get_x(&self) -> i16 {
        ((self.x.val + 0x8000) >> 16) as i16
    }
    fn get_color(&self) -> Color {
        Color {
            r: ((self.r.val + 0x8000) >> 16) as u8,
            g: ((self.g.val + 0x8000) >> 16) as u8,
            b: ((self.b.val + 0x8000) >> 16) as u8,
            mask: 0,
        }
    }
    fn get_tex_coords(&self) -> TexCoord {
        let s = ((self.tex_s.val + 0x8000) >> 16) as u8;
        let t = ((self.tex_t.val + 0x8000) >> 16) as u8;
        TexCoord { s, t }
    }

    /// Advance internal state.
    fn inc(&mut self) {
        self.x.inc();
        self.r.inc();
        self.g.inc();
        self.b.inc();
        self.tex_s.inc();
        self.tex_t.inc();
    }

    /// Inc i times.
    fn mul(&mut self, i: i32) {
        self.x.mul(i);
        self.r.mul(i);
        self.g.mul(i);
        self.b.mul(i);
        self.tex_s.mul(i);
        self.tex_t.mul(i);
    }
}

struct Lines {
    left: Line,
    right: Line,
    max_y: i16,
}

struct InterpolatedColor {
    r: InterpolatedValue,
    g: InterpolatedValue,
    b: InterpolatedValue,
}

impl InterpolatedColor {
    fn from_x_major(left: &Vertex, right: &Vertex) -> Self {
        let gradient = (1_i32 << 16).checked_div((right.coord.x - left.coord.x) as i32).unwrap_or(0);
        Self {
            r: InterpolatedValue::new(gradient, left.col.r.into(), right.col.r.into(), 0),
            g: InterpolatedValue::new(gradient, left.col.g.into(), right.col.g.into(), 0),
            b: InterpolatedValue::new(gradient, left.col.b.into(), right.col.b.into(), 0),
        }
    }

    fn from_y_major(top: &Vertex, bottom: &Vertex) -> Self {
        let gradient = (1_i32 << 16).checked_div((bottom.coord.y - top.coord.y) as i32).unwrap_or(0);
        Self {
            r: InterpolatedValue::new(gradient, top.col.r.into(), bottom.col.r.into(), 0),
            g: InterpolatedValue::new(gradient, top.col.g.into(), bottom.col.g.into(), 0),
            b: InterpolatedValue::new(gradient, top.col.b.into(), bottom.col.b.into(), 0),
        }
    }

    fn get(&self) -> Color {
        Color {
            r: ((self.r.val + 0x8000) >> 16) as u8,
            g: ((self.g.val + 0x8000) >> 16) as u8,
            b: ((self.b.val + 0x8000) >> 16) as u8,
            mask: 0,
        }
    }

    /// Advance internal state.
    fn inc(&mut self) {
        self.r.inc();
        self.g.inc();
        self.b.inc();
    }
}