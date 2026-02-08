mod voice;
mod adsr;
mod sweep;

use std::collections::VecDeque;

use crossbeam_channel::Sender;
use dasp::frame::{
    Frame, Stereo
};
use mips::mem::Data;

use crate::{
    audio::SamplePacket,
    interrupt::Interrupt,
    mem::{DMADevice, ram::RAM},
    utils::{bits::*, interface::MemInterface}
};

use voice::Voice;
use sweep::SweepVolume;

const SPU_RAM_SIZE: usize = 512 * 1024;
const SPU_FIFO_SIZE: usize = 32;

pub const CYCLES_PER_SAMPLE: usize = 0x300;
const SAMPLE_PACKET_SIZE: usize = 32;

/// Sound processing unit.
pub struct SPU {
    voices:         [Voice; 24],
    ram:            RAM,
    ram_full_addr:  u32,
    ram_fifo:       VecDeque<u16>,
    transfer_fifo:  bool,

    // Registers
    ram_addr:       u16,
    ram_irq_addr:   u16,
    ram_ctrl:       u16,

    main_vol:       SweepVolume,
    cd_input_vol:   StereoVolume,
    ext_input_vol:  StereoVolume,
    reverb_vol:     StereoVolume,

    control:    SPUControl,
    status:     SPUStatus,
    irq_latch:  bool,

    // Sample generation:
    cycle_count: usize,
    noise_level: i16,
    noise_timer: isize,

    cd_audio_sample:    Stereo<i16>,
    cd_audio_damping:   u8,
    cd_audio_fifo:      VecDeque<Stereo<i16>>,

    // Comms with audio thread
    sample_buffer:      Vec<Stereo<f32>>,
    sample_sender:      Option<Sender<SamplePacket>>,
}

impl SPU {
    pub fn new() -> Self{
        Self {
            voices:         Default::default(),
            ram:            RAM::new(SPU_RAM_SIZE),
            ram_full_addr:  0,
            ram_fifo:       VecDeque::new(),
            transfer_fifo:  false,

            ram_addr:       0,
            ram_irq_addr:   0,
            ram_ctrl:       0,

            main_vol:       Default::default(),
            cd_input_vol:   Default::default(),
            ext_input_vol:  Default::default(),
            reverb_vol:     Default::default(),

            control:    SPUControl::empty(),
            status:     SPUStatus::empty(),
            irq_latch:  false,

            cycle_count: 0,
            noise_level: 0,
            noise_timer: 0,

            cd_audio_sample:    Stereo::EQUILIBRIUM,
            cd_audio_damping:   0,
            cd_audio_fifo:      VecDeque::new(),

            sample_buffer:  Vec::new(),
            sample_sender:  None,
        }
    }

    /// Call to enable audio on the appropriate thread.
    /// 
    /// This should be done before any rendering.
    pub fn enable_audio(&mut self, sample_sender: Sender<SamplePacket>) {
        self.sample_sender = Some(sample_sender);
    }

    pub fn push_new_cd_audio(&mut self, cd_audio: &[Stereo<i16>]) {
        self.cd_audio_fifo.extend(cd_audio);
    }

    /// Clock internally, generate samples if necessary,
    /// and push a new audio data batch to output.
    pub fn clock(&mut self, cycles: usize) -> Interrupt {
        if self.transfer_fifo {
            self.transfer_from_fifo();
        }

        self.cycle_count += cycles;
        if self.cycle_count > CYCLES_PER_SAMPLE {
            self.cycle_count -= CYCLES_PER_SAMPLE;

            self.clock_noise();

            // Generate sample
            let sample = self.generate_sample();
            self.sample_buffer.push(sample);
            
            // Output to audio thread
            if self.sample_buffer.len() >= SAMPLE_PACKET_SIZE {
                let sample_packet = std::mem::replace(&mut self.sample_buffer, Vec::with_capacity(SAMPLE_PACKET_SIZE)).into_boxed_slice();
                if let Some(s) = &self.sample_sender {
                    let _ = s.send(sample_packet);
                }
            }

            if self.irq_latch {
                self.irq_latch = false;
                Interrupt::SPU
            } else {
                Interrupt::empty()
            }
        } else {
            Interrupt::empty()
        }
    }

    pub fn dma_ready(&self) -> bool {
        self.status.contains(SPUStatus::DMATransferReq)
    }
}

