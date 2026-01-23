use mips::coproc::Coprocessor0;
use crate::utils::bits::*;

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct ExceptionCause: u32 {
        const BranchDelay = bit!(31);
        const CoprocNumber = bits![28, 29];
        const InterruptPending = bits![8, 9, 10, 11, 12, 13, 14, 15];
        const ExCode = bits![2, 3, 4, 5, 6];

        const Writable = bits![8, 9]; // Software interrupt bits
        const HardwareInt = bits![10, 11, 12, 13, 14, 15];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct SystemStatus: u32 {
        const COP3Enable = bit!(31);
        const COP2Enable = bit!(30);
        const COP1Enable = bit!(29);
        const COP0Enable = bit!(28);
        const ReverseEndianness = bit!(25);
        const BootExcVectors = bit!(22);
        const TLBShutdown = bit!(21);
        const ParityError = bit!(20);
        const DataCacheHit = bit!(19);
        const ParityZero = bit!(18);
        const SwapCacheMode = bit!(17);
        const IsolateCache = bit!(16);
        const InterruptMask = bits![8, 9, 10, 11, 12, 13, 14, 15];
        const OldMode = bit!(5);
        const OldIntEnable = bit!(4);
        const PreviousMode = bit!(3);
        const PreviousIntEnable = bit!(2);
        const CurrentMode = bit!(1);
        const CurrentIntEnable = bit!(0);

        const IntStackBottom = bits![2, 3, 4, 5];
        const IntStackTop = bits![0, 1, 2, 3];
    }
}

pub struct SystemCoproc {
    system_status: SystemStatus,
    exception_cause: ExceptionCause,
    exception_ret_addr: u32,
    bad_virtual_addr: u32,

    // Breakpoints
    break_exec_addr: u32,
    break_exec_mask: u32,
    break_data_addr: u32,
    break_data_mask: u32,
    break_control: u32,
}

impl SystemCoproc {
    pub fn new() -> Self {
        let init_status = SystemStatus::BootExcVectors;
        Self {
            system_status: init_status,
            exception_cause: ExceptionCause::empty(),
            exception_ret_addr: 0,
            bad_virtual_addr: 0,

            break_exec_addr: 0,
            break_exec_mask: 0,
            break_data_addr: 0,
            break_data_mask: 0,
            break_control: 0,
        }
    }

    pub fn isolate_cache(&self) -> bool {
        self.system_status.contains(SystemStatus::IsolateCache)
    }
}

/// Processor ID.
const PRID: u32 = 0x0000_0001;

impl Coprocessor0 for SystemCoproc {
    fn move_from_reg(&mut self, reg: u8) -> u32 {
        match reg {
            3 => self.break_exec_addr,
            5 => self.break_data_addr,
            6 => 0, // jump dest
            7 => self.break_control,
            8 => self.bad_virtual_addr,
            9 => self.break_data_mask,
            11 => self.break_exec_mask,
            12 => self.system_status.bits(),
            13 => self.exception_cause.bits(),
            14 => self.exception_ret_addr,
            15 => PRID,
            _ => panic!("Reading from undefined coproc reg {:X}", reg), // Undefined
        }
    }

    fn move_to_reg(&mut self, reg: u8, data: u32) {
        match reg {
            3 => self.break_exec_addr = data,
            5 => self.break_data_addr = data,
            6 => {},
            7 => self.break_control = data,
            8 => {},
            9 => self.break_data_mask = data,
            11 => self.break_exec_mask = data,
            12 => self.set_status(data),
            13 => self.set_cause(data),
            14 => {},
            15 => {},
            _ => panic!("Writing to undefined coproc reg {:X}", reg), // Undefined
        }
    }

    fn operation(&mut self, op: u32) {
        match op & 0x1F {
            0x1 => self.reserved(), // TLBR
            0x2 => self.reserved(), // TLBWI
            0x6 => self.reserved(), // TLBWR
            0x8 => self.reserved(), // TLBP
            0x10 => self.rfe(),
            _ => {}, // Undefined
        }
    }

    fn reset(&mut self) -> u32 {
        // TODO: reset
        0xBFC0_0000
    }

    fn trigger_exception(&mut self, exception: &mips::coproc::Exception) -> u32 {
        use mips::cpu::ExceptionCode::*;
        if exception.code == AddrErrorLoad || exception.code == AddrErrorStore {
            self.bad_virtual_addr = exception.bad_virtual_addr;
        }
        self.exception_ret_addr = exception.ret_addr;
        self.exception_cause.remove(ExceptionCause::ExCode);
        let new_exception_cause = ExceptionCause::from_bits_truncate((exception.code as u32) << 2);
        self.exception_cause.insert(new_exception_cause);
        self.exception_cause.set(ExceptionCause::BranchDelay, exception.branch_delay);
        self.push_int_stack();
        if self.system_status.contains(SystemStatus::BootExcVectors) {
            0xBFC0_0180 // ROM
        } else {
            0x8000_0080 // RAM
        }
    }

    fn external_interrupt(&mut self, mask: u8) -> bool {
        let external_pending = (mask as u32) << 8;
        self.exception_cause.remove(ExceptionCause::HardwareInt);
        self.exception_cause.insert(ExceptionCause::from_bits_truncate(external_pending));
        self.check_interrupt()
    }
}

// Internal stuff
impl SystemCoproc {
    /// Return from Exception
    fn rfe(&mut self) {
        self.pop_int_stack();
    }

    /// Reserved.
    fn reserved(&mut self) {
        // TODO: trigger exception.
    }

    fn set_cause(&mut self, data: u32) {
        self.exception_cause.remove(ExceptionCause::Writable);
        let writable = ExceptionCause::from_bits_truncate(data) & ExceptionCause::Writable;
        self.exception_cause.insert(writable);
    }

    fn set_status(&mut self, data: u32) {
        self.system_status = SystemStatus::from_bits_truncate(data);
    }

    fn push_int_stack(&mut self) {
        let stack = self.system_status & SystemStatus::IntStackTop;
        // Write 0 to stack: i.e. switch to kernel mode + disable interrupts.
        self.system_status.remove(SystemStatus::IntStackBottom | SystemStatus::IntStackTop);
        self.system_status.insert(SystemStatus::from_bits_truncate(stack.bits() << 2));
    }

    fn pop_int_stack(&mut self) {
        let stack = self.system_status & SystemStatus::IntStackBottom;
        self.system_status.remove(SystemStatus::IntStackTop);
        self.system_status.insert(SystemStatus::from_bits_truncate(stack.bits() >> 2));
    }

    /// Check if an interrupt has triggered.
    fn check_interrupt(&self) -> bool {
        if self.system_status.contains(SystemStatus::CurrentIntEnable) {
            let mask = (self.system_status & SystemStatus::InterruptMask).bits();
            let irq = (self.exception_cause & ExceptionCause::InterruptPending).bits();
            mask & irq != 0
        } else {
            false
        }
    }
}
