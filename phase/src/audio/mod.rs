mod adpcm;
mod resampler;

pub use adpcm::*;
pub use resampler::*;

// TODO: source these numbers from elsewhere.
use crate::spu::CYCLES_PER_SAMPLE;

/// Cycles per second.
const CLOCK_RATE: usize = CYCLES_PER_SAMPLE * 44100;
/// Emulated cycles per second.
/// TODO: PAL (get these values from GPU)
const REAL_CLOCK_RATE: f64 = 3413. * 263. * 60. * 7. / 11.;

/// Base sample rate for audio.
const BASE_SAMPLE_RATE: f64 = 44_100.0;

const REAL_SAMPLE_RATE_RATIO: f64 = REAL_CLOCK_RATE / (CLOCK_RATE as f64);
pub const REAL_BASE_SAMPLE_RATE: f64 = BASE_SAMPLE_RATE * REAL_SAMPLE_RATE_RATIO;
