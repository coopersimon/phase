use mips::coproc::Coprocessor;

pub struct GTE {
    regs:           [u32; 32],
    control_regs:   [u32; 32]
}

impl GTE {
    pub fn new() -> Self {
        Self {
            regs:           [0; 32],
            control_regs:   [0; 32]
        }
    }
}

impl Coprocessor for GTE {
    fn load_from_mem(&mut self, reg: usize, data: u32) {
        self.regs[reg] = data;
    }

    fn store_to_mem(&mut self, reg: usize) -> u32 {
        self.regs[reg]
    }

    fn move_from_control(&mut self, reg: usize) -> u32 {
        self.control_regs[reg]
    }

    fn move_to_control(&mut self, reg: usize, data: u32) {
        self.control_regs[reg] = data;
    }

    fn move_from_reg(&mut self, reg: usize) -> u32 {
        self.regs[reg]
    }

    fn move_to_reg(&mut self, reg: usize, data: u32) {
        self.regs[reg] = data;
    }

    fn operation(&mut self, op: u32) {
        
    }
}

// Internal commands.
impl GTE {
    
}