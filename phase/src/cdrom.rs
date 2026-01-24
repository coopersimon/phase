use std::{
    io::{
        Read,
        Seek,
        SeekFrom
    },
    fs::File,
    path::Path
};

use mips::mem::Data;

use crate::{interrupt::Interrupt, mem::DMADevice};
use crate::utils::{
    bits::*,
    bcd::to_bcd,
    interface::MemInterface
};
use std::collections::VecDeque;

const DISC_BUFFER_SIZE: u64 = 64 * 1024; // Load 64kB chunks.

/// CD-ROM reader.
pub struct CDROM {
    disc: Option<File>,
    offset: u64,
    buffer: Vec<u8>,
    buffer_n: u64, // Chunk count.

    status: Status,
    int_enable: IntFlags,
    int_flags: IntFlags,
    request: Request,

    vol_left_to_left: u8,
    vol_left_to_right: u8,
    vol_right_to_left: u8,
    vol_right_to_right: u8,

    param_fifo: VecDeque<u8>,
    response_fifo: VecDeque<u8>,

    drive_status: DriveStatus,
    mode: DriveMode,
    loc: DriveLoc,

    irq_latch: bool,
}

impl CDROM {
    pub fn new() -> Self {
        Self {
            disc: None,
            offset: 0,
            buffer: vec![0; DISC_BUFFER_SIZE as usize],
            buffer_n: 0,

            status: Status::ParamFifoEmpty,
            int_enable: IntFlags::Unused,
            int_flags: IntFlags::Unused,
            request: Request::empty(),

            vol_left_to_left: 0,
            vol_left_to_right: 0,
            vol_right_to_left: 0,
            vol_right_to_right: 0,

            param_fifo: VecDeque::new(),
            response_fifo: VecDeque::new(),

            drive_status: DriveStatus::empty(),
            mode: DriveMode::empty(),
            loc: DriveLoc { minute: 0, second: 0, frame: 0 },

            irq_latch: false,
        }
    }

    /// Insert or remove a disc from the PlayStation.
    pub fn insert_disc(&mut self, path: Option<&Path>) -> std::io::Result<()> {
        if let Some(path) = path {
            let disc_file = File::open(path)?;
            self.disc = Some(disc_file);
            self.offset = 0;
            self.read_from_file(0);
        } else {
            self.disc = None
        }
        Ok(())
    }

    /// Clock the CD-ROM reader.
    /// 
    /// Returns an interrupt if it occurred.
    pub fn clock(&mut self, _cycles: usize) -> Interrupt {
        // TODO: proper timing.

        if self.check_irq() {
            Interrupt::CDROM
        } else {
            Interrupt::empty()
        }
    }
}

impl MemInterface for CDROM {
    fn read_byte(&mut self, addr: u32) -> u8 {
        println!("read cd {:X}", addr);
        match addr {
            0x1F80_1800 => self.status.bits(),
            0x1F80_1801 => self.read_response(),
            0x1F80_1802 => self.read_data(),
            0x1F80_1803 => match self.index() {
                0 | 2 => self.int_enable.bits(),
                1 | 3 => self.int_flags.bits(),
                _ => unreachable!()
            },
            _ => panic!("invalid CDROM addr {:X}", addr)
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        println!("write cd {:X}: {:X}", addr, data);
        match addr {
            0x1F80_1800 => self.write_status(data),
            0x1F80_1801 => match self.index() {
                0 => self.write_command(data),
                1 => {}, // TODO: sound map data out
                2 => {}, // TODO: sound map coding info
                3 => self.vol_right_to_right = data,
                _ => unreachable!()
            },
            0x1F80_1802 => match self.index() {
                0 => self.write_parameter(data),
                1 => self.set_int_enable(data),
                2 => self.vol_left_to_left = data,
                3 => self.vol_right_to_left = data,
                _ => unreachable!()
            },
            0x1F80_1803 => match self.index() {
                0 => self.write_request(data),
                1 => self.set_int_flags(data),
                2 => self.vol_left_to_right = data,
                3 => {}, // TODO: audio vol apply
                _ => unreachable!()
            },
            _ => panic!("invalid CDROM addr {:X}", addr)
        }
    }

    fn read_halfword(&mut self, addr: u32) -> u16 {
        u16::from_le_bytes([
            self.read_byte(addr),
            self.read_byte(addr + 1),
        ])
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        let data = data.to_le_bytes();
        self.write_byte(addr, data[0]);
        self.write_byte(addr + 1, data[1]);
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        u32::from_le_bytes([
            self.read_byte(addr),
            self.read_byte(addr + 1),
            self.read_byte(addr + 2),
            self.read_byte(addr + 3),
        ])
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        let data = data.to_le_bytes();
        self.write_byte(addr, data[0]);
        self.write_byte(addr + 1, data[1]);
        self.write_byte(addr + 2, data[2]);
        self.write_byte(addr + 3, data[3]);
    }
}

impl DMADevice for CDROM {
    fn dma_read_word(&mut self) -> Data<u32> {
        let data = u32::from_le_bytes([
            self.read_data(),
            self.read_data(),
            self.read_data(),
            self.read_data()
        ]);
        Data { data, cycles: 1 }
    }

