mod cpu;
mod mem;
mod gte;
mod interrupt;
mod timer;
mod cdrom;
mod expansion;
mod spu;
mod gpu;
mod peripheral;
mod utils;
mod io;

use std::path::PathBuf;

pub use crate::cpu::PSDebugger as PSDebugger;
use crate::peripheral::controller::ControllerState;

/// Config for PlayStation.
pub struct PlayStationConfig {
    pub bios_path:  PathBuf,
}

/// A PlayStation console.
pub struct PlayStation {
    cpu: Option<cpu::CPU>,
    io: io::IO,
    // Input state:
    input: Vec<io::InputMessage>,
    port_1_controller: Option<ControllerState>,
    port_2_controller: Option<ControllerState>,
}

impl PlayStation {
    pub fn new(config: PlayStationConfig) -> Self {
        let (io, bus_io) = io::IO::new();
        let cpu = cpu::CPU::new(&config, bus_io);
        Self {
            cpu: Some(cpu),
            io,
            input: Vec::new(),
            port_1_controller: None,
            port_2_controller: None,
        }
    }

    /// Start running the CPU on its own thread.
    pub fn run_cpu(&mut self) {
        let cpu = std::mem::take(&mut self.cpu).expect("CPU thread already running!");
        let _cpu_thread = std::thread::spawn(move || {
            cpu.run();
        });
    }

    /// Drives the emulator and returns a frame.
    /// 
    /// This should be called at 60fps for NTSC,
    /// and 50fps for PAL.
    pub fn frame(&mut self, frame: &mut Frame) {
        if let Some(state) = self.port_1_controller {
            self.input.push(io::InputMessage::ControllerInput { port: Port::One, state });
        }
        if let Some(state) = self.port_2_controller {
            self.input.push(io::InputMessage::ControllerInput { port: Port::Two, state });
        }
        let input = std::mem::replace(&mut self.input, Vec::new()).into_boxed_slice();
        self.io.get_frame(input, frame);
    }

    /// Make a debugger for stepping through instructions.
    /// 
    /// Warning: this will panic if the CPU thread has begun.
    pub fn make_debugger(self) -> PSDebugger {
        PSDebugger::new(self.cpu.expect("CPU thread running!"))
    }

    pub fn attach_controller(&mut self, controller: ControllerType, port: Port) {
        let state = ControllerState::new(controller);
        self.input.push(io::InputMessage::ControllerConnected { port, state });
        match port {
            Port::One => self.port_1_controller = Some(state),
            Port::Two => self.port_2_controller = Some(state),
        }
    }

    pub fn detach_controller(&mut self, port: Port) {
        self.input.push(io::InputMessage::ControllerDisconnected { port });
        match port {
            Port::One => self.port_1_controller = None,
            Port::Two => self.port_2_controller = None,
        }
    }

    pub fn press_button(&mut self, port: Port, button: Button, pressed: bool) {
        // TODO: more gracefully handle errors?
        let controller = match port {
            Port::One => self.port_1_controller.as_mut().expect("port 1 controller is missing!"),
            Port::Two => self.port_2_controller.as_mut().expect("port 2 controller is missing!"),
        };
        controller.press_button(button, pressed);
    }

    pub fn insert_cd(&mut self, path: PathBuf) {
        self.input.push(io::InputMessage::CDInserted { path });
    }

    pub fn remove_cd(&mut self) {
        self.input.push(io::InputMessage::CDRemoved);
    }
}

/// Information for frame.
pub struct Frame {
    pub frame_buffer: Vec<u8>,
    pub size: (usize, usize)
}

impl Frame {
    pub fn new() -> Self {
        Self {
            frame_buffer: Vec::new(),
            size: (0, 0),
        }
    }

    fn resize(&mut self, size: (usize, usize)) {
        if size.0 != self.size.0 || size.1 != self.size.1 {
            self.size = size;
            self.frame_buffer.resize(size.0 * size.1 * 4, 0);
            self.frame_buffer.fill(0);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ControllerType {
    Digital,
    // TODO: analog
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Port {
    One,
    Two,
}

#[derive(Clone, Copy)]
pub enum Button {
    Select,
    /// Only available on analog controllers
    L3,
    /// Only available on analog controllers
    R3,
    Start,
    DUp,
    DRight,
    DDown,
    DLeft,
    L2,
    R2,
    L1,
    R1,
    Triangle,
    Circle,
    Cross,
    Square,
}
