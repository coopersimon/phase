mod cpu;
mod mem;
mod gte;
mod utils;

/// A PlayStation console.
pub struct PlayStation {
    cpu: cpu::CPU
}

impl PlayStation {
    pub fn new() -> Self {
        let cpu = cpu::CPU::new();
        Self {
            cpu
        }
    }
}