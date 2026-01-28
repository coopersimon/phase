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
                let y = match interlace_state {
                    InterlaceState::Off => y,
                    InterlaceState::Even => y * 2,
                    InterlaceState::Odd => y * 2 + 1,
                };
                let mut frame_idx = y * line_size;
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
        let mut min_y = std::u16::MAX;
        let mut max_y = std::u16::MIN;
        println!("Draw triangle:");
        for v in vertices {
            println!("  {}, {}", v.coord.x, v.coord.y);
            min_y = min_y.min(v.coord.y);
            max_y = max_y.max(v.coord.y);
        }
        let Some(mut lines) = Self::get_intersection_points(vertices, min_y) else {
            panic!("no intersection points found"); // TODO: just return?
            return;
        };
        for y in min_y..lines.max_y {
            let left = lines.left.get_x();
            let right = lines.right.get_x();
            if left != right {
                let line_addr = (y as u32) * 2048;
                let mut line = Line::from_lines(&lines.left, &lines.right);
                for x in left..right {
                    let col = line.get_rgb15();
                    let addr = line_addr + (x as u32 * 2);
                    self.vram.write_halfword(addr, col);
                    line.inc();
                }
            }
            lines.left.inc();
            lines.right.inc();
        }
        min_y = lines.max_y;
        // TODO: validate that we get more if min_y < max_y
        // Also: we kind of want to _replace_ one of our lines (either left or right?)
        if let Some(mut lines) = Self::get_intersection_points(vertices, min_y) {
            //println!("  Draw2 {} => {}", min_y, lines.max_y);
            // Continue drawing.
            for y in min_y..lines.max_y {
                let left = lines.left.get_x();
                let right = lines.right.get_x();
                //println!("    Draw x {} => {}", left, right);
                if left != right {
                    let line_addr = (y as u32) * 2048;
                    let mut line = Line::from_lines(&lines.left, &lines.right);
                    for x in left..right {
                        let col = line.get_rgb15();
                        let addr = line_addr + (x as u32 * 2);
                        self.vram.write_halfword(addr, col);
                        line.inc();
                    }
                }
                lines.left.inc();
                lines.right.inc();
            }
        };
    }
}

// Internal
impl SoftwareRenderer {
    fn get_intersection_points(vertices: &[Vertex], line: u16) -> Option<Lines> {
        let mut left: Option<Line> = None;
        let mut right: Option<Line> = None;
        let mut max_y = u16::MAX;
        for i in 0..3 {
            let vertex_a = &vertices[i];
            let vertex_b = &vertices[(i + 1) % 3];
            // (0,0) is TOP-LEFT. (TODO: verify..?)
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
                } else {
                    left = Some(other_line);
                    right = Some(line);
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
}

impl Line {
    fn from_vertices(top: &Vertex, bottom: &Vertex) -> Self {
        let gradient = (1 << 16) / ((bottom.coord.y - top.coord.y) as i32);
        Self {
            x_gradient: gradient * (bottom.coord.x as i32 - top.coord.x as i32),
            x: (top.coord.x as i32) << 16,
            r_gradient: gradient * (bottom.col.r as i32 - top.col.r as i32),
            r: (top.col.r as i32) << 16,
            g_gradient: gradient * (bottom.col.g as i32 - top.col.g as i32),
            g: (top.col.g as i32) << 16,
            b_gradient: gradient * (bottom.col.b as i32 - top.col.b as i32),
            b: (top.col.b as i32) << 16,
        }
    }

    fn from_lines(left: &Line, right: &Line) -> Self {
        let gradient = (1 << 16) / ((right.x - left.x) as i32);
        Self {
            x_gradient: 0,
            x: 0,
            r_gradient: gradient * (right.r as i32 - left.r as i32),
            r: left.r as i32,
            g_gradient: gradient * (right.g as i32 - left.g as i32),
            g: left.g as i32,
            b_gradient: gradient * (right.b as i32 - left.b as i32),
            b: left.b as i32,
        }
    }

    fn get_x(&self) -> u16 {
        (self.x >> 16) as u16
    }
    fn get_rgb15(&self) -> u16 {
        Color {
            r: (self.r >> 16) as u8,
            g: (self.g >> 16) as u8,
            b: (self.b >> 16) as u8,
        }.to_rgb15()
    }

    /// Advance internal state.
    fn inc(&mut self) {
        self.x += self.x_gradient;
        self.r += self.r_gradient;
        self.g += self.g_gradient;
        self.b += self.b_gradient;
    }
}

struct Lines {
    left: Line,
    right: Line,
    max_y: u16,
}