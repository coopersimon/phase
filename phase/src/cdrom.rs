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
    bcd::*,
    interface::MemInterface
};
use std::collections::VecDeque;

/// CD sectors are 2352 bytes.
const SECTOR_SIZE: u64 = 2352;
/// Hold 1 second of data in the memory buffer.
const DISC_BUFFER_SIZE: u64 = 75 * SECTOR_SIZE;
/// Each sector starts with 12 sync bytes.
const SECTOR_SYNC_BYTES: u64 = 12;
/// Each sector starts with a 24 byte header, including
/// sync bytes, address, mode
const SECTOR_HEADER: u64 = 24;
/// Each sector contains 2048 bytes of data.
const SECTOR_DATA: u64 = 2048;

/// CD-ROM reader.
pub struct CDROM {
    disc: Option<File>,
    buffer: Vec<u8>,
    buffer_file_offset: u64,
    seek_file_offset: u64,
    sector_offset: u64,

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
    seeked: bool,

    counter: usize,
    command: u8,
    response_count: u8,
    data_fifo_size: u64,
}

impl CDROM {
    pub fn new() -> Self {
        Self {
            disc: None,
            buffer: vec![0; DISC_BUFFER_SIZE as usize],
            buffer_file_offset: u64::MAX,
            seek_file_offset: 0,
            sector_offset: 0,

            status: Status::ParamFifoEmpty | Status::ParamFifoFull,
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
            loc: DriveLoc { minute: 0, second: 0, sector: 0 },
            seeked: false,

            counter: 0,
            command: 0,
            response_count: 0,
            data_fifo_size: 0,
        }
    }

    /// Insert or remove a disc from the PlayStation.
    pub fn insert_disc(&mut self, path: Option<&Path>) -> std::io::Result<()> {
        self.drive_status.insert(DriveStatus::ShellOpen);
        if let Some(path) = path {
            let disc_file = File::open(path)?;
            self.disc = Some(disc_file);
            self.buffer_file_offset = u64::MAX;
            self.seek_file_offset = 0;
            self.sector_offset = 0;
            self.read_from_file();
        } else {
            self.disc = None
        }
        Ok(())
    }

    /// Clock the CD-ROM reader.
    /// 
    /// Returns an interrupt if it occurred.
    pub fn clock(&mut self, cycles: usize) -> Interrupt {
        if self.counter > 0 {
            self.counter = self.counter.saturating_sub(cycles);
            if self.counter == 0 {
                self.exec_command();
            }
        }
        if self.check_irq() {
            Interrupt::CDROM
        } else {
            Interrupt::empty()
        }
    }
}

impl MemInterface for CDROM {
    fn read_byte(&mut self, addr: u32) -> u8 {
        let data = match addr {
            0x1F80_1800 => self.status.bits(),
            0x1F80_1801 => self.read_response(),
            0x1F80_1802 => self.read_data(),
            0x1F80_1803 => match self.index() {
                0 | 2 => self.int_enable.bits(),
                1 | 3 => self.int_flags.bits(),
                _ => unreachable!()
            },
            _ => panic!("invalid CDROM addr {:X}", addr)
        };
        //println!("read cd {:X}.{}: {:X}", addr, self.index(), data);
        data
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        //println!("write cd {:X}.{}: {:X}", addr, self.index(), data);
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
        Data { data, cycles: 23 }
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
        const ResetParamFIFO= bit!(6);
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
        (self.int_enable & self.int_flags).intersects(!IntFlags::Unused)
    }

    fn write_status(&mut self, data: u8) {
        self.status.remove(Status::PortIndex);
        self.status.insert(Status::from_bits_truncate(data) & Status::PortIndex);
    }

    fn index(&self) -> u8 {
        (self.status & Status::PortIndex).bits()
    }

    fn write_command(&mut self, data: u8) {
        self.counter = 50000; // wait some arbitrary amount of time TODO: make this more accurate
        self.response_count = 0;
        self.command = data;
        self.status.insert(Status::Busy);
    }

    fn write_parameter(&mut self, data: u8) {
        if self.param_fifo.len() >= 16 {
            panic!("param fifo len too long");
        }
        self.param_fifo.push_back(data);
        self.status.remove(Status::ParamFifoEmpty);
        self.status.set(Status::ParamFifoFull, self.param_fifo.len() < 16);
    }

