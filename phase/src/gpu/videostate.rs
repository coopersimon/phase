// This file manages timing of the video system.

use crate::{interrupt::Interrupt, utils::bits::*};
use super::InterlaceState;

/// Returned when clocking the GPU.
/// 
/// Indicates state of interrupt / blanking.
#[derive(Default)]
pub struct GPUClockRes {
    pub irq: Interrupt,
    pub in_v_blank: bool,
    pub in_h_blank: bool,
    pub dots: usize,
    pub new_frame: bool,
}

impl GPUClockRes {
    fn dots(dots: usize) -> Self {
        Self {
            irq: Interrupt::empty(),
            in_v_blank: false,
            in_h_blank: false,
            dots,
            new_frame: false,
        }
    }
    fn enter_v_blank(dots: usize) -> Self {
        Self {
            irq: Interrupt::VBLANK,
            in_v_blank: true,
            in_h_blank: false,
            dots,
            new_frame: false,
        }
    }
    fn v_blank(dots: usize) -> Self {
        Self {
            irq: Interrupt::empty(),
            in_v_blank: true,
            in_h_blank: false,
            dots,
            new_frame: false,
        }
    }
    fn h_blank(dots: usize) -> Self {
        Self {
            irq: Interrupt::empty(),
            in_v_blank: false,
            in_h_blank: true,
            dots,
            new_frame: false,
        }
    }
    fn vh_blank(dots: usize) -> Self {
        Self {
            irq: Interrupt::empty(),
            in_v_blank: true,
            in_h_blank: true,
            dots,
            new_frame: false,
        }
    }
    fn exit_v_blank(dots: usize) -> Self {
        Self {
            irq: Interrupt::empty(),
            in_v_blank: false,
            in_h_blank: false,
            dots,
            new_frame: true,
        }
    }
}

/// Tracks cycle count and current state of the LCD drawing process.
pub struct StateMachine {
    state:          VideoState,
    h_cycle_count:  usize,
    h_dot:          usize,
    v_count:        usize,

    // Timing constants:
    v_res:          usize,
    h_res:          usize,
    cycles_per_dot: usize,
    h_draw_cycles:  usize,
    h_total_cycles: usize,

    interlace:      InterlaceState,
}

impl StateMachine {
    pub fn new() -> Self {
        let mut machine = Self {
            state:          VideoState::Drawing,
            h_cycle_count:  0,
            h_dot:          0,
            v_count:        0,

            v_res:          0,
            h_res:          0,
            cycles_per_dot: 0,
            h_draw_cycles:  0,
            h_total_cycles: 0,

            interlace:      InterlaceState::Off,
        };
        machine.set_h_res_ntsc(256);
        machine
    }

    /// Set new horizontal resolution using NTSC timings.
    pub fn set_h_res_ntsc(&mut self, h_res: usize) {
        self.v_res = ntsc::SCANLINES;
        self.h_total_cycles = ntsc::H_CYCLES;
        self.h_res = h_res;
        self.cycles_per_dot = match h_res {
            256 => ntsc::DOT_COUNT_256,
            320 => ntsc::DOT_COUNT_320,
            368 => ntsc::DOT_COUNT_368,
            512 => ntsc::DOT_COUNT_512,
            640 => ntsc::DOT_COUNT_640,
            _ => panic!("invalid horizontal resolution specified!"),
        };
        self.h_draw_cycles = self.cycles_per_dot * self.h_res;
    }

    /// Set or unset interlace mode.
    pub fn set_interlace(&mut self, interlace: bool) {
        if interlace {
            if self.interlace == InterlaceState::Off {
                self.interlace = InterlaceState::Odd;
            }
        } else {
            self.interlace = InterlaceState::Off;
        }
    }

    /// Advance the state machine.
    pub fn clock(&mut self, cycles: usize) -> GPUClockRes {
        use VideoState::*;
        self.h_cycle_count += cycles;
        let new_dot_count = self.h_cycle_count / self.cycles_per_dot;
        let dots = new_dot_count - self.h_dot;
        self.h_dot = new_dot_count;

        match self.state {
            Drawing => {
                if self.h_cycle_count >= self.h_draw_cycles {
                    self.state = HBlank;
                    GPUClockRes::h_blank(dots)
                } else {
                    GPUClockRes::dots(dots)
                }
            },
            VBlank => {
                if self.h_cycle_count >= self.h_draw_cycles {
                    self.state = VHBlank;
                    GPUClockRes::vh_blank(dots)
                } else {
                    GPUClockRes::v_blank(dots)
                }
            },
            HBlank => {
                if self.h_cycle_count >= self.h_total_cycles {
                    self.h_cycle_count -= self.h_total_cycles;
                    self.v_count += 1;
                    self.h_dot = self.h_cycle_count / self.cycles_per_dot;
                    if self.v_count < DRAWLINES {
                        self.state = Drawing;
                        GPUClockRes::dots(dots)
                    } else {
                        self.state = VBlank;
                        GPUClockRes::enter_v_blank(dots)
                    }
                } else {
                    GPUClockRes::h_blank(dots)
                }
            },
            VHBlank => {
                if self.h_cycle_count >= self.h_total_cycles {
                    self.h_cycle_count -= self.h_total_cycles;
                    self.v_count += 1;
                    self.h_dot = self.h_cycle_count / self.cycles_per_dot;
                    if self.v_count < self.v_res {
                        self.state = VBlank;
                        GPUClockRes::v_blank(dots)
                    } else {
                        // Exit vblank
                        self.v_count = 0;
                        self.state = Drawing;
                        self.toggle_interlace();
                        GPUClockRes::exit_v_blank(dots)
                    }
                } else {
                    GPUClockRes::vh_blank(dots)
                }
            }
        }
    }

    /// Get the interlace bit status.
    pub fn get_interlace_bit(&self) -> bool {
        use InterlaceState::*;
        if self.v_count >= self.v_res {
            return false;
        }
        match self.interlace {
            Off => test_bit!(self.v_count, 0),
            Even => false,
            Odd => true,
        }
    }

    pub fn get_interlace_state(&self) -> InterlaceState {
        self.interlace
    }

    fn toggle_interlace(&mut self) {
        self.interlace = self.interlace.toggle();
    }
}

/// State of the screen drawing process.
enum VideoState {
    Drawing,    // Drawing a line.
    HBlank,     // Horizontal blanking period.
    VBlank,     // Vertical blanking period.
    VHBlank,    // Horizontal blanking period during v-blank.
}

/// Lines to draw per frame.
const DRAWLINES: usize = 240;

/// NTSC timings
mod ntsc {
    pub const SCANLINES: usize = 263;
    pub const H_CYCLES: usize = 3413;

    pub const DOT_COUNT_256: usize = 10;
    pub const DOT_COUNT_320: usize = 8;
    pub const DOT_COUNT_368: usize = 7;
    pub const DOT_COUNT_512: usize = 5;
    pub const DOT_COUNT_640: usize = 4;
}

/// PAL timings (TODO)
mod pal {

}