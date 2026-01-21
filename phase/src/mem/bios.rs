use std::{
    io::{
        Result,
        Read,
        Seek,
        SeekFrom
    },
    fs::File,
    path::Path
};

const BIOS_SIZE: usize = 512 * 1024;

pub struct BIOS {
    data: Vec<u8>
}

impl BIOS {
    pub fn new(path: Option<&Path>) -> Result<Self> {
        let mut data = vec![0; BIOS_SIZE];
        if let Some(path) = path {
            let mut bios_file = File::open(path)?;
            bios_file.seek(SeekFrom::Start(0))?;
            bios_file.read(&mut data)?;
        }
        Ok(Self {
            data
        })
    }
}

impl BIOS {
    pub fn read_byte(&self, addr: u32) -> u8 {
        self.data[addr as usize]
    }

    pub fn read_halfword(&self, addr: u32) -> u16 {
        // Read 2 bytes in little-endian order.
        unsafe {
            let buffer_ptr = self.data.as_ptr();
            let src = buffer_ptr.offset(addr as isize);
            *(src.cast())
        }
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        // Read 4 bytes in little-endian order.
        unsafe {
            let buffer_ptr = self.data.as_ptr();
            let src = buffer_ptr.offset(addr as isize);
            *(src.cast())
        }
    }
}