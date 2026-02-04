use mips::mem::Data;
use crate::{
    mem::DMADevice,
    utils::{bits::*, interface::MemInterface}
};
use std::collections::VecDeque;

pub struct MDECStatus {
    pub data_in_ready: bool,
    pub data_out_ready: bool,
}

/// Motion decoder.
/// This handles video decoding.
pub struct MDEC {
    status:                 Status,
    command:                Option<Command>,
    param_words_remaining:  u16,

    luminance_quant_table:  [u8; 64],
    color_quant_table:      [u8; 64],
    scale_table:            [i16; 64],

    in_fifo:                VecDeque<u16>,
    out_fifo:               VecDeque<u32>,
    data_in_enable:         bool,
    data_out_enable:        bool,

    /// DMA re-orders RGB15 and RGB24 blocks.
    /// If this is set to Some, it marks the width (in words)
    /// of an 8-pixel row.
    use_reorder:            Option<usize>,
    dma_reorder_fifo:       VecDeque<u32>,

    current_block:          Block,
    cr_block:               [i16; 64],
    cb_block:               [i16; 64],
}

impl MDEC {
    pub fn new() -> Self {
        Self {
            status: Status::Init,
            command: None,
            param_words_remaining: 0,

            luminance_quant_table:  [0; 64],
            color_quant_table:      [0; 64],
            scale_table:            [0; 64],

            in_fifo:                VecDeque::new(),
            out_fifo:               VecDeque::new(),
            data_in_enable:         false,
            data_out_enable:        false,

            use_reorder:            None,
            dma_reorder_fifo:       VecDeque::new(),

            current_block:          Block::None,
            cr_block:               [0; 64],
            cb_block:               [0; 64],
        }
    }

    pub fn clock(&mut self, _cycles: usize) -> MDECStatus {
        if let Some(command) = self.command {
            if self.param_words_remaining == 0 {
                use Command::*;
                match command {
                    DecodeMacroblock { output_depth, signed, set_bit_15 } => {
                        while !self.in_fifo.is_empty() {
                            self.decode_macroblock(output_depth, signed, set_bit_15);
                        }
                    },
                    SetQuantTable { use_color } => self.set_quant_table(use_color),
                    SetScaleTable => self.set_scale_table(),
                }
                if self.current_block != Block::Cr {
                    panic!("MDEC block not ending on Cr!");
                }
                self.finish_command();
                self.status.remove(Status::DataInFifoFull);
            } else if self.in_fifo.len() > 4096 {
                use Command::*;
                match command {
                    DecodeMacroblock { output_depth, signed, set_bit_15 } => {
                        while self.in_fifo.len() > 128 {
                            self.decode_macroblock(output_depth, signed, set_bit_15);
                        }
                    },
                    _ => {},
                }
                self.status.remove(Status::DataInFifoFull);
                self.status.insert(Status::DataInReq);
            }
        }
        MDECStatus {
            data_in_ready: self.status.contains(Status::DataInReq),
            data_out_ready: self.status.contains(Status::DataOutReq),
        }
    }
}

impl MemInterface for MDEC {
    fn read_word(&mut self, addr: u32) -> u32 {
        let data = match addr {
            0x1F80_1820 => self.read_data(),
            0x1F80_1824 => self.read_status(),
            _ => panic!("invalid MDEC read address"),
        };
        //println!("read mdec {:X} from {:X}", data, addr);
        data
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        //println!("write mdec {:X} to {:X}", data, addr);
        match addr {
            0x1F80_1820 => self.write_command(data),
            0x1F80_1824 => self.write_control(data),
            _ => panic!("invalid MDEC write address"),
        }
    }
}

