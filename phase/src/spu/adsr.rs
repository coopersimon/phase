use crate::utils::bits::*;

/// Generates a volume envelope using ADSR:
/// Attack, Decay, Sustain, Release.
#[derive(Default)]
pub struct ADSRGenerator {
    current_state: State,
    current_level: i16,
    counter: usize,

    flags: ADSR,
}

impl ADSRGenerator {
    pub fn read_adsr_lo(&self) -> u16 {
        self.flags.bits() as u16
    }
    pub fn read_adsr_hi(&self) -> u16 {
        (self.flags.bits() >> 16) as u16
    }
    pub fn write_adsr_lo(&mut self, data: u16) {
        let new_flags = (self.flags.bits() & 0xFFFF_0000) | (data as u32);
        self.flags = ADSR::from_bits_truncate(new_flags);
    }
    pub fn write_adsr_hi(&mut self, data: u16) {
        let new_flags = (self.flags.bits() & 0x0000_FFFF) | ((data as u32) << 16);
        self.flags = ADSR::from_bits_truncate(new_flags);
    }

    /// Init the volume into attack.
    pub fn init(&mut self) {
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
            Off => {
                return vol;
            },
            Attack => {
                self.counter += 1;
                let base_cycles = self.flags.attack_cycles();
                let attack_cycles = if self.flags.attack_exp() && self.current_level > 0x6000 {
                    base_cycles * 4
                } else {
                    base_cycles
                };
                if self.counter >= attack_cycles {
                    self.current_level = self.current_level.saturating_add(self.flags.attack_step());
                    if self.current_level == 0x7FFF {
                        self.current_state = Decay;
                    }
                    self.counter = 0;
                }
            },
            Decay => {
                self.counter += 1;
                if self.counter >= self.flags.decay_cycles() {
                    self.current_level = self.current_level.saturating_add(self.flags.decay_step()).max(0);
                    if self.current_level <= self.flags.sustain_level() {
                        self.current_state = Sustain;
                        self.current_level = self.flags.sustain_level();
                    }
                    self.counter = 0;
                }
            },
            Sustain => {
                self.counter += 1;
                let base_cycles = self.flags.sustain_cycles();
                let sustain_cycles = if self.flags.sustain_exp() && !self.flags.sustain_decrease() && self.current_level > 0x6000 {
                    base_cycles * 4
                } else {
                    base_cycles
                };
                if self.counter >= sustain_cycles {
                    let base_step = self.flags.sustain_step();
                    let sustain_step = if self.flags.sustain_exp() && self.flags.sustain_decrease() {
                        ((base_step as i32 * self.current_level as i32) >> 15) as i16
                    } else {
                        base_step
                    };
                    self.current_level = self.current_level.saturating_add(sustain_step).max(0);
                    // TODO: switch to off..?
                    self.counter = 0;
                }
            },
            Release => {
                self.counter += 1;
                if self.counter >= self.flags.release_cycles() {
                    let base_step = self.flags.release_step();
                    let release_step = if self.flags.release_exp() {
                        ((base_step as i32 * self.current_level as i32) >> 15) as i16
                    } else {
                        base_step
                    };
                    self.current_level = self.current_level.saturating_add(release_step).max(0);
                    // TODO: switch to off..?
                    self.counter = 0;
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

    /// Release and force env to 0.
    pub fn end(&mut self) {
        self.release();
        self.current_level = 0;
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
    struct ADSR: u32 {
        const SustainExponential = bit!(31);
        const SustainDecrease    = bit!(30);
        const SustainShift       = bits![24, 25, 26, 27, 28];
        const SustainStep        = bits![22, 23];
        const ReleaseExponential = bit!(21);
        const ReleaseShift       = bits![16, 17, 18, 19, 20];

        const AttackExponential = bit!(15);
        const AttackShift       = bits![10, 11, 12, 13, 14];
        const AttackStep        = bits![8, 9];
        const DecayShift        = bits![4, 5, 6, 7];
        const SustainLevel      = bits![0, 1, 2, 3];
    }
}

impl ADSR {
    #[inline]
    const fn attack_shift(&self) -> i16 {
        (self.intersection(ADSR::AttackShift).bits() >> 10) as i16
    }
    #[inline]
    fn attack_step(&self) -> i16 {
        let step = 7 - (self.intersection(ADSR::AttackStep).bits() >> 8) as i16;
        let shift = (11 - self.attack_shift()).max(0);
        step << shift
    }
    #[inline]
    fn attack_cycles(&self) -> usize {
        1 << (self.attack_shift() - 11).max(0)
    }
    #[inline]
    const fn attack_exp(&self) -> bool {
        self.contains(ADSR::AttackExponential)
    }

    #[inline]
    const fn decay_shift(&self) -> i16 {
        (self.intersection(ADSR::DecayShift).bits() >> 4) as i16
    }
    #[inline]
    fn decay_step(&self) -> i16 {
        let step = -8;
        let shift = (11 - self.decay_shift()).max(0);
        step << shift
    }
    #[inline]
    fn decay_cycles(&self) -> usize {
        1 << (self.decay_shift() - 11).max(0)
    }

    #[inline]
    fn sustain_level(&self) -> i16 {
        let level = self.intersection(ADSR::SustainLevel).bits() + 1;
        (level * 0x800).min(0x7FFF) as i16
    }
    #[inline]
    const fn sustain_shift(&self) -> i16 {
        (self.intersection(ADSR::SustainShift).bits() >> 24) as i16
    }
    #[inline]
    fn sustain_step(&self) -> i16 {
        let step_base = (self.intersection(ADSR::SustainStep).bits() >> 22) as i16;
        let step = if self.sustain_decrease() {
            -8 + step_base
        } else {
            7 - step_base
        };
        let shift = (11 - self.sustain_shift()).max(0);
        step << shift
    }
    #[inline]
    fn sustain_cycles(&self) -> usize {
        1 << (self.sustain_shift() - 11).max(0)
    }
    #[inline]
    const fn sustain_exp(&self) -> bool {
        self.contains(ADSR::SustainExponential)
    }
    #[inline]
    const fn sustain_decrease(&self) -> bool {
        self.contains(ADSR::SustainDecrease)
    }

    #[inline]
    const fn release_shift(&self) -> i16 {
        (self.intersection(ADSR::ReleaseShift).bits() >> 16) as i16
    }
    #[inline]
    fn release_step(&self) -> i16 {
        let step = -8;
        let shift = (11 - self.release_shift()).max(0);
        step << shift
    }
    #[inline]
    fn release_cycles(&self) -> usize {
        1 << (self.release_shift() - 11).max(0)
    }
    #[inline]
    const fn release_exp(&self) -> bool {
        self.contains(ADSR::ReleaseExponential)
    }
}
