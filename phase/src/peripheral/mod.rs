pub mod controller;
mod memcard;

use std::collections::VecDeque;
use std::path::Path;

use crate::{
    Port, interrupt::Interrupt, utils::{bits::*, interface::MemInterface}
};

use controller::ControllerState;
use memcard::*;

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
    irq_latch: bool,
    multitap_mem_card: bool,

    // Devices:
    port_1_controller: ControllerData,
    port_2_controller: ControllerData,

    port_1_mem_card: Option<MemoryCard>,
    port_2_mem_card: Option<MemoryCard>,
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
            irq_latch: false,
            multitap_mem_card: false,

            port_1_controller: ControllerData::new(),
            port_2_controller: ControllerData::new(),

            port_1_mem_card: None,
            port_2_mem_card: None,
        }
    }

    pub fn clock(&mut self, cycles: usize) -> Interrupt {
        let clocks = cycles as u32;
        if self.baudrate_timer <= clocks {
            self.reload_baudrate(clocks - self.baudrate_timer);
            if self.transfer_active {
                self.process_data();
            }
        } else {
            self.baudrate_timer -= clocks;
        }

        if self.irq_latch {
            self.irq_latch = false;
            Interrupt::Peripheral
        } else {
            Interrupt::empty()
        }
    }

    pub fn set_controller_state(&mut self, port: Port, state: ControllerState) {
        match port {
            Port::One => state.get_binary(&mut self.port_1_controller.output_data),
            Port::Two => state.get_binary(&mut self.port_2_controller.output_data),
        }
    }

    pub fn clear_controller_state(&mut self, port: Port) {
        match port {
            Port::One => self.port_1_controller.output_data.fill(0xFFFF),// TODO: analog = 0?
            Port::Two => self.port_1_controller.output_data.fill(0xFFFF),// TODO: analog = 0?
        }
    }

    pub fn insert_mem_card(&mut self, port: Port, path: &Path) -> std::io::Result<()> {
        match port {
            Port::One => self.port_1_mem_card = Some(MemoryCard::new(path)?),
            Port::Two => self.port_2_mem_card = Some(MemoryCard::new(path)?),
        }
        Ok(())
    }

    pub fn remove_mem_card(&mut self, port: Port) {
        match port {
            Port::One => self.port_1_mem_card = None,
            Port::Two => self.port_2_mem_card = None,
        }
    }

    pub fn flush_mem_cards(&mut self) {
        if let Some(mem_card) = self.port_1_mem_card.as_mut() {
            mem_card.flush();
        }
        if let Some(mem_card) = self.port_2_mem_card.as_mut() {
            mem_card.flush();
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

        const Writable = bits![0, 2, 8, 9, 10, 11, 12, 13];
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TransferMode {
    None,
    Controller(u8),
    Multitap(u8),
    MemCard,
}

// Internal
impl PeripheralPort {
    fn send_data(&mut self, data: u8) {
        self.in_fifo.push_back(data);
        self.transfer_active = true;
        self.reload_baudrate(0);
        self.status.insert(JoypadStatus::TXReady1);
        self.status.remove(JoypadStatus::TXReady2);
    }

    fn receive_data(&mut self) -> u32 {
        u32::from_le_bytes([
            self.out_fifo.pop_front().unwrap_or(0xFF),
            self.out_fifo.get(0).cloned().unwrap_or(0xFF),
            self.out_fifo.get(1).cloned().unwrap_or(0xFF),
            self.out_fifo.get(2).cloned().unwrap_or(0xFF),
        ])
    }

    fn read_status(&mut self) -> u32 {
        let mut status = self.status;
        self.status.remove(JoypadStatus::AckInputLevel);
        if !self.out_fifo.is_empty() {
            status.insert(JoypadStatus::RXFifoNotEmpty);
        }
        let current_timer = self.baudrate_timer / 8;
        (status.bits() as u32) | (current_timer << 11)
    }

    fn set_mode(&mut self, data: u16) {
        self.mode = JoypadMode::from_bits_truncate(data);
    }

    fn set_control(&mut self, data: u16) {
        let control = JoypadControl::from_bits_truncate(data);
        if control.contains(JoypadControl::ACK) {
            self.status.remove(JoypadStatus::IRQ.union(JoypadStatus::RXParityError));
        }
        if control.contains(JoypadControl::Reset) {
            // TODO: A little unsure what this should actually reset.
            self.transfer_mode = TransferMode::None;
        }
        self.control = control.intersection(JoypadControl::Writable);
        if control.contains(JoypadControl::JoyNOutput) {
            self.slot_select = if control.contains(JoypadControl::SlotSelect) {1} else {0};
            //self.control.insert(JoypadControl::RXEnable);
        }
        if control.contains(JoypadControl::TXEnable) {
            self.status.insert(JoypadStatus::TXReady1);
            if control.contains(JoypadControl::TXIntEnable) {
                // TODO: ?
                self.trigger_irq();
            }
        } else {
            // Cancel ongoing transfer.
            self.transfer_active = false;
            self.status.remove(JoypadStatus::TXReady1);
            self.transfer_mode = TransferMode::None;
            self.in_fifo.clear();
            self.out_fifo.clear();
            if let Some(mem_card) = self.port_1_mem_card.as_mut() {
                mem_card.cancel_transfer();
            }
            if let Some(mem_card) = self.port_2_mem_card.as_mut() {
                mem_card.cancel_transfer();
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
        self.reload_baudrate(0);
    }

    fn reload_baudrate(&mut self, offset: u32) {
        let multiply_factor = match self.mode.intersection(JoypadMode::BaudrateReloadFactor).bits() {
            2 => 16,
            3 => 32,
            _ => 1
        } * 8; // 8 bits need to be transferred?
        let reload = self.baudrate_reload * multiply_factor;
        self.baudrate_timer = reload - offset;
    }

    /// When we have clocked enough cycles,
    /// we can process some data.
    fn process_data(&mut self) {
        self.status.remove(JoypadStatus::TXReady1);
        let Some(data_in) = self.in_fifo.pop_front() else {
            panic!("no data to process");
            return;
        };
        //println!("Peripheral in: {:X} (state: {:?}) (port 2: {})", data_in, self.transfer_mode, self.control.contains(JoypadControl::SlotSelect));
        self.transfer_active = !self.in_fifo.is_empty();
        match self.transfer_mode {
            TransferMode::None => {
                self.multitap_mem_card = false;
                if data_in == 0x01 {
                    self.transfer_mode = TransferMode::Controller(0);
                    self.push_data(0xFF);
                } else if data_in == 0x81 {
                    self.transfer_mode = TransferMode::MemCard;
                    self.push_data(0xFF);
                } else if data_in & 0xF0 == 0x80 {
                    self.multitap_mem_card = true;
                    self.transfer_mode = TransferMode::MemCard;
                    self.push_data(0xFF);
                } else if data_in == 0x00 {
                    // ...
                    self.push_data(0xFF);
                } else {
                    panic!("unrecognised peripheral data {:X}", data_in);
                }
            },
            TransferMode::Controller(n) => self.process_controller_mode(data_in, n),
            TransferMode::Multitap(n) => {
                // For now: just send 1 controller.
                let controller = if self.control.contains(JoypadControl::SlotSelect) {
                    &self.port_2_controller
                } else {
                    &self.port_1_controller
                };
                let (transfer_mode, data) = match n {
                    0 => (TransferMode::Multitap(1), controller.output_data[0].to_le_bytes()[0]),
                    1 => (TransferMode::Multitap(2), controller.output_data[0].to_le_bytes()[1]),
                    2 => (TransferMode::Multitap(3), controller.output_data[1].to_le_bytes()[0]),
                    3 => (TransferMode::Multitap(4), controller.output_data[1].to_le_bytes()[1]),
                    4..=30 => (TransferMode::Multitap(n+1), 0xFF),
                    31 => (TransferMode::None, 0xFF),
                    _ => unreachable!()
                };
                self.transfer_mode = transfer_mode;
                self.push_data(data);
            },
            TransferMode::MemCard => self.process_memcard_mode(data_in),
        }
        if self.transfer_mode == TransferMode::None {
            self.status.insert(JoypadStatus::TXReady2);
        }
        self.status.insert(JoypadStatus::AckInputLevel);
        if self.control.contains(JoypadControl::AckIntEnable) {
            self.trigger_irq();
        }
    }

    fn process_controller_mode(&mut self, data_in: u8, n: u8) {
        let controller = if self.control.contains(JoypadControl::SlotSelect) {
            &mut self.port_2_controller
        } else {
            &mut self.port_1_controller
        };
        let (transfer_mode, data) = if controller.config_mode {
            match n {
                0 => {
                    controller.config_command = data_in;
                    match data_in {
                        0x42 => { // Buttons: leave alone.
                            controller.config_data.clone_from_slice(&controller.output_data[1..]);
                        },
                        0x43 => {
                            controller.config_data.fill(0);
                        },
                        0x44 => { // set LED state
                            controller.config_data.fill(0);
                        },
                        0x45 => { // get LED state
                            controller.config_data[0] = 0x0201;
                            controller.config_data[1] = 0x0200; // TODO: lower-byte: analog mode.
                            controller.config_data[2] = 0x0001;
                        },
                        0x46 => { // pad info act
                            controller.config_data[0] = 0x0000;
                        },
                        0x47 => { // unknown?
                            controller.config_data[0] = 0x0000;
                            controller.config_data[1] = 0x0002;
                            controller.config_data[2] = 0x0001;
                        },
                        0x4C => { // get variable response
                            controller.config_data.fill(0);
                        },
                        0x4D => { // get/set rumble protocol
                            controller.config_data.fill(0xFF); // TODO: actually implement this
                        },
                        _ => panic!("unrecognised controller config mode {:X}", data_in),
                    }
                    (TransferMode::Controller(1), 0xF3)
                },
                1 => (TransferMode::Controller(2), 0x5A), // Data should be 0x00
                2 => {
                    match controller.config_command {
                        0x42 => {},
                        0x43 => match data_in {
                            0x00 => controller.config_pending = false, // Exit config mode
                            0x01 => controller.config_pending = true, // Stay in config mode.
                            _ => panic!("unrecognised config mode command {:X}", data_in),
                        },
                        0x44 => {}, // TODO: led state
                        0x45 => {},
                        0x46 => match data_in { // pad info act
                            0x00 => {
                                controller.config_data[1] = 0x0201;
                                controller.config_data[2] = 0x0A00;
                            },
                            0x01 => {
                                controller.config_data[1] = 0x0101;
                                controller.config_data[2] = 0x1401;
                            },
                            _ => {
                                controller.config_data[1] = 0x0000;
                                controller.config_data[2] = 0x0000;
                            }
                        },
                        0x47 => {},
                        0x4C => match data_in {
                            0x00 => controller.config_data[1] = 0x0400,
                            0x01 => controller.config_data[1] = 0x0700,
                            _ => controller.config_data[1] = 0x0000,
                        },
                        0x4D => {}, // TODO.
                        _ => unreachable!(),
                    }
                    (TransferMode::Controller(3), controller.config_data[0].to_le_bytes()[0])
                },
                3 => (TransferMode::Controller(4), controller.config_data[0].to_le_bytes()[1]),
                4 => (TransferMode::Controller(5), controller.config_data[1].to_le_bytes()[0]),
                5 => (TransferMode::Controller(6), controller.config_data[1].to_le_bytes()[1]),
                6 => (TransferMode::Controller(7), controller.config_data[2].to_le_bytes()[0]),
                7 => {
                    if !controller.config_pending {
                        controller.config_mode = false;
                    }
                    (TransferMode::None, controller.config_data[2].to_le_bytes()[1])
                },
                _ => unreachable!(),
            }
        } else {
            match n {
                0 => {
                    if data_in == 0x43 {
                        controller.config_pending = true;
                    } else if data_in != 0x42 {
                        panic!("Unexpected data in after controller select: {:X}", data_in);
                    }
                    (TransferMode::Controller(1), controller.output_data[0].to_le_bytes()[0])
                },
                1 => {
                    let data = controller.output_data[0].to_le_bytes()[1];
                    if data == 0xFF {
                        (TransferMode::None, 0xFF)
                    } else {
                        //if data_in == 0x01 { // TODO:
                        //    (TransferMode::Multitap(0), data)
                        //} else {
                            (TransferMode::Controller(2), data)
                        //}
                    }
                },
                2 => {
                    if controller.config_pending {
                        match data_in {
                            0x00 => controller.config_pending = false, // Exit config mode
                            0x01 => controller.config_pending = true, // Stay in config mode.
                            _ => panic!("unrecognised config mode command {:X}", data_in),
                        }
                    }
                    (TransferMode::Controller(3), controller.output_data[1].to_le_bytes()[0])
                },
                3 => {
                    if controller.config_pending {
                        controller.config_mode = true;
                    }
                    (TransferMode::None, controller.output_data[1].to_le_bytes()[1])
                },
                _ => unreachable!()
            }
        };
        self.transfer_mode = transfer_mode;
        self.push_data(data);
    }

    fn process_memcard_mode(&mut self, data_in: u8) {
        let mem_card = if self.multitap_mem_card {
            None
        } else if self.control.contains(JoypadControl::SlotSelect) {
            self.port_2_mem_card.as_mut()
        } else {
            self.port_1_mem_card.as_mut()
        };
        let data = if let Some(mem_card) = mem_card {
            let data = mem_card.transfer_data(data_in);
            if mem_card.transfer_complete() {
                self.transfer_mode = TransferMode::None;
            }
            data
        } else {
            // Eventually the system should cancel the transfer.
            0xFF
        };
        self.push_data(data);
    }

    fn push_data(&mut self, data: u8) {
        //if self.control.contains(JoypadControl::RXEnable) {
            self.out_fifo.push_back(data);
            if self.control.contains(JoypadControl::RXIntEnable) {
                let rx_int_mode = self.control.intersection(JoypadControl::RXIntMode).bits() >> 8;
                let out_fifo_len = 1 << rx_int_mode;
                if self.out_fifo.len() == out_fifo_len {
                    self.trigger_irq();
                }
            }
        //}
    }

    fn trigger_irq(&mut self) {
        self.irq_latch = true;
        self.status.insert(JoypadStatus::IRQ);
    }
}

struct ControllerData {
    output_data: [u16; 4],
    // TODO: config mode feels messy
    config_mode: bool, // TODO: one per controller?
    config_command: u8,
    config_data: [u16; 3],
    config_pending: bool,
}

impl ControllerData {
    fn new() -> Self {
        Self {
            output_data: [0xFFFF, 0xFFFF, 0x0000, 0x0000],

            config_mode: false,
            config_command: 0,
            config_data: [0; 3],
            config_pending: false,
        }
    }
}