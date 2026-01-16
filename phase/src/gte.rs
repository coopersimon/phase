use mips::coproc::Coprocessor;

pub struct GTE {

}

impl Coprocessor for GTE {
    fn load_from_mem(&mut self, reg: usize, val: u32) {
        
    }

    fn store_to_mem(&mut self, reg: usize) -> u32 {
        0
    }

    fn move_from_control(&mut self, reg: usize) -> u32 {
        0
    }

    fn move_to_control(&mut self, reg: usize, val: u32) {
        
    }

    fn move_from_reg(&mut self, reg: usize) -> u32 {
        0
    }

    fn move_to_reg(&mut self, reg: usize, val: u32) {
        
    }

    fn operation(&mut self, op: u32) {
        
    }
}