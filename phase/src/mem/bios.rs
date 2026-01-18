
pub struct BIOS {
    data: Vec<u8>
}

impl BIOS {
    // TODO.
    pub fn new() -> Self {
        let data = vec![0; 512 * 1024];
        Self {
            data
        }
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