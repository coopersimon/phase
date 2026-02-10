use std::{
    fs::File,
    io::{
        Read,
        Write,
        Seek,
        SeekFrom
    },
    path::Path
};

const MEM_CARD_SIZE: usize = 128 * 1024;
const ID_1: u8 = 0x5A;
const ID_2: u8 = 0x5D;
const CMD_ACK_1: u8 = 0x5C;
const CMD_ACK_2: u8 = 0x5D;
const MEM_CARD_ID: [u8; 4] = [0x04, 0x00, 0x00, 0x80];

/// A 128kB PSX memory card, backed by a file.
pub struct MemoryCard {
    file: File,

    buffer: Vec<u8>,
    dirty: bool,

    mode: Option<MemoryCardMode>,
    flag: u8,
    sector_addr: u16,
    byte_addr: usize,
    transfer_byte: usize,
    checksum: u8,

    sector_ok: bool,
    checksum_ok: bool,
}

impl MemoryCard {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        let mut buffer = vec![0; MEM_CARD_SIZE];
        let (file, dirty) = if std::fs::exists(path)? {
            let mut file = File::options()
                .read(true)
                .write(true)
                .open(path)?;
            file.read_exact(&mut buffer)?;
            (file, false)
        } else {
            let file = File::options()
                .read(true)
                .write(true)
                .create(true)
                .open(path)?;
            (file, true)
        };
        Ok(Self {
            file,

            buffer,
            dirty,

            mode: None,
            flag: 0x08,
            sector_addr: 0,
            byte_addr: 0,
            transfer_byte: 0,
            checksum: 0,

            sector_ok: false,
            checksum_ok: false,
        })
    }

    /// Flush the internal buffer to disk.
    pub fn flush(&mut self) {
        if self.dirty {
            self.file.seek(SeekFrom::Start(0)).expect("could not seek mem card");
            self.file.write(&self.buffer).expect("could not write mem card");
            self.dirty = false;
        }
    }

    /// Transfer a byte to the memory card, and receive a byte.
    pub fn transfer_data(&mut self, data_in: u8) -> u8 {
        let data_out = match self.mode {
            None => self.set_mode(data_in),
            Some(MemoryCardMode::GetID) => {
                let data = match self.transfer_byte {
                    0 => ID_1,
                    1 => ID_2,
                    2 => CMD_ACK_1,
                    3 => CMD_ACK_2,
                    4 => MEM_CARD_ID[0],
                    5 => MEM_CARD_ID[1],
                    6 => MEM_CARD_ID[2],
                    7 => {
                        self.mode = None;
                        MEM_CARD_ID[3]
                    },
                    _ => unreachable!("memcard: too many getid bytes")
                };
                self.transfer_byte += 1;
                data
            },
            Some(MemoryCardMode::Read) => {
                let data = match self.transfer_byte {
                    0 => ID_1,
                    1 => ID_2,
                    2 => {
                        self.set_sector_hi(data_in);
                        0x00
                    },
                    3 => {
                        self.set_sector_lo(data_in);
                        self.get_sector_hi()
                    },
                    4 => CMD_ACK_1,
                    5 => CMD_ACK_2,
                    6 => {
                        if self.sector_ok {
                            self.get_sector_hi()
                        } else {
                            0xFF
                        }
                    },
                    7 => {
                        if self.sector_ok {
                            self.get_sector_lo()
                        } else {
                            self.mode = None;
                            0xFF
                        }
                    },
                    8..=135 => self.read_data(),
                    136 => self.read_checksum(),
                    137 => {
                        self.flag = 0x00;
                        self.mode = None;
                        0x47 // 'G' = Good
                    }
                    _ => unreachable!("memcard: too many read bytes")
                };
                self.transfer_byte += 1;
                data
            },
            Some(MemoryCardMode::Write) => {
                let data = match self.transfer_byte {
                    0 => ID_1,
                    1 => ID_2,
                    2 => {
                        self.set_sector_hi(data_in);
                        0x00
                    },
                    3 => {
                        self.set_sector_lo(data_in);
                        self.get_sector_hi()
                    },
                    4..=131 => {
                        self.write_data(data_in);
                        0x00 // or previous written?
                    },
                    132 => {
                        self.checksum_ok = self.read_checksum() == data_in;
                        if !self.checksum_ok {
                            panic!("checksum provided to mem card does not match");
                        }
                        0x00
                    },
                    133 => CMD_ACK_1,
                    134 => CMD_ACK_2,
                    135 => {
                        //self.flag = 0x08; // ?
                        self.mode = None;
                        if !self.checksum_ok {
                            0x4E
                        } else if !self.sector_ok {
                            0xFF
                        } else {
                            self.dirty = true;
                            0x47 // 'G' = Good
                        }
                    }
                    _ => unreachable!("memcard: too many write bytes")
                };
                self.transfer_byte += 1;
                data
            }
        };
        //println!("MEMCARD (mode {:?}): in {:X} out {:X} (addr: {:X})", self.mode, data_in, data_out, self.byte_addr);
        data_out
    }

    /// Returns true if the memory card transfer is complete.
    pub fn transfer_complete(&self) -> bool {
        self.mode.is_none()
    }

    pub fn cancel_transfer(&mut self) {
        self.mode = None;
    }
}


#[derive(Clone, Copy, Debug)]
enum MemoryCardMode {
    Read,
    Write,
    GetID
}

// Internal.
impl MemoryCard {
    /// Sets the mode of the memory card transfer, and receives the flag byte.
    fn set_mode(&mut self, mode_byte: u8) -> u8 {
        self.transfer_byte = 0;
        self.mode = match mode_byte {
            0x52 => { // read 'R'
                Some(MemoryCardMode::Read)
            },
            0x57 => { // write 'W'
                Some(MemoryCardMode::Write)
            },
            0x53 => { // ID 'S'
                Some(MemoryCardMode::GetID)
            },
            _ => {
                None
            }
        };
        if self.mode.is_some() {
            //std::mem::replace(&mut self.flag, 0x00)
            self.flag
        } else {
            0x00
        }
    }

    fn set_sector_hi(&mut self, data: u8) {
        self.sector_addr = (data as u16) << 8;
        self.checksum = data;
        self.sector_ok = data <= 0x3;
    }

    fn set_sector_lo(&mut self, data: u8) {
        self.sector_addr |= data as u16;
        self.checksum ^= data;
        self.byte_addr = (self.sector_addr as usize) * 128;
    }

    fn get_sector_hi(&self) -> u8 {
        (self.sector_addr >> 8) as u8
    }

    fn get_sector_lo(&self) -> u8 {
        self.sector_addr as u8
    }

    fn read_data(&mut self) -> u8 {
        let data = self.buffer[self.byte_addr];
        self.byte_addr += 1;
        self.checksum ^= data;
        data
    }

    fn write_data(&mut self, data: u8) {
        self.buffer[self.byte_addr] = data;
        self.byte_addr += 1;
        self.checksum ^= data;
    }

    fn read_checksum(&mut self) -> u8 {
        self.checksum
    }
}