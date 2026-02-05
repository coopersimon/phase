use crate::{gpu::GPUClockRes, interrupt::Interrupt, utils::{bits::*, interface::MemInterface}};

/// Timers for PSX.
pub struct Timers {
    timers: [Timer; 3],
    in_h_blank: bool,
    in_v_blank: bool,

    clock_div:  usize,
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [
                Timer::new(true),
                Timer::new(true),
                Timer::new(false),
            ],
            in_h_blank: false,
            in_v_blank: false,

            clock_div: 0,
        }
    }

    pub fn clock(&mut self, cycles: usize, gpu: &GPUClockRes) -> Interrupt {
        let mut interrupt = Interrupt::empty();
        
        let entered_h_blank = self.set_blanks(gpu.in_h_blank, gpu.in_v_blank);
        if self.timers[0].use_sys_clock() {
            if self.timers[0].clock(cycles) {
                interrupt.insert(Interrupt::Timer0);
            }
        } else { // Dot clock.
            if self.timers[0].clock(gpu.dots) {
                interrupt.insert(Interrupt::Timer0);
            }
        }
        if self.timers[1].use_sys_clock() {
            if self.timers[1].clock(cycles) {
                interrupt.insert(Interrupt::Timer1);
            }
        } else if entered_h_blank { // H-blank.
            if self.timers[1].clock(1) {
                interrupt.insert(Interrupt::Timer1);
            }
        }
        if self.timers[2].use_sys_clock() {
            if self.timers[2].clock(cycles) {
                interrupt.insert(Interrupt::Timer2);
            }
        } else { // Clock / 8.
            self.clock_div += cycles;
            if self.clock_div >= 8 {
                if self.timers[2].clock(self.clock_div / 8) {
                    interrupt.insert(Interrupt::Timer2);
                }
                self.clock_div = self.clock_div % 8;
            }
        }
        interrupt
    }

    /// Update current h- and v-blank status.
    /// 
    /// Returns true if h-blank has been entered.
    fn set_blanks(&mut self, in_h_blank: bool, in_v_blank: bool) -> bool {
        let entered_h_blank = !self.in_h_blank && in_h_blank;
        if entered_h_blank {
            self.timers[0].blank_begin();
        }
        if self.in_h_blank && !in_h_blank {
            self.timers[0].blank_end();
        }
        if !self.in_v_blank && in_v_blank {
            self.timers[1].blank_begin();
        }
        if self.in_v_blank && !in_v_blank {
            self.timers[1].blank_end();
        }
        self.in_h_blank = in_h_blank;
        self.in_v_blank = in_v_blank;
        entered_h_blank
    }
}

impl MemInterface for Timers {
    fn read_word(&mut self, addr: u32) -> u32 {
        self.read_halfword(addr) as u32
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        self.write_halfword(addr, data as u16);
    }

    fn read_halfword(&mut self, addr: u32) -> u16 {
        let data = match addr {
            0x1F801100 => self.timers[0].counter,
            0x1F801104 => self.timers[0].read_mode(),
            0x1F801108 => self.timers[0].target,

            0x1F801110 => self.timers[1].counter,
            0x1F801114 => self.timers[1].read_mode(),
            0x1F801118 => self.timers[1].target,

            0x1F801120 => self.timers[2].counter,
            0x1F801124 => self.timers[2].read_mode(),
            0x1F801128 => self.timers[2].target,

            _ => panic!("invalid timer addr {:X}", addr),
        };
        //println!("timer read {:X} from {:X}", data, addr);
        data
    }
    
