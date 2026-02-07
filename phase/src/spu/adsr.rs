use crate::utils::bits::*;

/// Generates a volume envelope using ADSR:
/// Attack, Decay, Sustain, Release.
#[derive(Default)]
pub struct ADSRGenerator {
    current_state: State,
    current_level: i16,
    counter: usize,

    attack_count: usize,
    attack_hi_exp_count: usize,
    attack_step: i16,

    decay_count: usize,
    decay_step: i16,

    sustain_count: usize,
    sustain_hi_exp_count: usize,
    sustain_level: i16,
    sustain_step: i16,
    sustain_exp_dec: bool,

    release_count: usize,
    release_hi_exp_count: usize,
    release_step: i16,
    release_exp_dec: bool,
}

impl ADSRGenerator {
    /// Init the volume into attack.
    pub fn init(&mut self, lo: u16, hi: u16) {
        let lo_flags = ADSRLo::from_bits_retain(lo);
        let hi_flags = ADSRHi::from_bits_retain(hi);

        let attack_exp = lo_flags.contains(ADSRLo::AttackExponential);
        let attack_shift = (lo_flags.intersection(ADSRLo::AttackShift).bits() >> 10) as i8;
        self.attack_count = 1 << (attack_shift - 11).max(0);
        self.attack_hi_exp_count = if attack_exp {
            self.attack_count * 4
        } else {
            self.attack_count
        };
        self.attack_step = match lo_flags.intersection(ADSRLo::AttackStep).bits() >> 8 {
            0b00 => 7,
            0b01 => 6,
            0b10 => 5,
            0b11 => 4,
            _ => unreachable!()
        } << (11 - attack_shift).max(0);

        let decay_shift = (lo_flags.intersection(ADSRLo::DecayShift).bits() >> 4) as i8;
        self.decay_count = 1 << (decay_shift - 11).max(0);
        self.decay_step = -8 << (11 - decay_shift).max(0);

        self.sustain_level = ((lo_flags.intersection(ADSRLo::SustainLevel).bits() + 1) * 0x800).min(0x7FFF) as i16;
        let sustain_exp = hi_flags.contains(ADSRHi::SustainExponential);
        let sustain_dec = hi_flags.contains(ADSRHi::SustainDecrease);
        let sustain_shift = (hi_flags.intersection(ADSRHi::SustainShift).bits() >> 8) as i8;
        self.sustain_count = 1 << (sustain_shift - 11).max(0);
        self.sustain_hi_exp_count = if sustain_exp {
            self.sustain_count * 4
        } else {
            self.sustain_count
        };
        self.sustain_step = if sustain_dec {
            match hi_flags.intersection(ADSRHi::SustainStep).bits() >> 6 {
                0b00 => -8,
                0b01 => -7,
                0b10 => -6,
                0b11 => -5,
                _ => unreachable!()
            }
        } else {
            match hi_flags.intersection(ADSRHi::SustainStep).bits() >> 6 {
                0b00 => 7,
                0b01 => 6,
                0b10 => 5,
                0b11 => 4,
                _ => unreachable!()
            }
        } << (11 - sustain_shift).max(0);
        self.sustain_exp_dec = sustain_dec && sustain_exp;

        let release_exp = hi_flags.contains(ADSRHi::ReleaseExponential);
        let release_shift = hi_flags.intersection(ADSRHi::ReleaseShift).bits() as i8;
        self.release_count = 1 << (release_shift - 11).max(0);
        self.release_hi_exp_count = if release_exp {
            self.release_count * 4
        } else {
            self.release_count
        };
        self.release_exp_dec = release_exp;
        self.release_step = -8 << (11 - release_shift).max(0);

        self.current_state = State::Attack;
        self.current_level = 0;
        self.counter = 0;
    }

    /// Step the envelope and get the new volume.
    pub fn step(&mut self) -> i16 {
        use State::*;
        //println!("ADSR step {:?} lvl {:X}", self.current_state, self.current_level);
        let vol = self.current_level;
        match self.current_state {
            Off => {},
            Attack => {
                self.counter += 1;
                if self.counter >= self.attack_count {
                    self.counter = 0;
                    let (new_level, ovf) = self.current_level.overflowing_add(self.attack_step);
                    if new_level > 0x6000 {
                        self.attack_count = self.attack_hi_exp_count;
                    }
                    if ovf {
                        self.current_level = 0x7FFF;
                        self.current_state = Decay;
                    } else {
                        self.current_level = new_level;
                    }
                }
            },
            Decay => {
                self.counter += 1;
                if self.counter >= self.decay_count {
                    self.counter = 0;
                    let new_level = self.current_level - 8;
                    if new_level <= self.sustain_level {
                        self.current_level = self.sustain_level;
                        self.current_state = Sustain;
                    } else {
                        self.current_level = new_level;
                    }
                }
            },
            Sustain => {
                self.counter += 1;
                if self.counter >= self.sustain_count {
                    self.counter = 0;
                    if self.sustain_exp_dec {
                        let new_step = (self.sustain_step as i32 * self.current_level as i32) >> 15;
                        self.sustain_step = new_step as i16;
                    }
                    let (new_level, ovf) = self.current_level.overflowing_add(self.sustain_step);
                    if !self.sustain_exp_dec && new_level > 0x6000 {
                        self.sustain_count = self.sustain_hi_exp_count;
                    }
                    if ovf {
                        self.current_level = 0x7FFF;
                        self.current_state = Off;
                    } else if new_level < 0 {
                        self.current_level = 0;
                        self.current_state = Off;
                    } else {
                        self.current_level = new_level;
                    }
                }
            },
            Release => {
                self.counter += 1;
                if self.counter >= self.release_count {
                    self.counter = 0;
                    if self.release_exp_dec {
                        let new_step = (self.release_step as i32 * self.current_level as i32) >> 15;
                        self.release_step = new_step as i16;
                    }
                    let new_level = self.current_level + self.release_step;
                    if new_level <= 0 {
                        self.current_level = 0;
                        self.current_state = Off;
                    } else {
                        self.current_level = new_level;
                    }
                }
            }
        }
        vol
    }

    /// Force into release mode.
    pub fn release(&mut self) {
        self.counter = 0;
        self.current_state = State::Release;
    }

    pub fn is_off(&self) -> bool {
        self.current_state == State::Off
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
enum State {
    #[default]
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Default)]
    pub struct ADSRLo: u16 {
        const AttackExponential = bit!(15);
        const AttackShift       = bits![10, 11, 12, 13, 14];
        const AttackStep        = bits![8, 9];
        const DecayShift        = bits![4, 5, 6, 7];
        const SustainLevel      = bits![0, 1, 2, 3];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Default)]
    pub struct ADSRHi: u16 {
        const SustainExponential = bit!(15);
        const SustainDecrease    = bit!(14);
        const SustainShift       = bits![8, 9, 10, 11, 12];
        const SustainStep        = bits![6, 7];
        const ReleaseExponential = bit!(5);
        const ReleaseShift       = bits![0, 1, 2, 3, 4];
    }
}
