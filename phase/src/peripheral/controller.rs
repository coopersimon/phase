use crate::{
    utils::bits::*,
    ControllerType, Button
};

const DIGITAL_INFO: u16 = 0x5A41;

#[derive(Clone, Copy)]
pub struct ControllerState {
    info:        u16,
    buttons:     ControllerButtons,
    right_stick: Option<StickAxis>,
    left_stick:  Option<StickAxis>,
}

impl ControllerState {
    pub fn new(controller: ControllerType) -> Self {
        match controller {
            ControllerType::Digital => Self {
                info: DIGITAL_INFO,
                buttons: ControllerButtons::all(),
                left_stick: None,
                right_stick: None,
            }
        }
    }

    pub fn press_button(&mut self, button: Button, pressed: bool) {
        let bit = bit!(button as usize);
        self.buttons.set(ControllerButtons::from_bits_retain(bit), !pressed);
    }

    pub fn get_binary(&self, data: &mut [u16; 4]) {
        data[0] = self.info;
        data[1] = self.buttons.bits();
        if let Some(left) = self.left_stick {
            data[2] = left.get_binary();
        } else {
            data[2] = 0x0000;
        }
        if let Some(right) = self.right_stick {
            data[3] = right.get_binary();
        } else {
            data[3] = 0x0000;
        }
    }
}

#[derive(Clone, Copy)]
pub struct StickAxis {
    x: u8,
    y: u8,
}

impl StickAxis {
    pub fn get_binary(&self) -> u16 {
        (self.x as u16) | ((self.y as u16) << 8)
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    pub struct ControllerButtons: u16 {
        const Square    = bit!(15);
        const Cross     = bit!(14);
        const Circle    = bit!(13);
        const Triangle  = bit!(12);
        const R1        = bit!(11);
        const L1        = bit!(10);
        const R2        = bit!(9);
        const L2        = bit!(8);
        const DLeft     = bit!(7);
        const DDown     = bit!(6);
        const DRight    = bit!(5);
        const DUp       = bit!(4);
        const Start     = bit!(3);
        const R3        = bit!(2);
        const L3        = bit!(1);
        const Select    = bit!(0);
    }
}