impl DMADevice for MDEC {
    fn dma_read_word(&mut self) -> Data<u32> {
        let data = if let Some(row_words) = self.use_reorder {
            if self.dma_reorder_fifo.is_empty() {
                let block_size = 8 * row_words;
                let reorder_words = block_size * 2;
                for row in 0..8 {
                    for word in 0..row_words {
                        let data = self.out_fifo[word + row * row_words];
                        self.dma_reorder_fifo.push_back(data);
                    }
                    for word in 0..row_words {
                        let data = self.out_fifo[block_size + word + row * row_words];
                        self.dma_reorder_fifo.push_back(data);
                    }
                }
                self.out_fifo.drain(0..reorder_words);
            }
            self.dma_reorder_fifo.pop_front().unwrap()
        } else {
            self.read_data()
        };
        if self.dma_reorder_fifo.is_empty() && self.out_fifo.is_empty() {
            self.status.remove(Status::DataOutReq);
            self.status.insert(Status::DataOutFifoEmpty);
        }
        Data { data, cycles: 1 }
    }

    fn dma_write_word(&mut self, data: u32) -> usize {
        self.write_command(data);
        1
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct Status: u32 {
        const DataOutFifoEmpty  = bit!(31);
        const DataInFifoFull    = bit!(30);
        const CommandBusy       = bit!(29);
        const DataInReq         = bit!(28);
        const DataOutReq        = bit!(27);
        const DataOutputDepth   = bits![25, 26];
        const DataOutSigned     = bit!(24);
        const DataOutBit15      = bit!(23);
        const CurrentBlock      = bits![16, 17, 18];

        const Init = bits![31, 18];

        // Block modes:
        const Mono = bit!(18);
        const Cr = bits![18];
        const Cb = bits![16, 18];
        const Y0 = 0;
        const Y1 = bits![16];
        const Y2 = bits![17];
        const Y3 = bits![16, 17];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct Control: u32 {
        const Reset         = bit!(31);
        const EnableDataIn  = bit!(30);
        const EnableDataOut = bit!(29);
    }
}

#[derive(Clone, Copy)]
enum OutputDepth {
    Mono4,
    Mono8,
    RGB15,
    RGB24
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Block {
    None,
    Cr,
    Cb,
    Y0,
    Y1,
    Y2,
    Y3,
}

impl Block {
    fn to_status_bits(&self) -> Status {
        use Block::*;
        match self {
            None => Status::empty(),
            Cr => Status::Cr,
            Cb => Status::Cb,
            Y0 => Status::Y0,
            Y1 => Status::Y1,
            Y2 => Status::Y2,
            Y3 => Status::Y3,
        }
    }
}

/// MDEC commands.
#[derive(Clone, Copy)]
enum Command {
    DecodeMacroblock{output_depth: OutputDepth, signed: bool, set_bit_15: bool},
    SetQuantTable{use_color: bool},
    SetScaleTable,
}

impl Command {
    fn decode(data: u32) -> Command {
        use Command::*;
        let command = data >> 29;
        match command {
            1 => {
                let output_depth = match (data >> 27) & 0x3 {
                    0b00 => OutputDepth::Mono4,
                    0b01 => OutputDepth::Mono8,
                    0b10 => OutputDepth::RGB24,
                    0b11 => OutputDepth::RGB15,
                    _ => unreachable!()
                };
                DecodeMacroblock{output_depth, signed: test_bit!(data, 26), set_bit_15: test_bit!(data, 25)}
            },
            2 => SetQuantTable { use_color: test_bit!(data, 0) },
            3 => SetScaleTable,
            _ => panic!("unrecognised MDEC command"),
        }
    }
}

#[inline(always)]
const fn sign_extend_10(val: u16) -> i16 {
    (val << 6) as i16 >> 6
}

/// Combine the high nybbles of two values
/// to make a single 8-bit value.
#[inline(always)]
const fn combine_u8(lo: u8, hi: u8) -> u8 {
    (lo >> 4) | (hi & 0xF0)
}

/// Combine RGB values into a single halfword.
#[inline(always)]
const fn rgb_24_to_15(r: u8, g: u8, b: u8, hi: u16) -> u16 {
    ((r as u16) >> 3) |
    (((g as u16) >> 3) << 5) |
    (((b as u16) >> 3) << 10) |
    hi
}

// Internal
impl MDEC {
    fn read_data(&mut self) -> u32 {
        let data = self.out_fifo.pop_front().expect("reading from empty mdec buffer");
        if self.out_fifo.is_empty() {
            self.status.insert(Status::DataOutFifoEmpty);
        }
        data
    }

    fn read_status(&self) -> u32 {
        let param_words = self.param_words_remaining.wrapping_sub(1) as u32;
        let current_mode = self.current_block.to_status_bits();
        (self.status | current_mode).bits() | param_words
    }

    fn write_control(&mut self, data: u32) {
        let control = Control::from_bits_truncate(data);
        if control.contains(Control::Reset) {
            self.command = None;
            self.param_words_remaining = 0;
            self.status = Status::Init;
        }
        self.data_in_enable = control.contains(Control::EnableDataIn);
        self.data_out_enable = control.contains(Control::EnableDataOut);
    }

    fn write_command(&mut self, data: u32) {
        if self.command.is_some() {
            let data_lo = data as u16;
            let data_hi = (data >> 16) as u16;
            self.in_fifo.push_back(data_lo);
            self.in_fifo.push_back(data_hi);
            self.param_words_remaining -= 1;
            if self.param_words_remaining == 0 {
                self.status.insert(Status::DataInFifoFull);
                self.status.remove(Status::DataInReq);
            }
        } else {
            self.start_command(data);
        }
    }

    fn start_command(&mut self, data: u32) {
        let command = Command::decode(data);
        self.param_words_remaining = match command {
            Command::DecodeMacroblock {..} => (data & 0xFFFF) as u16,
            Command::SetQuantTable { use_color: true } => 32,
            Command::SetQuantTable { use_color: false } => 16,
            Command::SetScaleTable => 32,
        };
        self.command = Some(command);
        self.status.insert(Status::CommandBusy | Status::Mono);
        if self.data_in_enable {
            self.status.insert(Status::DataInReq);
        }
        self.current_block = Block::Cr;
    }

    /// This function will be called repeatedly for every input data chunk.
    /// Each input data chunk should be 32 words.
    fn decode_macroblock(&mut self, output_depth: OutputDepth, signed: bool, set_bit_15: bool) {
        match output_depth {
            OutputDepth::Mono4 => {
                let Some(mono_out) = self.process_mono(!signed) else {
                    return;
                };
                self.output_mono4(&mono_out);
            },
            OutputDepth::Mono8 => {
                let Some(mono_out) = self.process_mono(!signed) else {
                    return;
                };
                self.output_mono8(&mono_out);
            },
            OutputDepth::RGB15 => {
                match self.current_block {
                    Block::None => panic!("block set to none"),
                    Block::Cr => {
                        self.cr_block.fill(0);
                        if !decode_block(&mut self.in_fifo, &self.color_quant_table, &self.scale_table, &mut self.cr_block) {
                            // End padding.
                            return;
                        }
                        self.current_block = Block::Cb;
                    },
                    Block::Cb => {
                        self.cb_block.fill(0);
                        if !decode_block(&mut self.in_fifo, &self.color_quant_table, &self.scale_table, &mut self.cb_block) {
                            panic!("mdec: aborted during cb processing");
                        }
                        self.current_block = Block::Y0;
                    },
                    Block::Y0 => {
                        let Some(rgb) = self.process_rgb(!signed, 0, 0) else {
                            panic!("mdec: aborting during y processing");
                        };
                        self.output_rgb15(&rgb, set_bit_15);
                        self.current_block = Block::Y1;
                    },
                    Block::Y1 => {
                        let Some(rgb) = self.process_rgb(!signed, 8, 0) else {
                            panic!("mdec: aborting during y processing");
                        };
                        self.output_rgb15(&rgb, set_bit_15);
                        self.current_block = Block::Y2;
                    },
                    Block::Y2 => {
                        let Some(rgb) = self.process_rgb(!signed, 0, 8) else {
                            panic!("mdec: aborting during y processing");
                        };
                        self.output_rgb15(&rgb, set_bit_15);
                        self.current_block = Block::Y3;
                    },
                    Block::Y3 => {
                        let Some(rgb) = self.process_rgb(!signed, 8, 8) else {
                            panic!("mdec: aborting during y processing");
                        };
                        self.output_rgb15(&rgb, set_bit_15);
                        self.current_block = Block::Cr;
                    },
                }
            },
            OutputDepth::RGB24 => {
                match self.current_block {
                    Block::None => panic!("block set to none"),
                    Block::Cr => {
                        self.cr_block.fill(0);
                        if !decode_block(&mut self.in_fifo, &self.color_quant_table, &self.scale_table, &mut self.cr_block) {
                            // End padding.
                            return;
                        }
                        self.current_block = Block::Cb;
                    },
                    Block::Cb => {
                        self.cb_block.fill(0);
                        if !decode_block(&mut self.in_fifo, &self.color_quant_table, &self.scale_table, &mut self.cb_block) {
                            panic!("mdec: aborted during cb processing");
                        }
                        self.current_block = Block::Y0;
                    },
                    Block::Y0 => {
                        let Some(rgb) = self.process_rgb(!signed, 0, 0) else {
                            panic!("mdec: aborting during y processing");
                        };
                        self.output_rgb24(&rgb);
                        self.current_block = Block::Y1;
                    },
                    Block::Y1 => {
                        let Some(rgb) = self.process_rgb(!signed, 8, 0) else {
                            panic!("mdec: aborting during y processing");
                        };
                        self.output_rgb24(&rgb);
                        self.current_block = Block::Y2;
                    },
                    Block::Y2 => {
                        let Some(rgb) = self.process_rgb(!signed, 0, 8) else {
                            panic!("mdec: aborting during y processing");
                        };
                        self.output_rgb24(&rgb);
                        self.current_block = Block::Y3;
                    },
                    Block::Y3 => {
                        let Some(rgb) = self.process_rgb(!signed, 8, 8) else {
                            panic!("mdec: aborting during y processing");
                        };
                        self.output_rgb24(&rgb);
                        self.current_block = Block::Cr;
                    },
                }
            },
        }
    }

    fn process_mono(&mut self, unsigned: bool) -> Option<[u8; 64]> {
        let mut y_block = [0_i16; 64];
        if decode_block(&mut self.in_fifo, &self.luminance_quant_table, &self.scale_table, &mut y_block) {
            let mut mono_out = [0_u8; 64];
            y_to_mono(&y_block, unsigned, &mut mono_out);
            Some(mono_out)
        } else {
            None
        }
    }

    fn process_rgb(&mut self, unsigned: bool, x_offset: usize, y_offset: usize) -> Option<[u8; 64 * 3]> {
        let mut y_block = [0_i16; 64];
        if decode_block(&mut self.in_fifo, &self.color_quant_table, &self.scale_table, &mut y_block) {
            let mut rgb_out = [0_u8; 64 * 3];
            yuv_to_rgb(&y_block, &self.cr_block, &self.cb_block, unsigned, x_offset, y_offset, &mut rgb_out);
            Some(rgb_out)
        } else {
            None
        }
    }

    fn output_mono4(&mut self, mono: &[u8]) {
        for i in 0..8 {
            let index = i * 8;
            let word = u32::from_le_bytes([
                combine_u8(mono[index + 0], mono[index + 1]),
                combine_u8(mono[index + 2], mono[index + 3]),
                combine_u8(mono[index + 4], mono[index + 5]),
                combine_u8(mono[index + 6], mono[index + 7]),
            ]);
            self.out_fifo.push_back(word);
        }
        self.status.remove(Status::DataOutFifoEmpty);
        if self.data_out_enable {
            self.status.insert(Status::DataOutReq);
        }
    }

    fn output_mono8(&mut self, mono: &[u8]) {
        for i in 0..16 {
            let index = i * 4;
            let word = u32::from_le_bytes([
                mono[index + 0],
                mono[index + 1],
                mono[index + 2],
                mono[index + 3],
            ]);
            self.out_fifo.push_back(word);
        }
        self.status.remove(Status::DataOutFifoEmpty);
        if self.data_out_enable {
            self.status.insert(Status::DataOutReq);
        }
    }

    fn output_rgb15(&mut self, rgb: &[u8], set_bit_15: bool) {
        let hi = if set_bit_15 {0x8000} else {0};
        for i in 0..32 {
            let rgb_index = i * 6;
            let lo = {
                let r = rgb[rgb_index];
                let g = rgb[rgb_index + 1];
                let b = rgb[rgb_index + 2];
                rgb_24_to_15(r, g, b, hi)
            };
            let hi = {
                let r = rgb[rgb_index + 3];
                let g = rgb[rgb_index + 4];
                let b = rgb[rgb_index + 5];
                rgb_24_to_15(r, g, b, hi)
            };
            let data = (lo as u32) | ((hi as u32) << 16);
            self.out_fifo.push_back(data);
        }
        self.status.remove(Status::DataOutFifoEmpty);
        if self.data_out_enable {
            self.status.insert(Status::DataOutReq);
        }
    }

    fn output_rgb24(&mut self, rgb: &[u8]) {
        for i in 0..48 {
            let rgb_index = i * 4;
            let data = u32::from_le_bytes([
                rgb[rgb_index],
                rgb[rgb_index + 1],
                rgb[rgb_index + 2],
                rgb[rgb_index + 3],
            ]);
            self.out_fifo.push_back(data);
        }
        self.status.remove(Status::DataOutFifoEmpty);
        if self.data_out_enable {
            self.status.insert(Status::DataOutReq);
        }
    }

    fn set_quant_table(&mut self, use_color: bool) {
        for i in 0..32 {
            let data = self.in_fifo.pop_front().expect("not enough data in mdec fifo! (lum table)");
            let bytes = data.to_le_bytes();
            let index = i * 2;
            self.luminance_quant_table[index] = bytes[0];
            self.luminance_quant_table[index + 1] = bytes[1];
        }
        if use_color {
            for i in 0..32 {
                let data = self.in_fifo.pop_front().expect("not enough data in mdec fifo! (col table)");
                let bytes = data.to_le_bytes();
                let index = i * 2;
                self.color_quant_table[index] = bytes[0];
                self.color_quant_table[index + 1] = bytes[1];
            }
        }
    }

    fn set_scale_table(&mut self) {
        for i in 0..64 {
            let data = self.in_fifo.pop_front().expect("not enough data in mdec fifo! (scale table)");
            self.scale_table[i] = data as i16;
        }
    }

    fn finish_command(&mut self) {
        if let Some(command) = self.command {
            match command {
                Command::DecodeMacroblock { output_depth, .. } => match output_depth {
                    OutputDepth::Mono4 | OutputDepth::Mono8 => self.use_reorder = None,
                    OutputDepth::RGB15 => self.use_reorder = Some(4),
                    OutputDepth::RGB24 => self.use_reorder = Some(6),
                },
                _ => self.use_reorder = None,
            }
        }
        self.command = None;
        self.status.remove(Status::CommandBusy | Status::CurrentBlock | Status::DataInReq);
        self.current_block = Block::None;
    }
}

/// Inverse zig-zag lookup table.
const ZAGZIG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10,
    17, 24, 32, 25, 18, 11, 4, 5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13, 6, 7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63
];

fn decode_block(data_in: &mut VecDeque<u16>, quant_table: &[u8; 64], scale_table: &[i16; 64], block_out: &mut [i16]) -> bool {
    let mut out_idx = 0;
    while let Some(data) = data_in.front() {
        // Padding code.
        if *data != 0xFE00 {
            break;
        }
        let _ = data_in.pop_front();
    }
    let Some(first_data) = data_in.pop_front() else {
        return false;
    };
    let quant_factor = (first_data >> 10) as i32;
    {
        let direct_current = sign_extend_10(first_data);
        if quant_factor == 0 {
            block_out[out_idx] = direct_current << 1;
        } else {
            let val = direct_current * quant_table[0] as i16;
            block_out[ZAGZIG[out_idx]] = val.clamp(-0x400, 0x3FF);
        }
    }
    while let Some(data) = data_in.pop_front() {
        let skip = data >> 10;
        out_idx += (skip + 1) as usize;
        if out_idx > 63 {
            break;
        }
        let relative = sign_extend_10(data);
        if quant_factor == 0 {
            block_out[out_idx] = relative << 1;
        } else {
            let val = ((relative as i32 * quant_table[out_idx] as i32 * quant_factor + 4) / 8) as i16;
            block_out[ZAGZIG[out_idx]] = val.clamp(-0x400, 0x3FF);
        }
    }
    process_core(block_out, scale_table);
    true
}

fn process_core(block: &mut [i16], scale_table: &[i16; 64]) {
    let mut temp_buffer = [0_i16; 64];
    let mut src = block;
    let mut dst = temp_buffer.as_mut_slice();
    for _ in 0..2 {
        for y in 0..8 {
            let col_offset = y * 8;
            for x in 0..8 {
                let sum = (0..64).step_by(8).fold(0, |acc, z| {
                    let n = src[z + y] as i32 * (scale_table[z + x] as i32 / 8);
                    acc + n
                });
                // Round.
                dst[col_offset + x] = ((sum + 0xFFF) / 0x2000) as i16;
            }
        }
        let x = dst;
        dst = src;
        src = x;
    }
}

/// Directly output macroblock as monochrome.
fn y_to_mono(block: &[i16], unsigned: bool, out: &mut [u8]) {
    for i in 0..64 {
        let y = block[i] << 7 >> 7;                        // Clip to 9 bits
        let y = y.clamp(-0x80, 0x7F) as i8 as u8; // Saturate to 8 bits
        out[i] = if unsigned {
            y ^ 0x80
        } else {
            y
        };
    }
}

/// Combine luminance and chrominance blocks together,
/// to form RGB8 output.
fn yuv_to_rgb(lum: &[i16], cr: &[i16], cb: &[i16], unsigned: bool, x_offset: usize, y_offset: usize, out: &mut [u8]) {
    for y in 0..8 {
        let lum_y = y * 8;
        let color_y = ((y + y_offset) >> 1) * 8;
        for x in 0..8 {
            let color_x = (x + x_offset) >> 1;
            let cr_val = cr[color_y + color_x] as i32;
            let cb_val = cb[color_y + color_x] as i32;
            let r = (cr_val * 0x166E) >> 12; // 1.402
            let g = ((cb_val * -0x57F) + (cr_val * -0xB6D)) >> 12; // -0.3437, -0.7143
            let b = (cb_val * 0x1C5A) >> 12; // 1.772
            let y_val = lum[lum_y + x];
            let r = (r as i16 + y_val).clamp(-0x80, 0x7F) as i8 as u8;
            let g = (g as i16 + y_val).clamp(-0x80, 0x7F) as i8 as u8;
            let b = (b as i16 + y_val).clamp(-0x80, 0x7F) as i8 as u8;
            let out_idx = (lum_y + x) * 3;
            if unsigned {
                out[out_idx] = r ^ 0x80;
                out[out_idx + 1] = g ^ 0x80;
                out[out_idx + 2] = b ^ 0x80;
            } else {
                out[out_idx] = r;
                out[out_idx + 1] = g;
                out[out_idx + 2] = b;
            }
        }
    }
}