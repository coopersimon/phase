use crate::utils::{
    bits::*,
    interface::MemInterface
};

pub struct SerialIO {
    status:         SerialStatus,
    mode:           SerialMode,
    control:        SerialControl,
    baud_reload:    u16,

    timer:          u16,
}

impl SerialIO {
    pub fn new() -> Self {
        Self {
            status:         SerialStatus::empty(),
            mode:           SerialMode::empty(),
            control:        SerialControl::empty(),
            baud_reload:    0,

            timer:          0,
        }
    }
}

impl MemInterface for SerialIO {
    fn read_word(&mut self, addr: u32) -> u32 {
        let data = match addr {
            0x1F80_1050 => self.receive_data(),
            0x1F80_1054 => self.get_status(),
            0x1F80_1058 => self.get_mode_control(),
            0x1F80_105C => (self.baud_reload as u32) << 16,
            _ => panic!("invalid SIO addr {:X}", addr),
        };
        //println!("Serial: read w{:X} from {:X}", data, addr);
        data
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        //println!("Serial: write w{:X} to {:X}", data, addr);
        match addr {
            0x1F80_1050 => self.send_data(data as u8),
            0x1F80_1054 => {}, // status
            0x1F80_1058 => {
                self.set_mode(data as u16);
                self.set_control((data >> 16) as u16);
            },
            0x1F80_105C => self.set_baudrate((data >> 16) as u16),
            _ => panic!("invalid SIO addr {:X}", addr),
        }
    }

    fn read_halfword(&mut self, addr: u32) -> u16 {
        let data = match addr {
            0x1F80_1050 => self.receive_data() as u16,
            0x1F80_1054 => self.get_status() as u16,
            0x1F80_1056 => (self.get_status() >> 16) as u16,
            0x1F80_1058 => self.mode.bits(),
            0x1F80_105A => self.control.bits(),
            0x1F80_105E => self.baud_reload as u16,
            _ => panic!("invalid SIO addr {:X}", addr),
        };
        //println!("Serial: read h{:X} from {:X}", data, addr);
        data
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        //println!("Serial: write h{:X} to {:X}", data, addr);
        match addr {
            0x1F80_1050 => self.send_data(data as u8),
            0x1F80_1058 => self.set_mode(data),
            0x1F80_105A => self.set_control(data),
            0x1F80_105E => self.set_baudrate(data),
            _ => panic!("invalid SIO addr {:X}", addr),
        }
    }

    fn read_byte(&mut self, addr: u32) -> u8 {
        let data = match addr {
            0x1F80_1050 => self.receive_data() as u8,
            _ => panic!("invalid SIO addr {:X}", addr),
        };
        //println!("Serial: read b{:X} from {:X}", data, addr);
        data
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        //println!("Serial: write b{:X} to {:X}", data, addr);
        match addr {
            0x1F80_1050 => self.send_data(data),
            _ => panic!("invalid SIO addr {:X}", addr),
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct SerialStatus: u16 {
        const IRQ               = bit!(9);
        const CTSInputLevel     = bit!(8);
        const DSRInputLevel     = bit!(7);
        const RXInputLevel      = bit!(6);
        const RXBadStopBit      = bit!(5);
        const RXFIFOOverrun     = bit!(4);
        const RXParityError     = bit!(3);
        const TXReady2          = bit!(2);
        const RXFifoNotEmpty    = bit!(1);
        const TXReady1          = bit!(0);
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct SerialMode: u16 {
        const StopBitLength         = bits![6, 7];
        const ParityType            = bit!(5);
        const ParityEnable          = bit!(4);
        const CharLength            = bits![2, 3];
        const BaudrateReloadFactor  = bits![0, 1];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct SerialControl: u16 {
        const DSRIntEnable  = bit!(12);
        const RXIntEnable   = bit!(11);
        const TXIntEnable   = bit!(10);
        const RXIntMode     = bits![8, 9];
        const Reset         = bit!(6);
        const ACK           = bit!(4);
        const TXOutput      = bit!(3);
        const RXEnable      = bit!(2);
        const DTROutput     = bit!(1);
        const TXEnable      = bit!(0);

        const Writable = bits![0, 2, 8, 9, 10, 11, 12];
    }
}

// Internal
impl SerialIO {
    fn send_data(&mut self, _data: u8) {

    }

    fn receive_data(&mut self) -> u32 {
        0
    }

    fn get_status(&self) -> u32 {
        let status = self.status.bits() as u32;
        let timer = (self.timer as u32) << 11;
        status | timer
    }

    fn get_mode_control(&self) -> u32 {
        let mode = self.mode.bits() as u32;
        let control = (self.control.bits() as u32) << 16;
        mode | control
    }

    fn set_mode(&mut self, data: u16) {
        self.mode = SerialMode::from_bits_truncate(data);
    }

    fn set_control(&mut self, data: u16) {
        self.control = SerialControl::from_bits_truncate(data);
    }

    fn set_baudrate(&mut self, data: u16) {
        self.baud_reload = data;
    }
}