impl MemInterface for SPU {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x1F80_1C00..=0x1F80_1D7F => {
                let voice_idx = (addr >> 4) & 0x1F;
                self.voices[voice_idx as usize].read_halfword(addr & 0xF)
            },
            0x1F80_1D80 => self.main_vol.get_left(),
            0x1F80_1D82 => self.main_vol.get_right(),
            0x1F80_1D84 => self.reverb_vol.left as u16,
            0x1F80_1D86 => self.reverb_vol.right as u16,
            0x1F80_1D88 => 0, // KON
            0x1F80_1D8A => 0, // KON
            0x1F80_1D8C => 0, // KOFF
            0x1F80_1D8E => 0, // KOFF
            0x1F80_1D90 => self.get_pitch_mod_lo(),
            0x1F80_1D92 => self.get_pitch_mod_hi(),
            0x1F80_1D94 => self.get_noise_lo(),
            0x1F80_1D96 => self.get_noise_hi(),
            0x1F80_1D98 => 0, // TODO:Echo flags
            0x1F80_1D9A => 0, // TODO:Echo flags
            0x1F80_1D9C => self.get_endx_lo(),
            0x1F80_1D9E => self.get_endx_hi(),
            0x1F80_1DA2 => 0, // TODO: reverb base
            0x1F80_1DA4 => self.ram_irq_addr,
            0x1F80_1DA6 => self.ram_addr,
            0x1F80_1DAA => self.control.bits(),
            0x1F80_1DAC => self.ram_ctrl,
            0x1F80_1DAE => self.status.bits(),
            0x1F80_1DB0 => self.cd_input_vol.left as u16,
            0x1F80_1DB2 => self.cd_input_vol.right as u16,
            0x1F80_1DB4 => self.ext_input_vol.left as u16,
            0x1F80_1DB6 => self.ext_input_vol.right as u16,
            0x1F80_1DB8 => self.main_vol.get_left(),
            0x1F80_1DBA => self.main_vol.get_right(),
            0x1F80_1DC0..=0x1F80_1DFF => { // Reverb
                0
            },
            _ => panic!("invalid SPU read {:X}", addr)
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x1F80_1C00..=0x1F80_1D7F => {
                let voice_idx = (addr >> 4) & 0x1F;
                self.voices[voice_idx as usize].write_halfword(addr & 0xF, data);
            },
            0x1F80_1D80 => self.main_vol.set_left(data),
            0x1F80_1D82 => self.main_vol.set_right(data),
            0x1F80_1D84 => self.reverb_vol.left = data as i16,
            0x1F80_1D86 => self.reverb_vol.right = data as i16,
            0x1F80_1D88 => self.set_key_on_lo(data),
            0x1F80_1D8A => self.set_key_on_hi(data),
            0x1F80_1D8C => self.set_key_off_lo(data),
            0x1F80_1D8E => self.set_key_off_hi(data),
            0x1F80_1D90 => self.set_pitch_mod_lo(data),
            0x1F80_1D92 => self.set_pitch_mod_hi(data),
            0x1F80_1D94 => self.set_noise_lo(data),
            0x1F80_1D96 => self.set_noise_hi(data),
            0x1F80_1D98 => {}, // TODO:Echo flags
            0x1F80_1D9A => {}, // TODO:Echo flags
            0x1F80_1D9C => {}, // ENDX
            0x1F80_1D9E => {}, // ENDX
            0x1F80_1DA2 => {}, // TODO: reverb base
            0x1F80_1DA4 => self.ram_irq_addr = data,
            0x1F80_1DA6 => {
                self.ram_addr = data;
                self.ram_full_addr = (self.ram_addr as u32) << 3;
            },
            0x1F80_1DA8 => self.write_fifo(data),
            0x1F80_1DAA => self.set_control(data),
            0x1F80_1DAC => self.ram_ctrl = data,
            0x1F80_1DB0 => self.cd_input_vol.left = data as i16,
            0x1F80_1DB2 => self.cd_input_vol.right = data as i16,
            0x1F80_1DB4 => self.ext_input_vol.left = data as i16,
            0x1F80_1DB6 => self.ext_input_vol.right = data as i16,
            0x1F80_1DC0..=0x1F80_1DFF => { // Reverb
                
            },
            _ => panic!("invalid SPU write {:X}", addr)
        }
    }

    // Usually SPU should not be accessed via word interface.

    fn read_word(&mut self, addr: u32) -> u32 {
        panic!("read SPU word from {:X}", addr);
        let lo = self.read_halfword(addr) as u32;
        let hi = self.read_halfword(addr + 2) as u32;
        lo | (hi << 16)
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        panic!("write SPU word to {:X}", addr);
        let lo = data as u16;
        let hi = (data >> 16) as u16;
        self.write_halfword(addr, lo);
        self.write_halfword(addr + 2, hi);
    }
}

