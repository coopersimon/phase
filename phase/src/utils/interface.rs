
/// Memory interface with a 32-bit data bus.
/// 
/// Reading individual bytes or halfwords might lead to unusual behaviour.
pub trait MemInterface {
    fn read_byte(&mut self, addr: u32) -> u8 {
        let data = self.read_word(addr & 0xFFFF_FFFC);
        data.to_le_bytes()[(addr & 3) as usize]
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        let word_addr = addr & 0xFFFF_FFFC;
        let mut word_data = self.read_word(word_addr).to_le_bytes();
        word_data[(addr & 3) as usize] = data;
        self.write_word(word_addr, u32::from_le_bytes(word_data));
    }

    fn read_halfword(&mut self, addr: u32) -> u16 {
        let data = self.read_word(addr & 0xFFFF_FFFC).to_le_bytes();
        let sub_addr = (addr & 3) as usize;
        u16::from_le_bytes([data[sub_addr], data[sub_addr + 1]])
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        let word_addr = addr & 0xFFFF_FFFC;
        let mut word_data = self.read_word(word_addr).to_le_bytes();
        let sub_addr = (addr & 3) as usize;
        word_data[sub_addr] = data.to_le_bytes()[0];
        word_data[sub_addr + 1] = data.to_le_bytes()[1];
        self.write_word(word_addr, u32::from_le_bytes(word_data));
    }

    fn read_word(&mut self, addr: u32) -> u32;

    fn write_word(&mut self, addr: u32, data: u32);
}