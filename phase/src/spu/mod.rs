mod voice;
pub mod resampler;

use std::collections::VecDeque;

use crossbeam_channel::Sender;
use dasp::frame::Stereo;
use mips::mem::Data;

use crate::{
    interrupt::Interrupt,
    mem::{DMADevice, ram::RAM},
    utils::{bits::*, interface::MemInterface}
};

use voice::Voice;
use resampler::*;

#[derive(Default)]
struct StereoVolume {
    left: u16,
    right: u16,
}

const SPU_RAM_SIZE: usize = 512 * 1024;
const SPU_FIFO_SIZE: usize = 32;

const CYCLES_PER_SAMPLE: usize = 0x300;
const SAMPLE_PACKET_SIZE: usize = 32;

/// Cycles per second.
const CLOCK_RATE: usize = CYCLES_PER_SAMPLE * 44100;
/// Emulated cycles per second.
/// TODO: PAL (get these values from GPU)
const REAL_CLOCK_RATE: f64 = 3413. * 263. * 60. * 7. / 11.;

/// Base sample rate for audio.
const BASE_SAMPLE_RATE: f64 = 44_100.0;

const REAL_SAMPLE_RATE_RATIO: f64 = REAL_CLOCK_RATE / (CLOCK_RATE as f64);
pub const REAL_BASE_SAMPLE_RATE: f64 = BASE_SAMPLE_RATE * REAL_SAMPLE_RATE_RATIO;


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

    main_vol:       StereoVolume,
    cd_input_vol:   StereoVolume,
    ext_input_vol:  StereoVolume,
    reverb_vol:     StereoVolume,

    control:    SPUControl,
    status:     SPUStatus,

    // Sample generation:
    cycle_count: usize,

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

            cycle_count: 0,

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

    pub fn clock(&mut self, cycles: usize) -> Interrupt {
        if self.transfer_fifo {
            self.transfer_from_fifo();
        }

        // TODO: is this a bit intensive..?
        for voice in self.voices.iter_mut() {
            voice.clock(cycles);
        }

        self.cycle_count += cycles;
        if self.cycle_count > CYCLES_PER_SAMPLE {
            self.cycle_count -= CYCLES_PER_SAMPLE;

            // Generate sample
            let sample = self.generate_sample();
            // TODO:
            self.sample_buffer.push(sample);
            
            // Output to audio thread
            if self.sample_buffer.len() >= SAMPLE_PACKET_SIZE {
                let sample_packet = std::mem::replace(&mut self.sample_buffer, Vec::with_capacity(SAMPLE_PACKET_SIZE)).into_boxed_slice();
                if let Some(s) = &self.sample_sender {
                    let _ = s.send(sample_packet);
                }
            }
        }

        // TODO: latch IRQ.
        if self.control.contains(SPUControl::Enable.union(SPUControl::IRQEnable)) &&
            self.status.contains(SPUStatus::IRQ) {
            Interrupt::SPU
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
            0x1F80_1D80 => self.main_vol.left,
            0x1F80_1D82 => self.main_vol.right,
            0x1F80_1D84 => self.reverb_vol.left,
            0x1F80_1D86 => self.reverb_vol.right,
            0x1F80_1D88 => 0, // TODO:KON flags
            0x1F80_1D8A => 0, // TODO:KON flags
            0x1F80_1D8C => 0, // TODO:KOFF flags
            0x1F80_1D8E => 0, // TODO:KOFF flags
            0x1F80_1D90 => 0, // TODO:PMOD flags
            0x1F80_1D92 => 0, // TODO:PMOD flags
            0x1F80_1D94 => 0, // TODO:Noise flags
            0x1F80_1D96 => 0, // TODO:Noise flags
            0x1F80_1D98 => 0, // TODO:Echo flags
            0x1F80_1D9A => 0, // TODO:Echo flags
            0x1F80_1D9C => 0, // TODO:ENDX flags
            0x1F80_1D9E => 0, // TODO:ENDX flags
            0x1F80_1DA2 => 0, // TODO: reverb base
            0x1F80_1DA4 => self.ram_irq_addr,
            0x1F80_1DA6 => self.ram_addr,
            0x1F80_1DAA => self.control.bits(),
            0x1F80_1DAC => self.ram_ctrl,
            0x1F80_1DAE => self.status.bits(),
            0x1F80_1DB0 => self.cd_input_vol.left,
            0x1F80_1DB2 => self.cd_input_vol.right,
            0x1F80_1DB4 => self.ext_input_vol.left,
            0x1F80_1DB6 => self.ext_input_vol.right,
            0x1F80_1DB8 => 0, // TODO: current main volume.
            0x1F80_1DBA => 0, // TODO: current main volume.
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
            0x1F80_1D80 => self.main_vol.left = data,
            0x1F80_1D82 => self.main_vol.right = data,
            0x1F80_1D84 => self.reverb_vol.left = data,
            0x1F80_1D86 => self.reverb_vol.right = data,
            0x1F80_1D88 => {}, // TODO:KON flags
            0x1F80_1D8A => {}, // TODO:KON flags
            0x1F80_1D8C => {}, // TODO:KOFF flags
            0x1F80_1D8E => {}, // TODO:KOFF flags
            0x1F80_1D90 => {}, // TODO:PMOD flags
            0x1F80_1D92 => {}, // TODO:PMOD flags
            0x1F80_1D94 => {}, // TODO:Noise flags
            0x1F80_1D96 => {}, // TODO:Noise flags
            0x1F80_1D98 => {}, // TODO:Echo flags
            0x1F80_1D9A => {}, // TODO:Echo flags
            0x1F80_1D9C => {}, // TODO:ENDX flags
            0x1F80_1D9E => {}, // TODO:ENDX flags
            0x1F80_1DA2 => {}, // TODO: reverb base
            0x1F80_1DA4 => self.ram_irq_addr = data,
            0x1F80_1DA6 => {
                self.ram_addr = data;
                self.ram_full_addr = (self.ram_addr as u32) << 3;
            },
            0x1F80_1DA8 => self.write_fifo(data),
            0x1F80_1DAA => self.set_control(data),
            0x1F80_1DAC => self.ram_ctrl = data,
            0x1F80_1DB0 => self.cd_input_vol.left = data,
            0x1F80_1DB2 => self.cd_input_vol.right = data,
            0x1F80_1DB4 => self.ext_input_vol.left = data,
            0x1F80_1DB6 => self.ext_input_vol.right = data,
            0x1F80_1DC0..=0x1F80_1DFF => { // Reverb
                
            },
            _ => panic!("invalid SPU write {:X}", addr)
        }
    }

    // Usually SPU should not be accessed via word interface.

    fn read_word(&mut self, addr: u32) -> u32 {
        let lo = self.read_halfword(addr) as u32;
        let hi = self.read_halfword(addr + 2) as u32;
        lo | (hi << 16)
    }

    fn write_word(&mut self, addr: u32, data: u32) {
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

    fn generate_sample(&mut self) -> Stereo<f32> {
        if !self.control.contains(SPUControl::Enable) {
            return [0.0, 0.0];
        }

        let irq_addr = (self.ram_irq_addr as u32) * 8;
        let mut output = (0, 0);
        for voice in self.voices.iter() {
            let voice_out = voice.get_sample(&self.ram, irq_addr);
            output.0 += voice_out.0;
            output.1 += voice_out.1;
        }

        [0.0, 0.0]
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
