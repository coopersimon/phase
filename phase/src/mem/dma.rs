use crate::{interrupt::Interrupt, utils::{bits::*, interface::MemInterface}};


/// Direct memory access.
pub struct DMA {
    channels: [DMAChannel; 7],
    control: DMAControl,
    interrupt: DMAInterrupt,
    irq_pending: bool,
}

/// Represents a single word transfer via DMA.
/// 
/// The external memory bus just needs to transfer from src to dst.
pub struct DMATransfer {
    pub src_addr: u32,
    pub dst_addr: u32,
}

impl DMA {
    pub fn new() -> Self {
        Self {
            channels: [
                DMAChannel::new(0x1F80_1820), // MDEC in
                DMAChannel::new(0x1F80_1820), // MDEC out
                DMAChannel::new(0x1F80_1810), // GPU
                DMAChannel::new(0x1F80_1804), // CD
                DMAChannel::new(0x1F80_1DA8), // SPU
                DMAChannel::new(0), // PIO
                DMAChannel::new(0)  // OTC
            ],
            control: DMAControl::empty(),
            interrupt: DMAInterrupt::empty(),
            irq_pending: false,
        }
    }

    /// Check if an IRQ is pending.
    /// 
    /// This will reset the pending value to false.
    pub fn check_irq(&mut self) -> Interrupt {
        if std::mem::replace(&mut self.irq_pending, false) {
            Interrupt::DMA
        } else {
            Interrupt::empty()
        }
    }

    pub fn mdec_req(&mut self) {
        self.channels[0].start_sync_mode(1);
        self.channels[1].start_sync_mode(1);
    }

    pub fn spu_req(&mut self) {
        self.channels[4].start_sync_mode(1);
    }

    pub fn gpu_data_req(&mut self) {
        self.channels[2].start_sync_mode(1);
    }

    pub fn gpu_command_req(&mut self) {
        self.channels[2].start_sync_mode(2);
    }

    /// Get the DMA address for the channel provided.
    /// 
    /// If None, then no transfers are necessary.
    pub fn get_transfer(&mut self) -> Option<DMATransfer> {
        // Get active DMA channel.
        let mut current = None;
        let mut current_prio = 8;
        for n in 0..7 {
            let channel = self.control.bits() >> (n * 4);
            let active = channel & 0x8 == 0x8;
            let prio = channel & 0x7;
            if active && prio < current_prio && self.channels[n].control.contains(ChannelControl::StartBusy) {
                current_prio = prio;
                current = Some(n);
            }
        }
        if let Some(chan) = current {
            let channel = &mut self.channels[chan];
            channel.control.remove(ChannelControl::StartTrigger);
            let transfer = Some(if channel.control.contains(ChannelControl::TransferDir) {
                // From RAM
                DMATransfer {
                    src_addr: channel.current_addr,
                    dst_addr: channel.device_addr,
                }
            } else {
                // To RAM
                DMATransfer {
                    src_addr: channel.device_addr,
                    dst_addr: channel.current_addr,
                }
            });
            if channel.control.contains(ChannelControl::DecAddr) {
                channel.current_addr -= 4;
            } else {
                channel.current_addr += 4;
            }
            if channel.dec_word_count() {
                channel.finish_block();
            }
            transfer
        } else {
            None
        }
    }

    fn set_control(&mut self, data: u32) {
        self.control = DMAControl::from_bits_truncate(data);
    }

    fn set_interrupt(&mut self, data: u32) {
        let input = DMAInterrupt::from_bits_truncate(data);
        let irq = self.interrupt.contains(DMAInterrupt::InterruptReq);
        let flags = self.interrupt & DMAInterrupt::IRQFlags;
        self.interrupt = input & DMAInterrupt::Writable;
        self.interrupt.insert(flags);
        self.interrupt.remove(input & DMAInterrupt::IRQFlags); // Acknowledge IRQs.
        if self.interrupt.contains(DMAInterrupt::ForceIRQ) ||
            (self.interrupt.contains(DMAInterrupt::EnableIRQ) && self.interrupt.intersects(DMAInterrupt::IRQFlags)) {
            self.interrupt.insert(DMAInterrupt::InterruptReq);
            if !irq { // Mark pending IRQ if request bit changes 0 => 1.
                self.irq_pending = true;
            }
        }
    }
}

impl MemInterface for DMA {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x1F801080 => self.channels[0].base_addr,
            0x1F801084 => self.channels[0].block_control,
            0x1F801088 => self.channels[0].control.bits(),
            
            0x1F801090 => self.channels[1].base_addr,
            0x1F801094 => self.channels[1].block_control,
            0x1F801098 => self.channels[1].control.bits(),
            
            0x1F8010A0 => self.channels[2].base_addr,
            0x1F8010A4 => self.channels[2].block_control,
            0x1F8010A8 => self.channels[2].control.bits(),
            
            0x1F8010B0 => self.channels[3].base_addr,
            0x1F8010B4 => self.channels[3].block_control,
            0x1F8010B8 => self.channels[3].control.bits(),
            
            0x1F8010C0 => self.channels[4].base_addr,
            0x1F8010C4 => self.channels[4].block_control,
            0x1F8010C8 => self.channels[4].control.bits(),
            
            0x1F8010D0 => self.channels[5].base_addr,
            0x1F8010D4 => self.channels[5].block_control,
            0x1F8010D8 => self.channels[5].control.bits(),
            
            0x1F8010E0 => self.channels[6].base_addr,
            0x1F8010E4 => self.channels[6].block_control,
            0x1F8010E8 => self.channels[6].control.bits(),
            