impl DMADevice for SPU {
    fn dma_read_word(&mut self) -> Data<u32> {
        // TODO: further checks here.
        let data = self.ram.read_word(self.ram_full_addr);
        self.ram_full_addr += 4;
        Data { data, cycles: 1 }
    }

    fn dma_write_word(&mut self, data: u32) -> usize {
        // TODO: further checks here.
        self.ram.write_word(self.ram_full_addr, data);
        self.ram_full_addr += 4;
        1
    }
}

// Internal
impl SPU {
    fn set_control(&mut self, data: u16) {
        self.control = SPUControl::from_bits_truncate(data);
        if !self.control.contains(SPUControl::IRQEnable) {
            // Acknowledge
            self.status.remove(SPUStatus::IRQ);
        }
        // Set mode bits.
        self.status.remove(SPUStatus::SPUMode);
        let new_mode = (self.control.intersection(SPUControl::SPUMode)).bits();
        self.status.insert(SPUStatus::from_bits_truncate(new_mode));
        // Set DMA mode.
        self.status.remove(SPUStatus::DMABits);
        self.transfer_fifo = false;
        match self.control.intersection(SPUControl::SoundRAMTransfer).bits() >> 4 {
            0b00 => {}, // Stop
            0b01 => {   // Manual
                self.transfer_fifo = true;
                self.status.insert(SPUStatus::TransferBusy);
            },
            0b10 => self.status.insert(SPUStatus::DMAWriteReq.union(SPUStatus::DMATransferReq)),
            0b11 => self.status.insert(SPUStatus::DMAReadReq.union(SPUStatus::DMATransferReq)),
            _ => unreachable!()
        }
    }

    fn write_fifo(&mut self, data: u16) {
        if self.ram_fifo.len() < SPU_FIFO_SIZE {
            self.ram_fifo.push_back(data);
        } else {
            panic!("writing too much data to SPU RAM!");
        }
    }

    fn transfer_from_fifo(&mut self) {
        if let Some(data) = self.ram_fifo.pop_front() {
            self.ram.write_halfword(self.ram_full_addr, data);
            self.ram_full_addr += 2;
        } else { // Done!
            self.status.remove(SPUStatus::TransferBusy);
            self.transfer_fifo = false;
        }
    }

    fn set_key_on_lo(&mut self, data: u16) {
        for i in 0..16 {
            if test_bit!(data, i) {
                self.voices[i].key_on();
            }
        }
    }

    fn set_key_on_hi(&mut self, data: u16) {
        for i in 0..8 {
            if test_bit!(data, i) {
                self.voices[16 + i].key_on();
            }
        }
    }

    fn set_key_off_lo(&mut self, data: u16) {
        for i in 0..16 {
            if test_bit!(data, i) {
                self.voices[i].key_off();
            }
        }
    }

    fn set_key_off_hi(&mut self, data: u16) {
        for i in 0..8 {
            if test_bit!(data, i) {
                self.voices[16 + i].key_off();
            }
        }
    }

    fn get_endx_lo(&self) -> u16 {
        let mut endx = 0;
        for i in 0..16 {
            endx |= if self.voices[i].get_endx() {1 << i} else {0};
        }
        endx
    }

    fn get_endx_hi(&self) -> u16 {
        let mut endx = 0;
        for i in 0..8 {
            endx |= if self.voices[16 + i].get_endx() {1 << i} else {0};
        }
        endx
    }

    fn set_pitch_mod_lo(&mut self, data: u16) {
        for i in 1..16 {
            self.voices[i].set_pitch_mod(test_bit!(data, i));
        }
    }

    fn set_pitch_mod_hi(&mut self, data: u16) {
        for i in 0..8 {
            self.voices[16 + i].set_pitch_mod(test_bit!(data, i));
        }
    }

    fn get_pitch_mod_lo(&self) -> u16 {
        let mut pmod = 0;
        for i in 1..16 {
            pmod |= if self.voices[i].get_pitch_mod() {1 << i} else {0};
        }
        pmod
    }

    fn get_pitch_mod_hi(&self) -> u16 {
        let mut pmod = 0;
        for i in 0..8 {
            pmod |= if self.voices[16 + i].get_pitch_mod() {1 << i} else {0};
        }
        pmod
    }

    fn set_noise_lo(&mut self, data: u16) {
        for i in 0..16 {
            self.voices[i].set_noise(test_bit!(data, i));
        }
    }

    fn set_noise_hi(&mut self, data: u16) {
        for i in 0..8 {
            self.voices[16 + i].set_noise(test_bit!(data, i));
        }
    }

    fn get_noise_lo(&self) -> u16 {
        let mut noise = 0;
        for i in 0..16 {
            noise |= if self.voices[i].get_noise() {1 << i} else {0};
        }
        noise
    }

