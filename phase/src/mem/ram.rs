
/// A block of read-write RAM.
pub struct RAM {
    data: Vec<u8>
}

impl RAM {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size]
        }
    }
}

impl RAM {
    pub fn read_byte(&self, addr: u32) -> u8 {
        self.data[addr as usize]
    }

    pub fn write_byte(&mut self, addr: u32, data: u8) {
        self.data[addr as usize] = data;
    }

    pub fn read_halfword(&self, addr: u32) -> u16 {
        // Read 2 bytes in little-endian order.
        unsafe {
            let buffer_ptr = self.data.as_ptr();
            let src = buffer_ptr.offset(addr as isize);
            *(src.cast())
        }
    }

    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        // Write 2 bytes in little-endian order.
        unsafe {
            let buffer_ptr = self.data.as_mut_ptr();
            let dest = buffer_ptr.offset(addr as isize);
            *(dest.cast()) = data;
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

    pub fn write_word(&mut self, addr: u32, data: u32) {
        // Write 4 bytes in little-endian order.
        unsafe {
            let buffer_ptr = self.data.as_mut_ptr();
            let dest = buffer_ptr.offset(addr as isize);
            *(dest.cast()) = data;
        }
    }
}