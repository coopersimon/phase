mod software;

use std::sync::{
    Arc, Mutex,
    atomic::{Ordering, AtomicU32}
};

use crossbeam_channel::{
    Sender, Receiver
};

use crate::{
    Frame,
    mem::ram::RAM,
    utils::bits::*
};

use software::SoftwareRenderer;

const VRAM_SIZE: usize = 1024 * 1024;

pub enum RendererCmd {
    /// A command word sent via GP0.
    /// This could be a full or partial command,
    /// or data to be written to VRAM.
    GP0(u32),
    /// A new frame has begun, and we want to capture
    /// the frame buffer.
    GetFrame,

    // GP1 commands:
    AcknowledgeIRQ,
    DisplayEnable(bool),
    DataRequest(GPUStatus),
    DisplayVRAMOffset(u32),
    DisplayMode(GPUStatus),
    TexDisable(bool),
}

/// GPU renderer.
/// 
/// This lives on a different thread.
/// It receives GP0 commands and dispatches render calls.
/// 
/// It also manages VRAM.
pub struct Renderer {
    // Comms
    command_rx: Receiver<RendererCmd>,
    frame_tx: Sender<()>,
    atomic_status: Arc<AtomicU32>,

    // Internal state
    vram: RAM,
    status: GPUStatus,
    frame: Arc<Mutex<Frame>>,

    renderer: Box<dyn RendererImpl>,
}

impl Renderer {
    pub fn new(command_rx: Receiver<RendererCmd>, frame_tx: Sender<()>, status: Arc<AtomicU32>, frame: Arc<Mutex<Frame>>) -> Self {
        let init_status = GPUStatus::CommandReady | GPUStatus::DMARecvReady;
        status.store(init_status.bits(), Ordering::Release);
        let renderer = Box::new(SoftwareRenderer::new());
        Self {
            command_rx,
            frame_tx,
            atomic_status: status,

            vram: RAM::new(VRAM_SIZE),
            status: init_status,
            frame,

            renderer,
        }
    }

    /// Run in a separate thread.
    pub fn run(&mut self) {
        use RendererCmd::*;
        while let Ok(cmd) = self.command_rx.recv() {
            match cmd {
                GP0(data)                   => self.exec_gp0_command(data),
                GetFrame                    => self.send_frame(),
                AcknowledgeIRQ              => self.acknowledge_irq(),
                DisplayEnable(enable)       => self.display_enable(enable),
                DataRequest(data_req_stat)  => self.data_request(data_req_stat),
                DisplayVRAMOffset(offset)   => self.display_vram_offset(offset),
                DisplayMode(disp_mode_stat) => self.display_mode(disp_mode_stat),
                TexDisable(disable)         => self.tex_disable(disable),
            }
        }
    }

    fn send_frame(&mut self) {
        // TODO: assemble frame
        let _ = self.frame_tx.send(());
    }
}

// GP1.
impl Renderer {
    fn acknowledge_irq(&mut self) {
        self.status.remove(GPUStatus::IRQ);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
    }

    fn display_enable(&mut self, enable: bool) {
        self.status.set(GPUStatus::DisplayEnable, enable);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
    }

    fn data_request(&mut self, data_req_stat: GPUStatus) {
        self.status.remove(GPUStatus::DMAMode);
        self.status.insert(data_req_stat);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
    }

    fn display_vram_offset(&mut self, offset: u32) {
        let _x = offset & 0x3FF;
        let _y = (offset >> 10) & 0x1FF;
    }

    fn display_mode(&mut self, disp_mode_stat: GPUStatus) {
        self.status.remove(GPUStatus::DispModeFlags);
        self.status.insert(disp_mode_stat);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
    }

    fn tex_disable(&mut self, disable: bool) {
        self.status.set(GPUStatus::TexDisable, disable);
        self.atomic_status.store(self.status.bits(), Ordering::Release);
    }
}

// GP0.
impl Renderer {
    fn exec_gp0_command(&mut self, data: u32) {
        println!("GP0 command: {:X}", data);
        let command = (data >> 24) as u8;
        match command {
            
            0xE1 => self.draw_mode_setting(data),
            _ => {}, // Invalid command.
        }
    }

    fn draw_mode_setting(&mut self, param: u32) {
        let low_bits = param & 0x7FF;
        self.status.remove(GPUStatus::DrawModeFlags);
        self.status.insert(GPUStatus::from_bits_truncate(low_bits));
        self.status.set(GPUStatus::TexDisable, test_bit!(param, 11));
        self.atomic_status.store(self.status.bits(), Ordering::Release);

        // TODO: x-flip and y-flip
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    pub struct GPUStatus: u32 {
        const InterlaceOdd  = bit!(31);
        const DMAMode       = bits![29, 30];
        const DMARecvReady  = bit!(28);
        const VRAMSendReady = bit!(27);
        const CommandReady  = bit!(26);
        const DMARequest    = bit!(25);
        const IRQ           = bit!(24);
        const DisplayEnable = bit!(23);
        const Interlace     = bit!(22);
        const ColorDepth    = bit!(21);
        const PALMode       = bit!(20);
        const YResolution   = bit!(19);
        const XResolution   = bits![16, 17, 18];
        const TexDisable    = bit!(15);
        const Reverse       = bit!(14);
        const InterlaceField = bit!(13);
        const MaskDrawing   = bit!(12);
        const SetDrawMask   = bit!(11);
        const DrawDisplay   = bit!(10);
        const Dither        = bit!(9);
        const TexPageCol    = bits![7, 8];
        const SemiTrans     = bits![5, 6];
        const TexPageYBase  = bit!(4);
        const TexPageXBase  = bits![0, 1, 2, 3];

        const DispModeFlags = bits![14, 16, 17, 18, 19, 20, 21, 22];
        const DrawModeFlags = bits![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15];

        const CommandTransferReady = bits![28, 30];
    }
}

/// The code responsible for doing actual drawing
/// should implement this trait.
trait RendererImpl {
    
}