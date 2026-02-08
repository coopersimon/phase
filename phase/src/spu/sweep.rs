use crate::utils::bits::*;
use super::StereoVolume;

#[derive(Default)]
pub struct SweepVolume {
    left: Sweep,
    right: Sweep,
}

impl SweepVolume {
    pub fn set_left(&mut self, data: u16) {
        self.left.set(data as i16);
    }

    pub fn set_right(&mut self, data: u16) {
        self.right.set(data as i16);
    }

    pub fn get_left(&self) -> u16 {
        self.left.current as u16
    }

    pub fn get_right(&self) -> u16 {
        self.right.current as u16
    }

    pub fn get_vol(&mut self) -> StereoVolume {
        let left = self.left.get_vol();
        let right = self.right.get_vol();
        StereoVolume { left, right }
    }
}

#[derive(Default)]
struct Sweep {
    settings: VolumeSweepSettings,
    current: i16,
    step: i16,
    counter: usize,
    mod_count: usize,
}

impl Sweep {
    fn set(&mut self, data: i16) {
        self.settings = VolumeSweepSettings::from_bits_retain(data);
        if self.settings.contains(VolumeSweepSettings::Sweep) {
            self.current = if self.settings.contains(VolumeSweepSettings::Direction) {0x7FFF} else {0};
            let shift = self.settings.intersection(VolumeSweepSettings::Shift).bits() >> 2;
            self.mod_count = (1 << (shift - 11).max(0)) as usize;
            self.step = self.settings.base_step_value() << (11 - shift).max(0);
            self.counter = 0;
        } else {
            self.current = data;
        }
    }

    fn get_vol(&mut self) -> i16 {
        if self.settings.contains(VolumeSweepSettings::Sweep) {
            let out = if self.settings.contains(VolumeSweepSettings::Phase) {-self.current} else {self.current};
            self.counter += 1;
            if self.counter >= self.mod_count {
                if self.settings.contains(VolumeSweepSettings::Mode) { // Exponential
                    if !self.settings.contains(VolumeSweepSettings::Direction) && self.current > 0x6000 {
                        self.mod_count = self.mod_count * 4;
                    }
                    if self.settings.contains(VolumeSweepSettings::Direction) {
                        let new_step = ((self.step as i32) * (self.current as i32)) >> 15;
                        self.step = new_step as i16;
                    }
                }
                let new_value = self.current.saturating_add(self.step);
                self.current = new_value.max(0);
                self.counter = 0;
            }
            out
        } else {
            self.current << 1
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Default)]
    pub struct VolumeSweepSettings: i16 {
        const Sweep     = bit!(15);
        const Mode      = bit!(14);
        const Direction = bit!(13);
        const Phase     = bit!(12);
        const Shift     = bits![2, 3, 4, 5, 6];
        const Step      = bits![0, 1];
    }
}

impl VolumeSweepSettings {
    fn base_step_value(&self) -> i16 {
        if self.contains(VolumeSweepSettings::Direction) {
            match self.intersection(VolumeSweepSettings::Step).bits() {
                0b00 => -8,
                0b01 => -7,
                0b10 => -6,
                0b11 => -5,
                _ => unreachable!()
            }
        } else {
            match self.intersection(VolumeSweepSettings::Step).bits() {
                0b00 => 7,
                0b01 => 6,
                0b10 => 5,
                0b11 => 4,
                _ => unreachable!()
            }
        }
    }
}