    fn dma_write_word(&mut self, _data: u32) -> usize {
        panic!("not valid to use DMA to write to CDROM!")
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct Status: u8 {
        const Busy              = bit!(7);
        const DataFifoEmpty     = bit!(6);  // 0 = Empty
        const ResFifoEmpty      = bit!(5);  // 0 = Empty
        const ParamFifoFull     = bit!(4);  // 0 = Full
        const ParamFifoEmpty    = bit!(3);  // 1 = Empty
        const ADPBusy           = bit!(2);  // 0 = Empty
        const PortIndex         = bits![0, 1];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct IntFlags: u8 {
        const Unused        = bits![5, 6, 7];
        const CommandStart  = bit!(4);
        const Unknown       = bit!(3);
        const Response      = bits![0, 1, 2];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct Request: u8 {
        const WantData          = bit!(7);
        const BFWR              = bit!(6);
        const CommandStartInt   = bit!(5);
    }
}

// Internal.
impl CDROM {
    fn check_irq(&mut self) -> bool {
        let irq = (self.int_enable & self.int_flags).intersects(!IntFlags::Unused);
        if !self.irq_latch {
            self.irq_latch = irq;
            irq
        } else {
            self.irq_latch = irq;
            false
        }
    }

    fn write_status(&mut self, data: u8) {
        self.status.remove(Status::PortIndex);
        self.status.insert(Status::from_bits_truncate(data) & Status::PortIndex);
    }

    fn index(&self) -> u8 {
        (self.status & Status::PortIndex).bits()
    }

    fn write_command(&mut self, data: u8) {
        let res = match data {
            0x00 => self.sync(),
            0x01 => self.get_stat(),
            0x02 => self.set_loc(),
            0x06 => self.read_n(),
            0x07 => self.motor_on(),
            0x08 => self.stop(),
            0x09 => self.pause(),
            0x0A => self.init(),
            0x0D => self.set_filter(),
            0x0E => self.set_mode(),
            0x0F => self.get_param(),
            0x10 => self.get_loc_l(),
            0x11 => self.get_loc_p(),
            0x12 => self.set_session(),
            0x13 => self.get_tn(),
            0x14 => self.get_td(),
            0x15 => self.seek_l(),
            0x16 => self.seek_p(),
            0x1A => self.get_id(),
            0x1B => self.read_s(),
            0x1D => self.get_q(),
            0x1E => self.read_toc(),
            _ => panic!("unknown CD-ROM command {:X}", data),
        };
        if let Err(res) = res {
            self.send_response(res.bits(), 5);
        }
    }

    fn write_parameter(&mut self, data: u8) {
        if self.param_fifo.len() >= 16 {
            panic!("param fifo len too long");
        }
        self.param_fifo.push_back(data);
        self.status.remove(Status::ParamFifoEmpty);
        self.status.set(Status::ParamFifoFull, self.param_fifo.len() == 16);
    }

    fn read_parameter(&mut self) -> DriveResult<u8> {
        let param = self.param_fifo.pop_front();
        if let Some(param) = param {
            self.status.set(Status::ParamFifoEmpty, self.param_fifo.is_empty());
            self.status.remove(Status::ParamFifoFull);
            Ok(param)
        } else {
            self.drive_status.insert(DriveStatus::Error);
            Err(DriveError::MissingParam)
        }
    }

    fn write_request(&mut self, data: u8) {
        self.request = Request::from_bits_truncate(data);
    }

    fn set_int_enable(&mut self, data: u8) {
        self.int_enable = IntFlags::from_bits_truncate(data);
        self.int_enable.insert(IntFlags::Unused);
    }

    fn set_int_flags(&mut self, data: u8) {
        self.int_flags.remove(IntFlags::from_bits_truncate(data));
        self.int_flags.insert(IntFlags::Unused);
    }
    
    fn read_response(&mut self) -> u8 {
        let data = self.response_fifo.pop_front();
        if self.response_fifo.is_empty() {
            self.status.remove(Status::ResFifoEmpty);
        }
        data.unwrap_or(0)
    }

    /// Write response from command.
    /// 
    /// Also sets interrupt bits. Int should be a value 1-7.
    fn send_response(&mut self, data: u8, int: u8) {
        self.response_fifo.push_back(data);
        self.status.insert(Status::ResFifoEmpty);
        self.int_flags.remove(IntFlags::Response);
        self.int_flags.insert(IntFlags::from_bits_truncate(int));
    }

    fn read_data(&mut self) -> u8 {
        if self.drive_status.contains(DriveStatus::Reading) {
            let required_buffer = self.offset / DISC_BUFFER_SIZE;
            if required_buffer != self.buffer_n {
                self.read_from_file(required_buffer * DISC_BUFFER_SIZE);
            }
            let buffer_index = self.offset % DISC_BUFFER_SIZE;
            self.offset += 1;
            self.buffer[buffer_index as usize]
        } else {
            0
        }
    }

    /// Read from disc into buffer.
    fn read_from_file(&mut self, offset: u64) {
        let Some(disc_file) = self.disc.as_mut() else {
            return;
        };
        disc_file.seek(SeekFrom::Start(offset)).expect("could not seek in disc");
        disc_file.read(&mut self.buffer).expect("could not load disc data");
        self.buffer_n = offset / DISC_BUFFER_SIZE;
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct DriveStatus: u8 {
        const Playing       = bit!(7);
        const Seeking       = bit!(6);
        const Reading       = bit!(5);
        const ShellOpen     = bit!(4);
        const IDError       = bit!(3);
        const SeekError     = bit!(2);
        const SpindleMotor  = bit!(1);
        const Error         = bit!(0);

        const ReadBits      = bits![5, 6, 7];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct DriveError: u8 {
        const CantRespondYet= bit!(7);
        const InvalidCmd    = bit!(6);
        const MissingParam  = bit!(5);
        const InvalidParam  = bit!(4);
        const DriveOpen     = bit!(3);
        const SeekFailed    = bit!(2);
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct DriveMode: u8 {
        const Speed         = bit!(7);
        const XAADPCM       = bit!(6);
        const SectorSize    = bit!(5);
        const IgnoreBit     = bit!(4);
        const XAFilter      = bit!(3);
        const Report        = bit!(2);
        const AutoPause     = bit!(1);
        const CDDA          = bit!(0);
    }
}

struct DriveLoc {
    minute: u8,
    second: u8,
    frame: u8, // 75 frames per second
}

impl DriveLoc {
    fn byte_offset(&self) -> u64 {
        const FRAME_SIZE: u64 = 2352;
        const SEC_SIZE: u64 = 75 * FRAME_SIZE;
        const MIN_SIZE: u64 = 60 * SEC_SIZE;
        let frame_offset = (self.frame as u64) * FRAME_SIZE;
        let sec_offset = (self.second as u64) * SEC_SIZE;
        let min_offset = (self.minute as u64) * MIN_SIZE;
        min_offset + sec_offset + frame_offset
    }
}

type DriveResult<T> = Result<T, DriveError>;

// Commands.
impl CDROM {
    fn sync(&mut self) -> DriveResult<()> {
        Ok(())
    }

    fn set_filter(&mut self) -> DriveResult<()> {
        let _file = self.read_parameter()?;
        let _channel = self.read_parameter()?;
        self.send_response(self.drive_status.bits(), 3);
        Ok(())
    }

    fn set_mode(&mut self) -> DriveResult<()> {
        let mode = self.read_parameter()?;
        self.mode = DriveMode::from_bits_truncate(mode);
        self.send_response(self.drive_status.bits(), 3);
        Ok(())
    }

    fn init(&mut self) -> DriveResult<()> {
        self.drive_status.insert(DriveStatus::SpindleMotor);
        self.send_response(self.drive_status.bits(), 3);
        self.send_response(self.drive_status.bits(), 2);
        Ok(())
    }

    fn motor_on(&mut self) -> DriveResult<()> {
        if self.drive_status.contains(DriveStatus::SpindleMotor) {
            self.drive_status.insert(DriveStatus::Error);
            Err(DriveError::MissingParam)
        } else {
            self.drive_status.insert(DriveStatus::SpindleMotor);
            self.send_response(self.drive_status.bits(), 3);
            self.send_response(self.drive_status.bits(), 2);
            Ok(())
        }
    }

    fn stop(&mut self) -> DriveResult<()> {
        self.drive_status.remove(DriveStatus::ReadBits);
        self.send_response(self.drive_status.bits(), 3);
        self.drive_status.remove(DriveStatus::SpindleMotor);
        self.send_response(self.drive_status.bits(), 2);
        Ok(())
    }

    fn pause(&mut self) -> DriveResult<()> {
        self.send_response(self.drive_status.bits(), 3);
        self.drive_status.remove(DriveStatus::ReadBits);
        self.send_response(self.drive_status.bits(), 2);
        Ok(())
    }

    fn set_loc(&mut self) -> DriveResult<()> {
        self.loc.minute = self.read_parameter()?; // mm
        self.loc.second = self.read_parameter()?; // ss
        self.loc.frame = self.read_parameter()?;  // sector / frame
        self.send_response(self.drive_status.bits(), 3);
        Ok(())
    }

    /// Data seek
    fn seek_l(&mut self) -> DriveResult<()> {
        self.drive_status.remove(DriveStatus::ReadBits);
        // TODO: insert seek bits?
        self.offset = self.loc.byte_offset();
        self.send_response(self.drive_status.bits(), 3);
        self.send_response(self.drive_status.bits(), 2);
        Ok(())
    }

    /// Audio seek
    fn seek_p(&mut self) -> DriveResult<()> {
        self.drive_status.remove(DriveStatus::ReadBits);
        // TODO: insert seek bits?
        let _byte_offset = self.loc.byte_offset();
        // TODO ? (subchannel Q)
        Ok(())
    }

    fn set_session(&mut self) -> DriveResult<()> {
        // Only support session 1.
        let session = self.read_parameter()?;
        if session == 0x00 {
            self.send_response(0x03, 5);
            Err(DriveError::InvalidParam)
        } else if session > 0x01 {
            self.send_response(self.drive_status.bits(), 3);
            self.send_response(0x06, 5);
            Err(DriveError::InvalidCmd)
        } else {
            self.send_response(self.drive_status.bits(), 3);
            self.send_response(self.drive_status.bits(), 2);
            Ok(())
        }
    }

    /// Read with retry
    fn read_n(&mut self) -> DriveResult<()> {
        self.send_response(self.drive_status.bits(), 3);
        self.drive_status.remove(DriveStatus::ReadBits);
        self.drive_status.insert(DriveStatus::Reading);
        self.send_response(self.drive_status.bits(), 1);
        Ok(())
    }

    /// Read without retry
    fn read_s(&mut self) -> DriveResult<()> {
        self.send_response(self.drive_status.bits(), 3);
        self.drive_status.remove(DriveStatus::ReadBits);
        self.drive_status.insert(DriveStatus::Reading);
        self.send_response(self.drive_status.bits(), 1);
        Ok(())
    }

    /// Read table of contents
    fn read_toc(&mut self) -> DriveResult<()> {
        // TODO: .cue file?
        self.send_response(self.drive_status.bits(), 3);
        self.send_response(self.drive_status.bits(), 2);
        Ok(())
    }

    fn get_stat(&mut self) -> DriveResult<()> {
        self.send_response(self.drive_status.bits(), 3);
        self.drive_status.remove(DriveStatus::ShellOpen);
        Ok(())
    }

    fn get_param(&mut self) -> DriveResult<()> {
        self.send_response(self.drive_status.bits(), 3);
        // TODO: send mode
        //  send 00
        // send file filter
        // send channel filter
        Ok(())
    }

    fn get_loc_l(&mut self) -> DriveResult<()> {
        Ok(())
    }

    fn get_loc_p(&mut self) -> DriveResult<()> {
        Ok(())
    }

    /// Get track number
    fn get_tn(&mut self) -> DriveResult<()> {
        let first_track = to_bcd(1).unwrap(); // TODO: get from .cue..?
        let last_track = to_bcd(1).unwrap();
        self.send_response(self.drive_status.bits(), 3);
        self.send_response(first_track, 3);
        self.send_response(last_track, 3);
        Ok(())
    }

    /// Get track start
    fn get_td(&mut self) -> DriveResult<()> {
        let _track = self.read_parameter()?;
        let minute = to_bcd(0).unwrap(); // TODO: get from .cue..?
        let second = to_bcd(0).unwrap();
        self.send_response(self.drive_status.bits(), 3);
        self.send_response(minute, 3);
        self.send_response(second, 3);
        Ok(())
    }

    fn get_q(&mut self) -> DriveResult<()> {
        Ok(())
    }

    fn get_id(&mut self) -> DriveResult<()> {
        self.send_response(self.drive_status.bits(), 3);
        if self.disc.is_some() {
            self.send_response(0x02, 5); // Stat?
            self.send_response(0x00, 5); // Flags
            self.send_response(0x20, 5); // Mode 2
            self.send_response(0x00, 5);
            // Region String:
            self.send_response(0x53, 5); // S
            self.send_response(0x43, 5); // C
            self.send_response(0x45, 5); // E
            self.send_response(0x41, 5); // [Region: A/E/I] TODO: set based on disc.
        } else {
            self.send_response(0x08, 5); // Stat?
            self.send_response(0x40, 5); // Flags
            self.send_response(0x00, 5);
            self.send_response(0x00, 5);
            // Region String:
            self.send_response(0x00, 5);
            self.send_response(0x00, 5);
            self.send_response(0x00, 5);
            self.send_response(0x00, 5);
        }
        Ok(())
    }
}
