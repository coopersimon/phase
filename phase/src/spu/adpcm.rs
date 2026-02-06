use crate::utils::bits::*;

/// Generates sound samples from ADPCM data, in BRR format.
/// 
/// ADPCM is Adaptive Differential Pulse-Code Modulation.
/// 
/// BRR is bit-rate reduction.
#[derive(Default)]
pub struct ADPCMDecoder {
    samples:    [i16; 28],
    // Does this decoder have a set of decoded samples?
    is_decoded: bool,
    // The last block decoded had the loop_end bit set.
    loop_end:   bool,
    // The last block decoded forces release mode for ADSR envelope.
    release:    bool,
}

impl ADPCMDecoder {
    pub fn reset(&mut self) {
        self.is_decoded = false;
        self.samples.fill(0);
    }

    /// Returns true if a new block needs to be decoded.
    pub fn needs_block(&self) -> bool {
        !self.is_decoded
    }

    /// Decode a block of ADPCM samples. Slice input should be 16 bytes.
    /// Returns true if this is the start of a loop.
    pub fn decode_block(&mut self, data: &[u8]) -> bool {
        // Decode the samples:
        let shift = data[0] & 0xF;
        let filter = ((data[0] >> 4) & 0x7) as usize;
        let pos_filter = POS_ADPCM_FILTER[filter];
        let neg_filter = NEG_ADPCM_FILTER[filter];
        let mut prev_0 = self.samples[26] as i32;
        let mut prev_1 = self.samples[27] as i32;
        for i in 0..14 {
            let in_data = data[i + 2];
            let lo = (in_data as i16) << 12;
            let hi = ((in_data & 0xF0) as i16) << 8;
            let s_0 = decode_adpcm_sample(lo, shift, prev_0, prev_1, pos_filter, neg_filter);
            prev_0 = prev_1;
            prev_1 = s_0 as i32;
            let s_1 = decode_adpcm_sample(hi, shift, prev_0, prev_1, pos_filter, neg_filter);
            prev_0 = prev_1;
            prev_1 = s_1 as i32;
            self.samples[i * 2 + 0] = s_0;
            self.samples[i * 2 + 1] = s_1;
        }
        self.is_decoded = true;
        let flags = data[1];
        self.loop_end = test_bit!(flags, 0);
        if self.loop_end {
            self.release = !test_bit!(flags, 1);
        }
        let loop_start = test_bit!(flags, 2);
        loop_start
    }

    pub fn is_loop_end(&self) -> bool {
        self.loop_end
    }

    pub fn should_release(&self) -> bool {
        self.release
    }

    /// Get the next sample from the decoded data.
    pub fn get_sample(&self, n: usize) -> i16 {
        self.samples[n]
    }
}

const POS_ADPCM_FILTER: [i32; 5] = [0, 60, 115, 98, 122];
const NEG_ADPCM_FILTER: [i32; 5] = [0, 0, -52, -55, -60];

/// Decode a single ADPCM sample from the source nybble.
/// The nybble must be already shifted up to the top 4 bits.
/// 
/// prev_1 is the last sample, prev_0 is the sample before that.
#[inline]
fn decode_adpcm_sample(nybble: i16, shift: u8, prev_0: i32, prev_1: i32, pos_filter: i32, neg_filter: i32) -> i16 {
    let shifted = (nybble as i32) >> shift;
    let sample = shifted + ((prev_1 * pos_filter) + (prev_0 * neg_filter) + 32) / 64;
    sample.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}