
use std::collections::VecDeque;

use crate::{interrupt::Interrupt, utils::{bits::*, interface::MemInterface}};

/// Handles the controllers and memory cards.
pub struct PeripheralPort {
    status: JoypadStatus,
    mode: JoypadMode,
    control: JoypadControl,
    baudrate_reload: u32,
    baudrate_timer: u32,

    slot_select: u8,
    in_fifo: VecDeque<u8>,
    out_fifo: VecDeque<u8>,
    transfer_mode: TransferMode,
    transfer_active: bool,
}

impl PeripheralPort {
    pub fn new() -> Self {
        Self {
            status: JoypadStatus::empty(),
            mode: JoypadMode::empty(),
            control: JoypadControl::empty(),
            baudrate_reload: 0,
            baudrate_timer: 0,

            slot_select: 0,
            in_fifo: VecDeque::new(),
            out_fifo: VecDeque::new(),
            transfer_mode: TransferMode::None,
            transfer_active: false,
        }
    }

    pub fn clock(&mut self, cycles: usize) -> Interrupt {
        let clocks = (cycles * 2) as u32;
        if self.baudrate_timer <= clocks {
            let multiply_factor = match (self.mode & JoypadMode::BaudrateReloadFactor).bits() {
                2 => 16,
                3 => 32,
                _ => 1
            };
            let reload = self.baudrate_reload * multiply_factor;
            self.baudrate_timer += reload - clocks;
            if self.transfer_active {
                self.process_data();
            }
        } else {
            self.baudrate_timer -= clocks;
        }

        if self.status.contains(JoypadStatus::IRQ) {
            // TODO: edge-triggered..?
            Interrupt::Peripheral
        } else {
            Interrupt::empty()
        }
    }
}

// We need the full gamut of read/write operations here.
// (In fact, word might be unnecessary)
impl MemInterface for PeripheralPort {
    fn read_word(&mut self, addr: u32) -> u32 {
        let data = match addr {
            0x1F80_1040 => self.receive_data(),
            0x1F80_1044 => self.read_status(),
            0x1F80_1048 => self.get_mode_control(),
            0x1F80_104C => self.baudrate_reload << 16,
            _ => panic!("invalid peripheral addr"),
        };
        //println!("Peripheral: read w{:X} from {:X}", data, addr);
        data
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        //println!("Peripheral: write w{:X} to {:X}", data, addr);
        match addr {
            0x1F80_1040 => self.send_data(data as u8),
            0x1F80_1044 => {},
            0x1F80_1048 => {
                self.set_mode(data as u16);
                self.set_control((data >> 16) as u16);
            },
            0x1F80_104C => self.set_baudrate_reload((data >> 16) as u16),
            _ => panic!("invalid peripheral addr"),
        }
    }

    fn read_halfword(&mut self, addr: u32) -> u16 {
        let data = match addr {
            0x1F80_1040 => self.receive_data() as u16,
            0x1F80_1044 => self.read_status() as u16,
            0x1F80_1046 => (self.read_status() >> 16) as u16,
            0x1F80_1048 => self.mode.bits(),
            0x1F80_104A => self.control.bits(),
            0x1F80_104E => self.baudrate_reload as u16,
            _ => panic!("invalid peripheral addr"),
        };
        //println!("Peripheral: read h{:X} from {:X}", data, addr);
        data
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        //println!("Peripheral: write h{:X} to {:X}", data, addr);
        match addr {
            0x1F80_1040 => self.send_data(data as u8),
            0x1F80_1048 => self.set_mode(data),
            0x1F80_104A => self.set_control(data),
            0x1F80_104E => self.set_baudrate_reload(data),
            _ => panic!("invalid peripheral addr"),
        }
    }