    fn read_parameter(&mut self) -> DriveResult<u8> {
        let param = self.param_fifo.pop_front();
        if let Some(param) = param {
            self.status.set(Status::ParamFifoEmpty, self.param_fifo.is_empty());
            if self.param_fifo.len() < 16 {
                self.status.insert(Status::ParamFifoFull);
            }
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
        let data_in = IntFlags::from_bits_truncate(data);
        if data_in.contains(IntFlags::ResetParamFIFO) {
            self.param_fifo.clear();
            self.status.insert(Status::ParamFifoEmpty);
            self.status.insert(Status::ParamFifoFull);
        }
        self.int_flags.remove(data_in);
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

    /// Indicate first response has been sent.
    fn first_response(&mut self) -> DriveResult<()> {
        self.counter = 50000;
        self.response_count += 1;
        Ok(())
    }

    /// Indicate final response has been sent.
    fn command_complete(&mut self) -> DriveResult<()> {
        self.status.remove(Status::Busy);
        Ok(())
    }

    fn read_data(&mut self) -> u8 {
        let index = self.sector_offset as usize;
        let data = self.buffer[index];
        self.sector_offset += 1;
        self.data_fifo_size -= 1;
        if self.data_fifo_size == 0 {
            self.counter = 50000;
            self.status.remove(Status::DataFifoEmpty);
            self.seek_file_offset += SECTOR_SIZE;
        }
        data
    }

    /// Read a sector.
    fn read_sector(&mut self) {
        // Check if we need to load from disc.
        println!("CD read @ {:X}", self.seek_file_offset);
        self.read_from_file();
        let start_pos = self.seek_file_offset - self.buffer_file_offset;
        if self.mode.contains(DriveMode::SectorSize) {
            self.sector_offset = start_pos + SECTOR_SYNC_BYTES;
            self.data_fifo_size = SECTOR_DATA + SECTOR_SYNC_BYTES;
        } else {
            self.sector_offset = start_pos + SECTOR_HEADER;
            self.data_fifo_size = SECTOR_DATA;
        }
        self.status.insert(Status::DataFifoEmpty);
    }

    /// Read from disc into buffer, if necessary.
    /// 
    /// It will read the sector pointed to by the offset, in addition to
    /// other nearby sectors.
    fn read_from_file(&mut self) {
        let chunk_num = self.seek_file_offset / DISC_BUFFER_SIZE;
        let target_file_offset = chunk_num * DISC_BUFFER_SIZE;
        if self.buffer_file_offset == target_file_offset {
            // No read necessary.
            return;
        }
        let Some(disc_file) = self.disc.as_mut() else {
            return;
        };
        self.buffer_file_offset = target_file_offset;
        disc_file.seek(SeekFrom::Start(self.buffer_file_offset)).expect("could not seek in disc");
        disc_file.read(&mut self.buffer).expect("could not load disc data");
        println!("CD load from disc @ {:X}", self.buffer_file_offset);
    }

    /// Inspect the final sector.
    fn get_final_sector(&self) -> DriveResult<DriveLoc> {
        let Some(disc_file) = self.disc.as_ref() else {
            return Err(DriveError::SeekFailed);
        };
        let metadata = disc_file.metadata().expect("could not get file metadata");
        let file_len = metadata.len();
        let sector_count = file_len / SECTOR_SIZE;
        let total_seconds = (sector_count / 75) + 2; // Round down to nearest second, and offset by 2.
        let minute = (total_seconds / 60) as u8;
        let second = (total_seconds % 60) as u8;
        Ok(DriveLoc { minute, second, sector: 0 })
    }

    /// Execute the current command.
    fn exec_command(&mut self) {
        // TODO: command interrupt!
        println!("cd command: {:X}", self.command);
        let res = match self.command {
            0x00 => self.sync(),
            0x01 => self.get_stat(),
            0x02 => self.set_loc(),
            0x06 => self.read_n(),
            0x07 => self.motor_on(),
            0x08 => self.stop(),
            0x09 => self.pause(),
            0x0A => self.init(),
            0x0B => self.mute(),
            0x0C => self.demute(),
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
            0x19 => self.subfunction(),
            0x1A => self.get_id(),
            0x1B => self.read_s(),
            0x1D => self.get_q(),
            0x1E => self.read_toc(),
            _ => panic!("unknown CD-ROM command {:X}", self.command),
        };
        if let Err(res) = res {
            self.send_response(res.bits(), 5);
            self.status.remove(Status::Busy);
        }
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
    sector: u8, // 75 sectors per second
}

impl DriveLoc {
    fn byte_offset(&self) -> u64 {
        const SEC_SIZE: u64 = 75 * SECTOR_SIZE;
        const MIN_SIZE: u64 = 60 * SEC_SIZE;
        const ROOT_OFFSET: u64 = 2 * SEC_SIZE;
        let sector_offset = (self.sector as u64) * SECTOR_SIZE;
        let sec_offset = (self.second as u64) * SEC_SIZE;
        let min_offset = (self.minute as u64) * MIN_SIZE;
        min_offset + sec_offset + sector_offset - ROOT_OFFSET
    }
}

type DriveResult<T> = Result<T, DriveError>;

// Commands.
impl CDROM {
    fn sync(&mut self) -> DriveResult<()> {
        self.command_complete()
    }

    fn set_filter(&mut self) -> DriveResult<()> {
        let _file = self.read_parameter()?;
        let _channel = self.read_parameter()?;
        self.send_response(self.drive_status.bits(), 3);
        self.command_complete()
    }

    fn set_mode(&mut self) -> DriveResult<()> {
        let mode = self.read_parameter()?;
        println!("Set mode: {:X}", mode);
        self.mode = DriveMode::from_bits_truncate(mode);
        self.send_response(self.drive_status.bits(), 3);
        self.command_complete()
    }

    fn init(&mut self) -> DriveResult<()> {
        match self.response_count {
            0 => {
                self.send_response(self.drive_status.bits(), 3);
                self.first_response()
            },
            _ => {
                self.mode = DriveMode::SectorSize;
                self.drive_status.insert(DriveStatus::SpindleMotor);
                self.send_response(self.drive_status.bits(), 2);
                self.command_complete()
            }
        }
    }

    fn motor_on(&mut self) -> DriveResult<()> {
        if self.drive_status.contains(DriveStatus::SpindleMotor) {
            self.drive_status.insert(DriveStatus::Error);
            Err(DriveError::MissingParam)
        } else {
            match self.response_count {
                0 => {
                    self.send_response(self.drive_status.bits(), 3);
                    self.first_response()
                },
                _ => {
                    self.drive_status.insert(DriveStatus::SpindleMotor);
                    self.send_response(self.drive_status.bits(), 2);
                    self.command_complete()
                }
            }
        }
    }

    fn stop(&mut self) -> DriveResult<()> {
        match self.response_count {
            0 => {
                self.drive_status.remove(DriveStatus::ReadBits);
                self.send_response(self.drive_status.bits(), 3);
                self.first_response()
            },
            _ => {
                self.drive_status.remove(DriveStatus::SpindleMotor);
                self.send_response(self.drive_status.bits(), 2);
                self.command_complete()
            }
        }
    }

    fn pause(&mut self) -> DriveResult<()> {
        match self.response_count {
            0 => {
                self.send_response(self.drive_status.bits(), 3);
                self.first_response()
            },
            _ => {
                self.drive_status.remove(DriveStatus::ReadBits);
                self.send_response(self.drive_status.bits(), 2);
                self.command_complete()
            }
        }
    }

    fn set_loc(&mut self) -> DriveResult<()> {
        self.loc.minute = from_bcd(self.read_parameter()?).ok_or(DriveError::InvalidParam)?; // mm
        self.loc.second = from_bcd(self.read_parameter()?).ok_or(DriveError::InvalidParam)?; // ss
        self.loc.sector = from_bcd(self.read_parameter()?).ok_or(DriveError::InvalidParam)?; // sector / frame
        println!("Seek to {:X},{:X},{:X}", self.loc.minute, self.loc.second, self.loc.sector);
        self.seeked = false;
        self.send_response(self.drive_status.bits(), 3);
        self.command_complete()
    }

    /// Data seek
    fn seek_l(&mut self) -> DriveResult<()> {
        match self.response_count {
            0 => {
                self.drive_status.remove(DriveStatus::ReadBits);
                self.drive_status.insert(DriveStatus::Seeking);
                self.drive_status.insert(DriveStatus::SpindleMotor);
                self.send_response(self.drive_status.bits(), 3);
                self.first_response()
            },
            _ => {
                self.seek_file_offset = self.loc.byte_offset();
                self.seeked = true;
                self.send_response(self.drive_status.bits(), 2);
                self.command_complete()
            }
        }
    }

    /// Audio seek
    fn seek_p(&mut self) -> DriveResult<()> {
        unimplemented!("audio seek");
        /*self.drive_status.remove(DriveStatus::ReadBits);
        self.drive_status.insert(DriveStatus::Seeking);
        self.drive_status.insert(DriveStatus::SpindleMotor);
        let _byte_offset = self.loc.byte_offset();
        // TODO ? (subchannel Q)
        self.command_complete();
        Ok(())*/
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
            self.command_complete()
        }
    }

    /// Read with retry
    fn read_n(&mut self) -> DriveResult<()> {
        match self.response_count {
            0 => {
                self.drive_status.remove(DriveStatus::ReadBits);
                self.drive_status.insert(DriveStatus::Seeking);
                self.send_response(self.drive_status.bits(), 3);
                self.first_response()
            },
            _ => {
                // We need to seek if we have an unprocessed seek.
                if !self.seeked {
                    self.seek_file_offset = self.loc.byte_offset();
                    self.seeked = true;
                }
                self.drive_status.remove(DriveStatus::ReadBits);
                self.drive_status.insert(DriveStatus::Reading);
                self.read_sector();
                self.send_response(self.drive_status.bits(), 1);
                self.command_complete()
            }
        }
    }

    /// Read without retry
    fn read_s(&mut self) -> DriveResult<()> {
        self.read_n()
    }

    /// Read table of contents
    fn read_toc(&mut self) -> DriveResult<()> {
        // TODO: .cue file?
        match self.response_count {
            0 => {
                self.send_response(self.drive_status.bits(), 3);
                self.first_response()
            },
            _ => {
                self.send_response(self.drive_status.bits(), 2);
                self.command_complete()
            }
        }
    }

    fn get_stat(&mut self) -> DriveResult<()> {
        self.send_response(self.drive_status.bits(), 3);
        self.drive_status.remove(DriveStatus::ShellOpen);
        self.command_complete()
    }

    fn get_param(&mut self) -> DriveResult<()> {
        unimplemented!("get param");
        self.send_response(self.drive_status.bits(), 3);
        // TODO: send mode
        //  send 00
        // send file filter
        // send channel filter
        self.command_complete()
    }

    fn get_loc_l(&mut self) -> DriveResult<()> {
        unimplemented!("get loc l");
        self.command_complete()
    }

    fn get_loc_p(&mut self) -> DriveResult<()> {
        // TODO: verify this.
        self.send_response(0x01, 3); // Track number
        self.send_response(0x01, 3); // Index number
        self.send_response(0x00, 3); // Minute number
        self.send_response(0x00, 3); // Second number
        self.send_response(0x00, 3); // Sector number
        self.send_response(0x00, 3); // Minute number
        self.send_response(0x00, 3); // Second number
        self.send_response(0x00, 3); // Sector number
        self.command_complete()
    }

    /// Get track number
    fn get_tn(&mut self) -> DriveResult<()> {
        // Assume 1 track. (TODO: read .cue files)
        let first_track = to_bcd(1).unwrap();
        let last_track = to_bcd(1).unwrap();
        self.send_response(self.drive_status.bits(), 3);
        self.send_response(first_track, 3);
        self.send_response(last_track, 3);
        self.command_complete()
    }

    /// Get track start
    fn get_td(&mut self) -> DriveResult<()> {
        // Assume 1 track. (TODO: read .cue files)
        let track = from_bcd(self.read_parameter()?).ok_or(DriveError::InvalidParam)?;
        println!("get TD {}", track);
        match track {
            0 => { // End of last track.
                let loc = self.get_final_sector()?;
                self.send_response(self.drive_status.bits(), 3);
                self.send_response(to_bcd(loc.minute).unwrap(), 3);
                self.send_response(to_bcd(loc.second).unwrap(), 3);
                self.command_complete()
            },
            1 => {
                let minute = to_bcd(0).unwrap();
                let second = to_bcd(0).unwrap();
                self.send_response(self.drive_status.bits(), 3);
                self.send_response(minute, 3);
                self.send_response(second, 3);
                self.command_complete()
            },
            _ => {
                Err(DriveError::InvalidParam)
            }
        }
    }

    fn get_q(&mut self) -> DriveResult<()> {
        unimplemented!("get Q")
    }

    fn get_id(&mut self) -> DriveResult<()> {
        match self.response_count {
            0 => {
                self.send_response(self.drive_status.bits(), 3);
                self.first_response()
            },
            _ => {
                if self.disc.is_some() {
                    self.send_response(0x02, 2); // Stat?
                    self.send_response(0x00, 2); // Flags
                    self.send_response(0x20, 2); // Mode 2
                    self.send_response(0x00, 2);
                    // Region String:
                    self.send_response(0x53, 2); // S
                    self.send_response(0x43, 2); // C
                    self.send_response(0x45, 2); // E
                    self.send_response(0x41, 2); // [Region: A/E/I] TODO: set based on disc.
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
                self.command_complete()
            }
        }
    }

    fn subfunction(&mut self) -> DriveResult<()> {
        let op = self.read_parameter()?;
        match op {
            0x20 => { // CDROM BIOS
                self.send_response(0x95, 3); // yy
                self.send_response(0x05, 3); // mm
                self.send_response(0x16, 3); // dd
                self.send_response(0xC1, 3); // version
                self.command_complete()
            },
            _ => panic!("unsupported CD subfunction {:X}", op),
        }
    }

    fn mute(&mut self) -> DriveResult<()> {
        self.send_response(self.drive_status.bits(), 3);
        self.command_complete()
    }

    fn demute(&mut self) -> DriveResult<()> {
        // TODO: start audio streaming..?
        self.send_response(self.drive_status.bits(), 3);
        self.command_complete()
    }
}
