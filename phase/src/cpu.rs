use mips::{coproc::{Coprocessor0, EmptyCoproc}, cpu::mips1::MIPSI};
use crate::mem::MemBus;
use crate::gte::GTE;

pub type CPU = MIPSI<MemBus, SystemCoproc, EmptyCoproc, GTE, EmptyCoproc>;

pub struct SystemCoproc {

}

impl Coprocessor0 for SystemCoproc {
    fn move_from_reg(&mut self, reg: usize) -> u32 {
        0
    }

    fn move_to_reg(&mut self, reg: usize, val: u32) {
        
    }

    fn operation(&mut self, op: u32) {
        
    }
}