    fn get_noise_hi(&self) -> u16 {
        let mut noise = 0;
        for i in 0..8 {
            noise |= if self.voices[16 + i].get_noise() {1 << i} else {0};
        }
        noise
    }

    /// Advance the noise. Should be called at 44.1kHz.
    fn clock_noise(&mut self) {
        let noise_step = ((self.control.intersection(SPUControl::NoiseFreqStep).bits() >> 8) + 4) as isize;
        self.noise_timer -= noise_step;
        if self.noise_timer < 0 {
            let noise_shift = self.control.intersection(SPUControl::NoiseFreqShift).bits() >> 10;
            let bit = (self.noise_level >> 15) ^
                (self.noise_level >> 12) ^
                (self.noise_level >> 11) ^
                (self.noise_level >> 10) ^ 1;
            self.noise_level = (self.noise_level << 1) | (bit & 1);
            self.noise_timer += 0x20000 >> noise_shift;
            if self.noise_timer < 0 {
                self.noise_timer += 0x20000 >> noise_shift;
            }
        }
    }

    /// Generate sample from SPU audio.
    fn generate_sample(&mut self) -> Stereo<f32> {
        if !self.control.contains(SPUControl::Enable) {
            return Stereo::EQUILIBRIUM;
        }
        
        let irq_addr = (self.ram_irq_addr as u32) * 8;
        let mut output = Stereo::<i32>::EQUILIBRIUM;
        let mut prev_voice_vol = 0;
        for voice in self.voices.iter_mut() {
            let (voice_out, irq) = voice.clock(&self.ram, irq_addr, prev_voice_vol, self.noise_level);
            output[0] += voice_out[0];
            output[1] += voice_out[1];
            if irq && self.control.contains(SPUControl::IRQEnable) && !self.status.contains(SPUStatus::IRQ) {
                self.status.insert(SPUStatus::IRQ);
                self.irq_latch = true;
            }
            prev_voice_vol = voice.get_adsr_vol();
        }

        if self.control.contains(SPUControl::CDAudioEnable) {
            let cd_audio = if let Some(cd_audio) = self.cd_audio_fifo.pop_front() {
                self.cd_audio_sample = cd_audio;
                self.cd_audio_damping = 255;
                cd_audio
            } else {
                self.cd_audio_damping = self.cd_audio_damping.saturating_sub(1);
                [
                    ((self.cd_audio_sample[0] as i32 * self.cd_audio_damping as i32) >> 8) as i16,
                    ((self.cd_audio_sample[1] as i32 * self.cd_audio_damping as i32) >> 8) as i16
                ]
            };
            output[0] += (cd_audio[0] as i32) * (self.cd_input_vol.left as i32) >> 15;
            output[1] += (cd_audio[1] as i32) * (self.cd_input_vol.right as i32) >> 15;
        }

        // TODO: reverb.

        if !self.control.contains(SPUControl::Mute) {
            Stereo::EQUILIBRIUM
        } else {
            let main_vol = self.main_vol.get_vol();
            let left = ((output[0].clamp(i16::MIN as i32, i16::MAX as i32)) * (main_vol.left as i32)) >> 15;
            let right = ((output[1].clamp(i16::MIN as i32, i16::MAX as i32)) * (main_vol.right as i32)) >> 15;
            [left as f32 / (32768.0), right as f32 / (32768.0)]
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct SPUControl: u16 {
        const Enable            = bit!(15);
        const Mute              = bit!(14);
        const NoiseFreqShift    = bits![10, 11, 12, 13];
        const NoiseFreqStep     = bits![8, 9];
        const ReverbEnable      = bit!(7);
        const IRQEnable         = bit!(6);
        const SoundRAMTransfer  = bits![4, 5];
        const ExtAudioReverb    = bit!(3);
        const CDAudioReverb     = bit!(2);
        const ExtAudioEnable    = bit!(1);
        const CDAudioEnable     = bit!(0);

        const SPUMode           = bits![0, 1, 2, 3, 4, 5];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct SPUStatus: u16 {
        const CaptureBuffers    = bit!(11);
        const TransferBusy      = bit!(10);
        const DMAReadReq        = bit!(9);
        const DMAWriteReq       = bit!(8);
        const DMATransferReq    = bit!(7);
        const IRQ               = bit!(6);
        const SPUMode           = bits![0, 1, 2, 3, 4, 5];

        const DMABits           = bits![7, 8, 9];
    }
}

#[derive(Default)]
struct StereoVolume {
    pub left: i16,
    pub right: i16,
}