            0x1F8010F0 => self.control.bits(),
            0x1F8010F4 => self.interrupt.bits(),

            _ => panic!("invalid DMA address {:X}", addr),
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x1F801080 => self.channels[0].set_addr(data),
            0x1F801084 => self.channels[0].block_control = data,
            0x1F801088 => self.channels[0].set_control(data),
            
            0x1F801090 => self.channels[1].set_addr(data),
            0x1F801094 => self.channels[1].block_control = data,
            0x1F801098 => self.channels[1].set_control(data),
            
            0x1F8010A0 => self.channels[2].set_addr(data),
            0x1F8010A4 => self.channels[2].block_control = data,
            0x1F8010A8 => self.channels[2].set_control(data),
            
            0x1F8010B0 => self.channels[3].set_addr(data),
            0x1F8010B4 => self.channels[3].block_control = data,
            0x1F8010B8 => self.channels[3].set_control(data),
            
            0x1F8010C0 => self.channels[4].set_addr(data),
            0x1F8010C4 => self.channels[4].block_control = data,
            0x1F8010C8 => self.channels[4].set_control(data),
            
            0x1F8010D0 => self.channels[5].set_addr(data),
            0x1F8010D4 => self.channels[5].block_control = data,
            0x1F8010D8 => self.channels[5].set_control(data),
            
            0x1F8010E0 => self.channels[6].set_addr(data),
            0x1F8010E4 => self.channels[6].block_control = data,
            0x1F8010E8 => self.channels[6].set_control(data),
            
            0x1F8010F0 => self.set_control(data),
            0x1F8010F4 => self.set_interrupt(data),

            _ => panic!("invalid DMA address {:X}", addr),
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct DMAControl: u32 {
        const DMA6Enable    = bit!(27);
        const DMA6Priority  = bits![24, 25, 26];
        const DMA5Enable    = bit!(23);
        const DMA5Priority  = bits![20, 21, 22];
        const DMA4Enable    = bit!(19);
        const DMA4Priority  = bits![16, 17, 18];
        const DMA3Enable    = bit!(15);
        const DMA3Priority  = bits![12, 13, 14];
        const DMA2Enable    = bit!(11);
        const DMA2Priority  = bits![8, 9, 10];
        const DMA1Enable    = bit!(7);
        const DMA1Priority  = bits![4, 5, 6];
        const DMA0Enable    = bit!(3);
        const DMA0Priority  = bits![0, 1, 2];
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct DMAInterrupt: u32 {
        const InterruptReq = bit!(31);
        const IRQFlag6  = bit!(30);
        const IRQFlag5  = bit!(29);
        const IRQFlag4  = bit!(28);
        const IRQFlag3  = bit!(27);
        const IRQFlag2  = bit!(26);
        const IRQFlag1  = bit!(25);
        const IRQFlag0  = bit!(24);
        const EnableIRQ = bit!(23);
        const IRQMask6  = bit!(22);
        const IRQMask5  = bit!(21);
        const IRQMask4  = bit!(20);
        const IRQMask3  = bit!(19);
        const IRQMask2  = bit!(18);
        const IRQMask1  = bit!(17);
        const IRQMask0  = bit!(16);
        const ForceIRQ  = bit!(15);

        const IRQFlags = bits![24, 25, 26, 27, 28, 29, 30];
        const Writable = bits![15, 16, 17, 18, 19, 20, 21, 22, 23];
    }
}

/// A single channel for DMA.
/// There are 7 in total.
struct DMAChannel {
    // Registers
    base_addr: u32,
    block_control: u32,
    control: ChannelControl,

    // Fixed address
    device_addr: u32,

    // Transfer state
    current_addr: u32,
    current_word_count: u32,
    current_block_count: u32,
}

impl DMAChannel {
    fn new(device_addr: u32) -> Self {
        Self {
            base_addr: 0,
            block_control: 0,
            control: ChannelControl::empty(),

            device_addr,

            current_addr: 0,
            current_word_count: 0,
            current_block_count: 0,
        }
    }

    fn set_addr(&mut self, data: u32) {
        self.base_addr = data & 0x00FF_FFFF;
    }

    fn set_control(&mut self, data: u32) {
        self.control = ChannelControl::from_bits_truncate(data);
        self.start_sync_mode(0);
    }

    fn start_sync_mode(&mut self, mode: u32) {
        let current_mode = (self.control & ChannelControl::SyncMode).bits() >> 9;
        if mode == current_mode {
            self.control.insert(ChannelControl::StartBusy);
            self.current_addr = self.base_addr;
            self.current_word_count = self.block_control & 0xFFFF;
            self.current_block_count = (self.block_control >> 16) & 0xFFFF;
        }
    }

    /// Decrement the block by 1.
    /// Returns true if the block is complete.
    fn dec_word_count(&mut self) -> bool {
        self.current_word_count = self.current_word_count.wrapping_sub(1);
        self.current_word_count == 0
    }

    fn finish_block(&mut self) {
        if self.current_block_count == 0 {
            self.control.remove(ChannelControl::StartBusy);
        } else {
            self.current_block_count -= 1;
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct ChannelControl: u32 {
        const StartTrigger      = bit!(28);
        const StartBusy         = bit!(24);
        const ChopCPUWindowSize = bits![20, 21, 22];
        const ChopDMAWindowSize = bits![16, 17, 18];
        const SyncMode          = bits![9, 10];
        const ChopEnable        = bit!(8);
        const DecAddr           = bit!(1);
        const TransferDir       = bit!(0);
    }
}