    fn write_halfword(&mut self, addr: u32, data: u16) {
        //println!("timer write {:X} to {:X}", data, addr);
        match addr {
            0x1F801100 => self.timers[0].counter = 0,
            0x1F801104 => self.timers[0].write_mode(data),
            0x1F801108 => self.timers[0].target = data,

            0x1F801110 => self.timers[1].counter = 0,
            0x1F801114 => self.timers[1].write_mode(data),
            0x1F801118 => self.timers[1].target = data,

            0x1F801120 => self.timers[2].counter = 0,
            0x1F801124 => self.timers[2].write_mode(data),
            0x1F801128 => self.timers[2].target = data,

            _ => panic!("invalid timer addr {:X}", addr),
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct TimerMode: u16 {
        const ReachedMax    = bit!(12);
        const ReachedTarget = bit!(11);
        const IRQ           = bit!(10);
        const ClockSrc      = bits![8, 9];
        const ToggleIRQ     = bit!(7);
        const RepeatIRQ     = bit!(6);
        const MaxIRQ        = bit!(5);
        const TargetIRQ     = bit!(4);
        const Reset         = bit!(3);
        const SyncMode      = bits![1, 2];
        const SyncEnable    = bit!(0);

        const Writable      = bits![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    }
}

/// Timer.
struct Timer {
    counter:     u16,
    mode:        TimerMode,
    target:      u16,
    irq_latch:   bool,
    pulse_latch: bool,
    in_blank:    bool,

    pause:       bool,
    blank_timer: bool,
}

impl Timer {
    fn new(blank_timer: bool) -> Self {
        Self {
            counter:     0,
            mode:        TimerMode::IRQ,
            target:      0,
            irq_latch:   false,
            pulse_latch: false,
            in_blank:    false,

            pause:       false,
            blank_timer
        }
    }

    fn write_mode(&mut self, mode: u16) {
        let mode_write = TimerMode::from_bits_truncate(mode);
        self.mode.remove(TimerMode::Writable);
        self.mode.insert(mode_write.intersection(TimerMode::Writable));
        if mode_write.contains(TimerMode::IRQ) {
            self.irq_latch = false;
        }
        self.mode.insert(TimerMode::IRQ);
        if mode_write.contains(TimerMode::SyncEnable) {
            if self.blank_timer {
                match (self.mode.intersection(TimerMode::SyncMode)).bits() >> 1 {
                    0b00 => self.pause = self.in_blank,
                    0b01 => self.pause = false,
                    0b10 => self.pause = !self.in_blank,
                    0b11 => self.pause = true,
                    _ => unreachable!()
                }
            } else {
                match (self.mode.intersection(TimerMode::SyncMode)).bits() >> 1 {
                    0b00 | 0b11 => self.pause = true,
                    0b01 | 0b10 => self.pause = false,
                    _ => unreachable!()
                }
            }
        } else {
            self.pause = false;
        }
        self.counter = 0;
    }

    fn read_mode(&mut self) -> u16 {
        let mode = self.mode.bits();
        self.mode.remove(TimerMode::ReachedMax.union(TimerMode::ReachedTarget));
        mode
    }

    fn blank_begin(&mut self) {
        self.in_blank = true;
        if self.mode.contains(TimerMode::SyncEnable) {
            match (self.mode.intersection(TimerMode::SyncMode)).bits() >> 1 {
                0b00 => self.pause = true,
                0b01 => self.counter = 0,
                0b10 => {
                    self.counter = 0;
                    self.pause = false;
                },
                0b11 => self.pause = false,
                _ => unreachable!()
            }
        }
    }

    fn blank_end(&mut self) {
        self.in_blank = false;
        if self.mode.contains(TimerMode::SyncEnable) {
            match (self.mode.intersection(TimerMode::SyncMode)).bits() >> 1 {
                0b00 => self.pause = false,
                0b01 => (),
                0b10 => self.pause = true,
                0b11 => (),
                _ => unreachable!()
            }
        }
    }

    fn use_sys_clock(&self) -> bool {
        let src = (self.mode.intersection(TimerMode::ClockSrc)).bits() >> 8;
        if self.blank_timer {
            !test_bit!(src, 0)
        } else {
            !test_bit!(src, 1)
        }
    }

    fn clock(&mut self, cycles: usize) -> bool {
        if self.pulse_latch {
            self.mode.insert(TimerMode::IRQ);
            self.pulse_latch = false;
        }
        if self.pause {
            return false;
        }
        let prev_irq_req = self.mode.contains(TimerMode::IRQ);
        let new_counter = (self.counter as usize) + cycles;
        self.counter = new_counter as u16;
        if new_counter >= 0xFFFF {
            if self.mode.contains(TimerMode::MaxIRQ) {
                self.trigger_interrupt();
            }
            if !self.mode.contains(TimerMode::Reset) {
                self.counter = (new_counter - 0xFFFF) as u16;
            }
        }
        if new_counter >= (self.target as usize) {
            if self.mode.contains(TimerMode::TargetIRQ) {
                self.trigger_interrupt();
            }
            if self.mode.contains(TimerMode::Reset) {
                self.counter = (new_counter - (self.target as usize)) as u16;
            }
        }
        prev_irq_req && !self.mode.contains(TimerMode::IRQ)
    }

    fn trigger_interrupt(&mut self) {
        if self.mode.contains(TimerMode::RepeatIRQ) {
            if self.mode.contains(TimerMode::ToggleIRQ) {
                self.mode.toggle(TimerMode::IRQ);
            } else {
                self.mode.remove(TimerMode::IRQ);
                self.pulse_latch = true;
            }
        } else {
            if !self.irq_latch {
                self.irq_latch = true;
                // TODO: should it do this? or always remove IRQReq?
                if self.mode.contains(TimerMode::ToggleIRQ) {
                    self.mode.toggle(TimerMode::IRQ);
                } else {
                    self.mode.remove(TimerMode::IRQ);
                }
            }
        }
    }
}