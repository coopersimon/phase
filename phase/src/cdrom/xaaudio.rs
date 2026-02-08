use dasp::frame::{
    Stereo
};

use super::{CodingInfo};
use crate::{
    audio::ADPCMDecoder,
    utils::bits::*
};

/// CD subsystem for decoding XA-ADPCM.
pub struct XAAudio {
    current_vol: VolumeMap,
    staging_vol: VolumeMap,

    file_filter: u8,
    channel_filter: u8,
    sound_map_info: CodingInfo,

    mute: bool,

    sample_buffer: Vec<Stereo<i16>>,
    conv_sample_buffer: Vec<Stereo<i16>>,
    pending_samples: bool,
    left_decoder: ADPCMDecoder,
    right_decoder: ADPCMDecoder,
}

impl XAAudio {
    pub fn new() -> Self {
        Self {
            current_vol: VolumeMap::default(),
            staging_vol: VolumeMap::default(),

            file_filter: 0,
            channel_filter: 0,
            sound_map_info: CodingInfo::empty(),

            mute: false,

            sample_buffer:      Vec::new(),
            conv_sample_buffer: Vec::new(),
            pending_samples: false,
            left_decoder: ADPCMDecoder::default(),
            right_decoder: ADPCMDecoder::default(),
        }
    }

    /// Set filters, via CD command 0x0D.
    pub fn set_filters(&mut self, file: u8, channel: u8) {
        self.file_filter = file;
        self.channel_filter = channel;
    }

    /// Get (file, channel) filters.
    pub fn get_filters(&self) -> (u8, u8) {
        (self.file_filter, self.channel_filter)
    }

    /// Returns true if the file _and_ channel match.
    pub fn test_filter(&mut self, file: u8, channel: u8) -> bool {
        self.file_filter == file && self.channel_filter == channel
    }

    /// When encountering a new audio sector, this method
    /// decodes the data.
    pub fn write_audio_sector(&mut self, buffer: &[u8], coding_info: CodingInfo) {
        let stereo = coding_info.contains(CodingInfo::Stereo);
        if coding_info.contains(CodingInfo::BitsPerSample) {
            self.decode_8bit_samples(buffer, stereo);
        } else {
            self.decode_4bit_samples(buffer, stereo);
        }
        let low_sample_rate = coding_info.contains(CodingInfo::SampleRate);
        self.resample_audio(low_sample_rate);
        self.pending_samples = true;
    }

    /// This method can be used to retrieve the decoded audio samples
    /// for transportation to SPU, if new ones are ready.
    pub fn fetch_decoded_audio<'a> (&'a mut self) -> Option<&'a [Stereo<i16>]> {
        if self.pending_samples {
            self.pending_samples = false;
            Some(&self.conv_sample_buffer)
        } else {
            None
        }
    }
}

// CD register setters.
impl XAAudio {
    pub fn apply_changes(&mut self, data: u8) {
        self.mute = test_bit!(data, 0);
        if test_bit!(data, 5) {
            self.current_vol = self.staging_vol.clone();
        }
        // TODO: should these be reset here?
        self.left_decoder.reset();
        self.right_decoder.reset();
    }

    pub fn set_sound_map_info(&mut self, data: u8) {
        self.sound_map_info = CodingInfo::from_bits_truncate(data);
    }

    pub fn set_left_to_left(&mut self, data: u8) {
        self.staging_vol.left_to_left = data;
    }

    pub fn set_left_to_right(&mut self, data: u8) {
        self.staging_vol.left_to_right = data;
    }

    pub fn set_right_to_left(&mut self, data: u8) {
        self.staging_vol.right_to_left = data;
    }

    pub fn set_right_to_right(&mut self, data: u8) {
        self.staging_vol.right_to_right = data;
    }

    pub fn write_data(&mut self, _data: u8) {
        unimplemented!("writing data directly to sound out");
    }
}

#[derive(Default, Clone)]
struct VolumeMap {
    left_to_left: u8,
    left_to_right: u8,
    right_to_left: u8,
    right_to_right: u8,
}