    fn read_byte(&mut self, addr: u32) -> u8 {
        let data = match addr {
            0x1F80_1040 => self.receive_data() as u8,
            _ => panic!("cannot access peripheral byte {:X}", addr),
        };
        //println!("Peripheral: read b{:X} from {:X}", data, addr);
        data
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        //println!("Peripheral: write b{:X} to {:X}", data, addr);
        match addr {
            0x1F80_1040 => self.send_data(data),
            _ => panic!("cannot access peripheral byte {:X}", addr),
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct JoypadStatus: u16 {
        const IRQ               = bit!(9);
        const AckInputLevel     = bit!(7);
        const RXParityError     = bit!(3);
        const TXReady2          = bit!(2);
        const RXFifoNotEmpty    = bit!(1);
        const TXReady1          = bit!(0);
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct JoypadMode: u16 {
        const ClockOutPolarity      = bit!(8);
        const ParityType            = bit!(5);
        const ParityEnable          = bit!(4);
        const CharLength            = bits![2, 3];
        const BaudrateReloadFactor  = bits![0, 1];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct JoypadControl: u16 {
        const SlotSelect    = bit!(13);
        const AckIntEnable  = bit!(12);
        const RXIntEnable   = bit!(11);
        const TXIntEnable   = bit!(10);
        const RXIntMode     = bits![8, 9];
        const Reset         = bit!(6);
        const ACK           = bit!(4);
        const RXEnable      = bit!(2);
        const JoyNOutput    = bit!(1);
        const TXEnable      = bit!(0);

        const Writable = bits![0, 1, 2, 8, 9, 10, 11, 12, 13];
    }
}

#[derive(Clone, Copy)]
enum TransferMode {
    None,
    Controller(u8),
    MemCard(u8),
}

// Internal
impl PeripheralPort {
    fn send_data(&mut self, data: u8) {
        self.in_fifo.push_back(data);
        self.transfer_active = true;
    }

    fn receive_data(&mut self) -> u32 {
        u32::from_le_bytes([
            self.out_fifo.pop_front().unwrap_or(0xFF),
            self.out_fifo.get(0).cloned().unwrap_or(0xFF),
            self.out_fifo.get(1).cloned().unwrap_or(0xFF),
            self.out_fifo.get(2).cloned().unwrap_or(0xFF),
        ])
    }

    fn read_status(&self) -> u32 {
        let mut status = self.status;
        if !self.out_fifo.is_empty() {
            status.insert(JoypadStatus::RXFifoNotEmpty);
        }
        /*if self.transfer_active {
            status.insert(JoypadStatus::TXReady1);
        } else { // TODO: only if TXEnabled ?
            status.insert(JoypadStatus::TXReady2);
        }*/
        (status.bits() as u32) | (self.baudrate_timer << 11)
    }

    fn set_mode(&mut self, data: u16) {
        self.mode = JoypadMode::from_bits_truncate(data);
    }

    fn set_control(&mut self, data: u16) {
        let control = JoypadControl::from_bits_truncate(data);
        if control.contains(JoypadControl::ACK) {
            self.status.remove(JoypadStatus::IRQ | JoypadStatus::RXParityError);
        }
        if control.contains(JoypadControl::Reset) {
            // ? TODO
        }
        if control.contains(JoypadControl::JoyNOutput) {
            self.slot_select = if control.contains(JoypadControl::SlotSelect) {1} else {0};
        }
        self.control = control & JoypadControl::Writable;
        if control.contains(JoypadControl::JoyNOutput) {
            self.control.insert(JoypadControl::RXEnable);
        }
        if control.contains(JoypadControl::TXEnable) {
            self.status.insert(JoypadStatus::TXReady1);
            if control.contains(JoypadControl::TXIntEnable) {
                // TODO: ?
                self.status.insert(JoypadStatus::IRQ);
            }
        }
    }

    fn get_mode_control(&self) -> u32 {
        let mode = self.mode.bits() as u32;
        let control = self.control.bits() as u32;
        mode | (control << 16)
    }

    fn set_baudrate_reload(&mut self, data: u16) {
        self.baudrate_reload = data as u32;
        self.baudrate_timer = self.baudrate_reload;
    }

    /// When we have clocked enough cycles,
    /// we can process some data.
    fn process_data(&mut self) {
        let Some(data_in) = self.in_fifo.pop_front() else {
            return;
        };
        self.transfer_active = !self.in_fifo.is_empty();
        match self.transfer_mode {
            TransferMode::None => {
                if data_in == 0x01 {
                    self.transfer_mode = TransferMode::Controller(0);
                    self.push_data(0xFF);
                } else if data_in == 0x81 {
                    self.transfer_mode = TransferMode::MemCard(0);
                    self.push_data(0xFF);
                } else {
                    panic!("unrecognised peripheral data {:X}", data_in);
                }
            },
            TransferMode::Controller(n) => {
                match n {
                    0 => { // ID lo
                        self.transfer_mode = TransferMode::Controller(1);
                        self.push_data(0xFF);
                    },
                    1 => { // ID hi
                        self.transfer_mode = TransferMode::None;
                        self.push_data(0xFF);
                    },
                    _ => unreachable!()
                }
            },
            TransferMode::MemCard(n) => {
                match n {
                    0 => { // Flag
                        self.transfer_mode = TransferMode::MemCard(1);
                        self.push_data(0xFF);
                    },
                    1 => {
                        self.transfer_mode = TransferMode::MemCard(2);
                        self.push_data(0xFF);
                    },
                    2 => {
                        self.transfer_mode = TransferMode::None;
                        self.push_data(0xFF);
                    },
                    _ => unreachable!()
                }
            }
        }
        if self.control.contains(JoypadControl::AckIntEnable) {
            self.status.insert(JoypadStatus::IRQ);
        }
        self.status.insert(JoypadStatus::TXReady2);
    }

    fn push_data(&mut self, data: u8) {
        if self.control.contains(JoypadControl::RXEnable) {
            self.out_fifo.push_back(data);
            if self.control.contains(JoypadControl::RXIntEnable) {
                let rx_int_mode = (self.control & JoypadControl::RXIntMode).bits() >> 8;
                let out_fifo_len = 1 << rx_int_mode;
                if self.out_fifo.len() == out_fifo_len {
                    // TODO: wait..?
                    self.status.insert(JoypadStatus::IRQ);
                }
            }
        }
    }
}
