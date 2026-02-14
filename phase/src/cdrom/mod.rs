mod disc;
mod xaaudio;

use mips::mem::Data;
use dasp::frame::Stereo;

use disc::Disc;
use xaaudio::XAAudio;
use crate::{interrupt::Interrupt, mem::DMADevice};
use crate::utils::{
    bits::*,
    bcd::*,
    interface::MemInterface
};
use std::collections::VecDeque;

/// CD sectors are 2352 bytes.
const SECTOR_SIZE: u64 = 2352;
/// Each sector starts with 12 sync bytes.
const SECTOR_SYNC_BYTES: u64 = 12;
/// Each sector starts with a 24 byte header, including
/// sync bytes, address, mode
const SECTOR_HEADER: u64 = 24;
/// Each sector contains 2048 bytes of data.
const SECTOR_DATA: u64 = 2048;

const COMMAND_CYCLES: usize = 24000;
/// 1x read cycles
const READ_CYCLES: usize = 451584;
/// Varies in reality, just an arbitrary amount here.
const SEEK_CYCLES: usize = 300000;

/// CD-ROM reader.
pub struct CDROM {
    disc: Option<Disc>,
    seek_file_offset: u64,

    status: Status,
    int_enable: IntFlags,
    int_flags: IntFlags,
    request: Request,

    xa_audio: XAAudio,

    param_fifo: VecDeque<u8>,
    response_fifo: VecDeque<u8>,

    drive_status: DriveStatus,
    mode: DriveMode,
    loc: DriveLoc,
    /// Determines if we have done setloc,
    /// and are pending a seek or read operation.
    pending_seek: bool,
    /// Determines if we are in seek mode.
    seeking: bool,
    read_data_counter: usize,
    current_sector_header: SectorHeader,

    counter: usize,
    command: u8,
    response_count: u8,
    data_fifo_size: u64,
    irq_latch: bool,
}

impl CDROM {
    pub fn new() -> Self {
        Self {
            disc: None,
            seek_file_offset: 0,

            status: Status::ParamFifoEmpty | Status::ParamFifoNotFull,
            int_enable: IntFlags::Unused,
            int_flags: IntFlags::Unused,
            request: Request::empty(),

            xa_audio: XAAudio::new(),

            param_fifo: VecDeque::new(),
            response_fifo: VecDeque::new(),

            drive_status: DriveStatus::empty(),
            mode: DriveMode::empty(),
            loc: DriveLoc { minute: 0, second: 0, sector: 0 },
            pending_seek: false,
            seeking: false,
            read_data_counter: 0,
            current_sector_header: SectorHeader::default(),

            counter: 0,
            command: 0,
            response_count: 0,
            data_fifo_size: 0,
            irq_latch: false,
        }
    }

