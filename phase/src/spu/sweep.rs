use crate::utils::bits::*;
use super::StereoVolume;

#[derive(Default)]
pub struct SweepVolume {
    pub left: i16,
    pub right: i16,

    sweep_left_current: i16,
    sweep_left_step: i16,
    sweep_left_min: i16,
    sweep_left_max: i16,
    sweep_left_exp_counter: i16,

    sweep_right_current: i16,
    sweep_right_step: i16,
    sweep_right_min: i16,
    sweep_right_max: i16,
    sweep_right_exp_counter: i16,
}

impl SweepVolume {
    pub fn set_left(&mut self, data: u16) {
        self.left = data as i16;
        let settings = VolumeSweepSettings::from_bits_retain(self.left);
        if settings.contains(VolumeSweepSettings::Sweep) {
            self.sweep_left_current = settings.start_vol();
            self.sweep_left_min = settings.min();
            self.sweep_left_max = settings.max();
            self.sweep_left_step = settings.step_value();
            self.sweep_left_exp_counter = 0;
        }
    }

    pub fn set_right(&mut self, data: u16) {
        self.right = data as i16;
        let settings = VolumeSweepSettings::from_bits_retain(self.right);
        if settings.contains(VolumeSweepSettings::Sweep) {
            self.sweep_right_current = settings.start_vol();
            self.sweep_right_min = settings.min();
            self.sweep_right_max = settings.max();
            self.sweep_right_step = settings.step_value();
            self.sweep_right_exp_counter = 0;
        }
    }

    pub fn get_vol(&mut self) -> StereoVolume {
        return StereoVolume { left: 0x7FFF, right: 0x7FFF };
        // TODO: fix the below.
        let left_settings = VolumeSweepSettings::from_bits_retain(self.left);
        let left = if left_settings.contains(VolumeSweepSettings::Sweep) {
            let out = self.sweep_left_current;
            if left_settings.contains(VolumeSweepSettings::Mode) {
                // Exponential
                self.sweep_left_exp_counter += 1;
                if self.sweep_left_exp_counter > self.sweep_left_step {
                    self.sweep_left_exp_counter = 0;
                    if left_settings.contains(VolumeSweepSettings::Direction) {
                        self.sweep_left_current = (self.sweep_left_current >> 1)
                            .clamp(self.sweep_left_min, self.sweep_left_max);
                    } else {
                        self.sweep_left_current = (self.sweep_left_current << 1)
                            .clamp(self.sweep_left_min, self.sweep_left_max);
                    }
                }
            } else {
                // Linear
                self.sweep_left_current = self.sweep_left_current
                    .saturating_add(self.sweep_left_step)
                    .clamp(self.sweep_left_min, self.sweep_left_max);
            }
            out
        } else {
            self.left << 1
        };
        let right_settings = VolumeSweepSettings::from_bits_retain(self.right);
        let right = if right_settings.contains(VolumeSweepSettings::Sweep) {
            let out = self.sweep_right_current;
            if right_settings.contains(VolumeSweepSettings::Mode) {
                // Exponential
                self.sweep_right_exp_counter += 1;
                if self.sweep_right_exp_counter > self.sweep_right_step {
                    self.sweep_right_exp_counter = 0;
                    if right_settings.contains(VolumeSweepSettings::Direction) {
                        self.sweep_right_current = (self.sweep_right_current >> 1)
                            .clamp(self.sweep_right_min, self.sweep_right_max);
                    } else {
                        self.sweep_right_current = (self.sweep_right_current << 1)
                            .clamp(self.sweep_right_min, self.sweep_right_max);
                    }
                }
            } else {
                // Linear
                self.sweep_right_current = self.sweep_right_current
                    .saturating_add(self.sweep_right_step)
                    .clamp(self.sweep_right_min, self.sweep_right_max);
            }
            out
        } else {
            self.right << 1
        };
        StereoVolume { left, right }
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
    fn start_vol(&self) -> i16 {
        if self.contains(VolumeSweepSettings::Direction) {
            if self.contains(VolumeSweepSettings::Phase) {
                -0x7FFF
            } else {
                0x7FFF
            }
        } else {
            0
        }
    }

    fn min(&self) -> i16 {
        if self.contains(VolumeSweepSettings::Phase) {
            -0x7FFF
        } else {
            if self.contains(VolumeSweepSettings::Mode) && !self.contains(VolumeSweepSettings::Direction) {
                // When exponentially increasing, we need to set a min of 1 to ensure we set the bit.
                1
            } else {
                0
            }
        }
    }

    fn max(&self) -> i16 {
        if self.contains(VolumeSweepSettings::Phase) {
            0
        } else {
            0x7FFF
        }
    }

    fn step_value(&self) -> i16 {
        if self.contains(VolumeSweepSettings::Mode) {
            // Exponential:
            self.intersection(VolumeSweepSettings::Shift).bits() >> 2
        } else {
            // Linear:
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
}