impl VolumeMap {
    #[inline]
    fn apply_stereo(&self, left: i16, right: i16) -> Stereo<i16> {
        let left_to_left = ((left as i32) * (self.left_to_left as i32)) >> 7;
        let left_to_right = ((left as i32) * (self.left_to_right as i32)) >> 7;
        let right_to_left = ((right as i32) * (self.right_to_left as i32)) >> 7;
        let right_to_right = ((right as i32) * (self.right_to_right as i32)) >> 7;
        let left_out = (left_to_left + right_to_left) as i16; // TODO: saturate + clamp?
        let right_out = (left_to_right + right_to_right) as i16;
        [left_out, right_out]
    }

    #[inline]
    fn apply_mono(&self, sample: i16) -> Stereo<i16> {
        let left_mul = (self.left_to_left as i32) + (self.right_to_left as i32);
        let right_mul = (self.left_to_right as i32) + (self.right_to_right as i32);
        let left_out = ((sample as i32) * left_mul) >> 7;
        let right_out = ((sample as i32) * right_mul) >> 7;
        [left_out as i16, right_out as i16]
    }
}

impl XAAudio {
    fn decode_4bit_samples(&mut self, buffer: &[u8], stereo: bool) {
        self.sample_buffer.clear();
        // 18 chunks of 128 bytes.
        for chunk_buffer in buffer.as_chunks::<128>().0 {
            if stereo {
                for block in 0..4 {
                    let header_offset = 4 + (block * 2);
                    let data = &chunk_buffer[(0x10 + block)..];
                    let left_header = chunk_buffer[header_offset];
                    self.left_decoder.decode_xa_4bit_block(data, left_header, 0);
                    let right_header = chunk_buffer[header_offset + 1];
                    self.right_decoder.decode_xa_4bit_block(data, right_header, 4);
                    let left_samples = self.left_decoder.get_sample_block();
                    let right_samples = self.right_decoder.get_sample_block();
                    for (left, right) in left_samples.iter().zip(right_samples.iter()) {
                        let sample = self.current_vol.apply_stereo(*left, *right);
                        self.sample_buffer.push(sample);
                    }
                }
            } else {
                for block in 0..4 {
                    let header_offset = 4 + (block * 2);
                    let data = &chunk_buffer[(0x10 + block)..];
                    let header_0 = chunk_buffer[header_offset];
                    self.left_decoder.decode_xa_4bit_block(data, header_0, 0);
                    for mono in self.left_decoder.get_sample_block().iter() {
                        let sample = self.current_vol.apply_mono(*mono);
                        self.sample_buffer.push(sample);
                    }
                    let header_1 = chunk_buffer[header_offset + 1];
                    self.left_decoder.decode_xa_4bit_block(data, header_1, 4);
                    for mono in self.left_decoder.get_sample_block().iter() {
                        let sample = self.current_vol.apply_mono(*mono);
                        self.sample_buffer.push(sample);
                    }
                }
            }
        }
    }

    fn decode_8bit_samples(&mut self, buffer: &[u8], stereo: bool) {
        unimplemented!("8bit audio")
    }

    /// CD audio arrives at 18.9kHz or 37.8kHz.
    /// We need to resample to 44.1kHz for the SPU.
    fn resample_audio(&mut self, low_sample_rate: bool) {
        if low_sample_rate {
            unimplemented!("18.9kHz CD audio");
        }
        self.conv_sample_buffer.clear();
        // 6 samples in => 7 samples out
        for base_idx in (0..self.sample_buffer.len()).step_by(6) {
            self.conv_sample_buffer.push(self.sample_buffer[base_idx]);
            for i in 0..6 {
                let a_idx = base_idx + i;
                let b_idx = if a_idx + 1 == self.sample_buffer.len() {a_idx} else {a_idx + 1};
                let a = self.sample_buffer[a_idx];
                let b = self.sample_buffer[b_idx];
                let a_factor = LINEAR_INTERPOLATE_TABLE[i];
                let b_factor = 32768 - a_factor;
                let left = a[0] as i32 * a_factor + b[0] as i32 * b_factor;
                let right = a[1] as i32 * a_factor + b[1] as i32 * b_factor;
                self.conv_sample_buffer.push([(left >> 15) as i16, (right >> 15) as i16])
            }
        }
    }
}

const LINEAR_INTERPOLATE_TABLE: [i32; 6] = [
    4681,
    9362,
    14043,
    18725,
    23406,
    28087,
];