    /// Insert or remove a disc from the PlayStation.
    pub fn insert_disc(&mut self, path: Option<&std::path::Path>) -> std::io::Result<()> {
        self.drive_status.insert(DriveStatus::ShellOpen);
        if let Some(path) = path {
            let mut disc = Disc::new(path)?;
            self.seek_file_offset = 0;
            disc.load_from_file(0, self.seek_file_offset);
            self.disc = Some(disc);
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
        if self.read_data_counter > 0 {
            self.read_data_counter = self.read_data_counter.saturating_sub(cycles);
            if self.read_data_counter == 0 {
                if self.seeking {
                    self.drive_status.remove(DriveStatus::ReadBits);
                    self.drive_status.insert(DriveStatus::Reading);
                    self.read_data_counter = self.get_read_cycles();
                    self.seeking = false;
                } else {
                    if self.read_sector() {
                        self.send_response(self.drive_status.bits(), 1);
                    }
                }
                self.status.remove(Status::ADPBusy); //?
            }
        }
        if self.check_irq() {
            Interrupt::CDROM
        } else {
            Interrupt::empty()
        }
    }

    /// This method can be used to retrieve the decoded audio samples
    /// for transportation to SPU, if new ones are ready.
    pub fn fetch_decoded_audio<'a> (&'a mut self) -> Option<&'a [Stereo<i16>]> {
        self.xa_audio.fetch_decoded_audio()
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
                1 => self.xa_audio.write_data(data),
                2 => self.xa_audio.set_sound_map_info(data),
                3 => self.xa_audio.set_right_to_right(data),
                _ => unreachable!()
            },
            0x1F80_1802 => match self.index() {
                0 => self.write_parameter(data),
                1 => self.set_int_enable(data),
                2 => self.xa_audio.set_left_to_left(data),
                3 => self.xa_audio.set_right_to_left(data),
                _ => unreachable!()
            },
            0x1F80_1803 => match self.index() {
                0 => self.write_request(data),
                1 => self.set_int_flags(data),
                2 => self.xa_audio.set_left_to_right(data),
                3 => self.xa_audio.apply_changes(data),
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
        Data { data, cycles: 8 }
    }

    fn dma_write_word(&mut self, _data: u32) -> usize {
        panic!("not valid to use DMA to write to CDROM!")
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct Status: u8 {
        const Busy              = bit!(7);
        const DataFifoNotEmpty  = bit!(6);  // 0 = Empty
        const ResFifoNotEmpty   = bit!(5);  // 0 = Empty
        const ParamFifoNotFull  = bit!(4);  // 0 = Full
        const ParamFifoEmpty    = bit!(3);  // 1 = Empty
        const ADPBusy           = bit!(2);  // 0 = Empty
        const PortIndex         = bits![0, 1];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct IntFlags: u8 {
        const ResetParamFIFO= bit!(6);
        const CommandStart  = bit!(4);
        const Unknown       = bit!(3);
        const Response      = bits![0, 1, 2];

        const Unused        = bits![5, 6, 7];
        const IntBits       = bits![0, 1, 2, 3, 4];
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

bitflags::bitflags! {
    #[derive(Clone, Copy, Default)]
    struct CodingInfo: u8 {
        const Emphasis      = bit!(6);
        const BitsPerSample = bit!(4);
        const SampleRate    = bit!(2);
        const Stereo        = bit!(0);
    }
}

// Internal.
impl CDROM {
    fn check_irq(&mut self) -> bool {
        std::mem::take(&mut self.irq_latch)
    }

    fn write_status(&mut self, data: u8) {
        self.status.remove(Status::PortIndex);
        self.status.insert(Status::from_bits_truncate(data).intersection(Status::PortIndex));
    }

    fn index(&self) -> u8 {
        (self.status.intersection(Status::PortIndex)).bits()
    }

    fn write_command(&mut self, data: u8) {
        self.counter = COMMAND_CYCLES;
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
        self.status.set(Status::ParamFifoNotFull, self.param_fifo.len() < 16);
    }

    fn read_parameter(&mut self) -> DriveResult<u8> {
        let param = self.param_fifo.pop_front();
        if let Some(param) = param {
            self.status.set(Status::ParamFifoEmpty, self.param_fifo.is_empty());
            if self.param_fifo.len() < 16 {
                self.status.insert(Status::ParamFifoNotFull);
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
        if (self.int_enable & self.int_flags).intersects(IntFlags::IntBits) {
            self.irq_latch = true;
        }
    }

    fn set_int_flags(&mut self, data: u8) {
        let data_in = IntFlags::from_bits_truncate(data);
        if data_in.contains(IntFlags::ResetParamFIFO) {
            self.param_fifo.clear();
            self.status.insert(Status::ParamFifoEmpty);
            self.status.insert(Status::ParamFifoNotFull);
        }
        self.int_flags.remove(data_in);
        self.int_flags.insert(IntFlags::Unused);
        self.irq_latch = false;
    }
    
    fn read_response(&mut self) -> u8 {
        let data = self.response_fifo.pop_front();
        if self.response_fifo.is_empty() {
            self.status.remove(Status::ResFifoNotEmpty);
        }
        data.unwrap_or(0)
    }

    /// Write response from command.
    /// 
    /// Also sets interrupt bits. Int should be a value 1-7.
    fn send_response(&mut self, data: u8, int: u8) {
        self.response_fifo.push_back(data);
        self.status.insert(Status::ResFifoNotEmpty);
        self.int_flags.remove(IntFlags::Response);
        self.int_flags.insert(IntFlags::from_bits_truncate(int));
        if (self.int_enable & self.int_flags).intersects(IntFlags::IntBits) {
            self.irq_latch = true;
        }
    }

    /// Indicate first response has been sent.
    fn first_response(&mut self) -> DriveResult<()> {
        self.counter = COMMAND_CYCLES;
        self.response_count += 1;
        Ok(())
    }

    /// Indicate first response has been sent for a seeking or pausing command.
    fn begin_seek(&mut self) -> DriveResult<()> {
        self.counter = SEEK_CYCLES;
        self.response_count += 1;
        Ok(())
    }

    /// Indicate final response has been sent.
    fn command_complete(&mut self) -> DriveResult<()> {
        self.command = 0;
        self.status.remove(Status::Busy);
        Ok(())
    }

    fn read_data(&mut self) -> u8 {
        let data = self.disc.as_mut().map(|d| d.read_byte()).unwrap_or_default();
        self.data_fifo_size -= 1;
        if self.data_fifo_size == 0 {
            self.status.remove(Status::DataFifoNotEmpty);
        }
        data
    }

    fn get_read_cycles(&self) -> usize {
        if self.mode.contains(DriveMode::Speed) {READ_CYCLES / 2} else {READ_CYCLES}
    }

    /// Read a sector.
    /// 
    /// Returns true if the sector is read as data,
    /// and as such we need to trigger interrupt 1.
    fn read_sector(&mut self) -> bool {
        // Check if we need to load from disc.
        println!("CD read @ {:X}", self.seek_file_offset);
        if let Some(disc) = self.disc.as_mut() {
            // TODO: set track.
            disc.load_from_file(0, self.seek_file_offset);
            disc.set_sector_offset(self.seek_file_offset);
            self.current_sector_header = SectorHeader::from_slice(disc.ref_sector_data(SECTOR_SYNC_BYTES, 8));
        } else {
            // No disc inserted.
            return false;
        };
        let trigger_int_1 = if self.send_xa_adpcm_sector() {
            false
        } else {
            // Send as data.
            if self.mode.contains(DriveMode::SectorSize) {
                if let Some(disc) = self.disc.as_mut() {
                    disc.adjust_sector_offset(SECTOR_SYNC_BYTES);
                }
                self.data_fifo_size = SECTOR_SIZE - SECTOR_SYNC_BYTES;
            } else {
                if let Some(disc) = self.disc.as_mut() {
                    disc.adjust_sector_offset(SECTOR_HEADER);
                }
                self.data_fifo_size = SECTOR_DATA;
            }
            self.status.insert(Status::DataFifoNotEmpty);
            true
        };
        // Begin count down for the next read.
        self.read_data_counter = self.get_read_cycles();
        self.seek_file_offset += SECTOR_SIZE;
        trigger_int_1
    }

    /// Try and send the read sector as XA-ADPCM to SPU.
    /// 
    /// The sector might not be ADPCM, in which case, this
    /// will return false.
    fn send_xa_adpcm_sector(&mut self) -> bool {
        if self.mode.contains(DriveMode::XAADPCM) {
            //println!("try XA-ADPCM: {:X} {:X} {:X} {:X}", self.current_sector_header.file, self.current_sector_header.channel, self.current_sector_header.submode.bits(), self.current_sector_header.coding);
            if !self.current_sector_header.submode.contains(CDSectorSubmode::Audio | CDSectorSubmode::RealTime) {
                return false;
            }
            self.status.insert(Status::ADPBusy);
            if self.mode.contains(DriveMode::XAFilter) {
                if !self.xa_audio.test_filter(self.current_sector_header.file, self.current_sector_header.channel) {
                    // Skip this sector.
                    return true;
                }
            }
            if let Some(disc) = self.disc.as_ref() {
                let buffer = disc.ref_sector_data(SECTOR_HEADER, 0x900);
                self.xa_audio.write_audio_sector(buffer, self.current_sector_header.coding);
            }
            true
        } else {
            false
        }
    }

    /// Inspect the final sector.
    fn get_final_sector(&mut self) -> DriveResult<DriveLoc> {
        let Some(disc) = self.disc.as_mut() else {
            return Err(DriveError::SeekFailed);
        };
        Ok(disc.get_end_pos())
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
    #[derive(Clone, Copy, PartialEq)]
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

bitflags::bitflags! {
    #[derive(Default, Clone, Copy)]
    struct CDSectorSubmode: u8 {
        const EOF       = bit!(7);
        const RealTime  = bit!(6);
        const Form2     = bit!(5);
        const Trigger   = bit!(4);
        const Data      = bit!(3);
        const Audio     = bit!(2);
        const Video     = bit!(1);
        const EOR       = bit!(0);
    }
}

/// The sector header appears after the sync section.
/// The final 4 bytes are duplicated.
#[derive(Clone, Default)]
struct SectorHeader {
    minute:  u8, // Stored as BCD.
    second:  u8, // Stored as BCD.
    sector:  u8, // Stored as BCD.
    mode:    u8,
    // Subheader:
    file:    u8,
    channel: u8,
    submode: CDSectorSubmode,
    coding:  CodingInfo,
}

impl SectorHeader {
    fn from_slice(data: &[u8]) -> Self {
        Self {
            minute:  data[0],
            second:  data[1],
            sector:  data[2],
            mode:    data[3],
            file:    data[4],
            channel: data[5],
            submode: CDSectorSubmode::from_bits_truncate(data[6]),
            coding:  CodingInfo::from_bits_truncate(data[7]),
        }
    }
}

type DriveResult<T> = Result<T, DriveError>;

// Commands.
impl CDROM {
    fn sync(&mut self) -> DriveResult<()> {
        self.command_complete()
    }

    fn set_filter(&mut self) -> DriveResult<()> {
        let file_filter = self.read_parameter()?;
        let channel_filter = self.read_parameter()?;
        self.xa_audio.set_filters(file_filter, channel_filter);
        self.send_response(self.drive_status.bits(), 3);
        self.command_complete()
    }

    fn set_mode(&mut self) -> DriveResult<()> {
        let mode = self.read_parameter()?;
        println!("Set mode: {:X}", mode);
        let new_mode = DriveMode::from_bits_truncate(mode);
        if new_mode != self.mode {
            // There are a few cases of games modifying the mode
            // mid-read, before firing off the new read. This can
            // lead to issues when the "old" read arrives and tries
            // to be read with the "new" mode.
            self.read_data_counter = 0;
        }
        self.mode = new_mode;
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
        // This should return an error as follows.
        // However certain games (Chrono Cross) call this command after init.
        /*if self.drive_status.contains(DriveStatus::SpindleMotor) {
            self.drive_status.insert(DriveStatus::Error);
            return Err(DriveError::MissingParam);
        }*/
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

    fn stop(&mut self) -> DriveResult<()> {
        match self.response_count {
            0 => {
                self.drive_status.remove(DriveStatus::ReadBits);
                self.read_data_counter = 0;
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
                self.read_data_counter = 0;
                self.begin_seek()
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
        self.pending_seek = true;
        self.send_response(self.drive_status.bits(), 3);
        self.command_complete()
    }

    /// Data seek
    fn seek_l(&mut self) -> DriveResult<()> {
        match self.response_count {
            0 => {
                self.pending_seek = false;
                self.seeking = true;
                self.drive_status.remove(DriveStatus::ReadBits);
                self.drive_status.insert(DriveStatus::Seeking);
                self.drive_status.insert(DriveStatus::SpindleMotor);
                self.send_response(self.drive_status.bits(), 3);
                self.begin_seek()
            },
            _ => {
                self.seek_file_offset = self.loc.byte_offset();
                self.seeking = false;
                self.drive_status.remove(DriveStatus::ReadBits);
                self.send_response(self.drive_status.bits(), 2);
                self.command_complete()
            }
        }
    }

    /// Audio seek
    fn seek_p(&mut self) -> DriveResult<()> {
        //self.seek_l()
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
        self.drive_status.remove(DriveStatus::ReadBits);
        if self.pending_seek {
            self.seek_file_offset = self.loc.byte_offset();
            self.pending_seek = false;
            self.seeking = true;
            self.drive_status.insert(DriveStatus::Seeking);
            self.read_data_counter = SEEK_CYCLES;
        } else {
            self.seeking = false;
            self.drive_status.insert(DriveStatus::Reading);
            self.read_data_counter = self.get_read_cycles();
        }
        self.send_response(self.drive_status.bits(), 3);
        self.command_complete()
    }

    /// Read without retry
    fn read_s(&mut self) -> DriveResult<()> {
        self.read_n()
    }

    /// Read table of contents
    fn read_toc(&mut self) -> DriveResult<()> {
        // This doesn't return anything interesting.
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
        println!("get stat: {:X}", self.drive_status.bits());
        self.send_response(self.drive_status.bits(), 3);
        self.drive_status.remove(DriveStatus::ShellOpen);
        self.command_complete()
    }

    fn get_param(&mut self) -> DriveResult<()> {
        self.send_response(self.drive_status.bits(), 3);
        self.send_response(self.mode.bits(), 3);
        self.send_response(0x00, 3);
        let (file_filter, channel_filter) = self.xa_audio.get_filters();
        self.send_response(file_filter, 3);
        self.send_response(channel_filter, 3);
        self.command_complete()
    }

    fn get_loc_l(&mut self) -> DriveResult<()> {
        self.send_response(self.current_sector_header.minute, 3);
        self.send_response(self.current_sector_header.second, 3);
        self.send_response(self.current_sector_header.sector, 3);
        self.send_response(self.current_sector_header.mode, 3);
        self.send_response(self.current_sector_header.file, 3);
        self.send_response(self.current_sector_header.channel, 3);
        self.send_response(self.current_sector_header.submode.bits(), 3);
        self.send_response(self.current_sector_header.coding.bits(), 3);
        self.command_complete()
    }

    fn get_loc_p(&mut self) -> DriveResult<()> {
        // TODO: verify this.
        self.send_response(0x01, 3); // Track number
        self.send_response(0x01, 3); // Index number
        self.send_response(self.current_sector_header.minute, 3); // Minute number
        self.send_response(self.current_sector_header.second, 3); // Second number
        self.send_response(self.current_sector_header.sector, 3); // Sector number
        self.send_response(self.current_sector_header.minute, 3); // Minute number
        self.send_response(self.current_sector_header.second, 3); // Second number
        self.send_response(self.current_sector_header.sector, 3); // Sector number
        self.command_complete()
    }

    /// Get track number
    fn get_tn(&mut self) -> DriveResult<()> {
        // Assume 1 track. (TODO: read .cue files)
        let first_track = to_bcd(1).unwrap();
        let last_track = to_bcd(1).unwrap();
        println!("get TN {:X} => {:X}", first_track, last_track);
        self.send_response(self.drive_status.bits(), 3);
        self.send_response(first_track, 3);
        self.send_response(last_track, 3);
        self.command_complete()
    }

    /// Get track start
    fn get_td(&mut self) -> DriveResult<()> {
        // Assume 1 track. (TODO: read .cue files)
        let track = from_bcd(self.read_parameter()?).ok_or(DriveError::InvalidParam)?;
        println!("get TD {:X}", track);
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
                let second = to_bcd(2).unwrap();
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
                    self.send_response(0x41, 2); // [Region: A/E/I] TODO: set based on disc. (41, 45, 49)
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
