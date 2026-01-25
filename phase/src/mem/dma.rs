use mips::mem::Data;

use crate::{
    interrupt::Interrupt,
    utils::{bits::*, interface::MemInterface}
};

/// Device that is capable of sending and/or receiving
/// data via DMA.
pub trait DMADevice {
    /// Read a word from the DMA port.
    fn dma_read_word(&mut self) -> Data<u32>;

    /// Write a word to the DMA port.
    /// 
    /// Returns a cycle count.
    fn dma_write_word(&mut self, data: u32) -> usize;
}

/// Direct memory access.
pub struct DMA {
    channels:           [DMAChannel; 7],
    control:            DMAControl,
    interrupt:          DMAInterrupt,
    irq_pending:        bool,
    table_generator:    OrderingTableGen,
}

/// Represents a single word transfer via DMA.
/// 
/// The external memory bus just needs to transfer between RAM and the device.
pub struct DMATransfer {
    pub addr:       u32,
    pub from_ram:   bool,
    pub device:     usize,
}

impl DMA {
    pub fn new() -> Self {
        Self {
            channels:           core::array::from_fn(|_| DMAChannel::new()),
            control:            DMAControl::empty(),
            interrupt:          DMAInterrupt::empty(),
            irq_pending:        false,
            table_generator:    OrderingTableGen::new(),
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

    pub fn mut_table_gen<'a>(&'a mut self) -> &'a mut OrderingTableGen {
        &mut self.table_generator
    }

    /// Get the DMA address for the channel provided.
    /// 
    /// If None, then no transfers are necessary.
    pub fn get_transfer(&mut self) -> Option<DMATransfer> {
        // Get active DMA channel.
        let mut current = None;
        let mut current_prio = 8;
        for n in (0..7).rev() {
            let channel = self.control.bits() >> (n * 4);
            let active = channel & 0x8 == 0x8;
            let prio = channel & 0x7;
            if active && prio < current_prio && self.channels[n].control.contains(ChannelControl::StartBusy) {
                current_prio = prio;
                current = Some(n);
            }
        }
        if let Some(chan_idx) = current {
            let channel = &mut self.channels[chan_idx];
            channel.control.remove(ChannelControl::StartTrigger);
            let transfer = Some(DMATransfer {
                addr:       channel.current_addr,
                from_ram:   channel.control.contains(ChannelControl::TransferDir),
                device:     chan_idx
            });
            if channel.control.contains(ChannelControl::DecAddr) {
                channel.current_addr -= 4;
            } else {
                channel.current_addr += 4;
            }
            if channel.dec_word_count() {
                if channel.finish_block() {
                    self.transfer_complete(chan_idx);
                }
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

    /// Set IRQ bit and trigger IRQ if necessary.
    fn transfer_complete(&mut self, channel: usize) {
        let mask_bit = 1 << (channel + 16);
        if self.interrupt.contains(DMAInterrupt::from_bits_truncate(mask_bit)) {
            let irq_bit = 1 << (channel + 24);
            self.interrupt.insert(DMAInterrupt::from_bits_truncate(irq_bit));
            if self.interrupt.contains(DMAInterrupt::EnableIRQ) {
                // Mark pending IRQ if request bit changes 0 => 1.
                if !self.interrupt.contains(DMAInterrupt::InterruptReq) {
                    self.interrupt.insert(DMAInterrupt::InterruptReq);
                    self.irq_pending = true;
                }
            }
        }
    }
}

impl MemInterface for DMA {
    fn read_word(&mut self, addr: u32) -> u32 {
        //println!("DMA read {:X}", addr);
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
        //println!("DMA write {:X} => {:X}", data, addr);
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
            0x1F8010E8 => {
                self.channels[6].set_control(data);
                if self.channels[6].control.contains(ChannelControl::StartTrigger) {
                    self.table_generator.init(self.channels[6].base_addr, self.channels[6].current_word_count);
                }
            },
            
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

    // Transfer state
    current_addr: u32,
    current_word_count: u32,
    current_block_count: u32,
}

impl DMAChannel {
    fn new() -> Self {
        Self {
            base_addr: 0,
            block_control: 0,
            control: ChannelControl::empty(),

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
        if self.control.contains(ChannelControl::StartTrigger) {
            self.start_sync_mode(0);
        }
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

    /// Finish a transfer block.
    /// Returns true if the entire transfer is complete.
    fn finish_block(&mut self) -> bool {
        if self.current_block_count == 0 {
            self.control.remove(ChannelControl::StartBusy);
            true
        } else {
            self.current_block_count -= 1;
            false
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
        const TransferDir       = bit!(0); // 1 = From RAM
    }
}

/// Reverse ordering table generator.
/// Used by DMA channel 6 to initialize in RAM.
pub struct OrderingTableGen {
    write_addr: u32,
    count:      u32,
}

impl OrderingTableGen {
    fn new() -> Self {
        Self {
            write_addr: 0,
            count:      0,
        }
    }

    fn init(&mut self, addr: u32, count: u32) {
        self.write_addr = addr;
        self.count = count;
    }
}

impl DMADevice for OrderingTableGen {
    fn dma_read_word(&mut self) -> Data<u32> {
        self.count -= 1;
        let data = if self.count == 0 {
            0x00FF_FFFF
        } else {
            self.write_addr -= 4;
            self.write_addr
        };
        Data { data, cycles: 1 }
    }

    fn dma_write_word(&mut self, _data: u32) -> usize {
        panic!("cannot write to DMA ordering table");
    }
}