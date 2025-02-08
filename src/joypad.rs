use bitflags::bitflags;

bitflags! {
    #[derive(PartialEq, Clone)]
    pub struct JoypadButton: u8 {
        const Right             = 0b10000000;
        const Left              = 0b01000000;
        const Down              = 0b00100000;
        const Up                = 0b00010000;
        const Start             = 0b00001000;
        const Select            = 0b00000100;
        const B                 = 0b00000010;
        const A                 = 0b00000001;
    }
}

pub struct Joypad {
    strobe: bool,
    index: u8,
    status: JoypadButton,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad {
            strobe: false,
            index: 0,
            status: JoypadButton::from_bits_truncate(0),
        }
    }

    pub fn write(&mut self, data: u8) {
        self.strobe = data & 1 == 1;
        if self.strobe {
            self.index = 0
        }
    }

    pub fn read(&mut self) -> u8 {
        if self.index > 7 {
            return 1;
        }
        let response = (self.status.bits() & (1 << self.index)) >> self.index;
        if !self.strobe && self.index <= 7 {
            self.index += 1;
        }
        response
    }

    pub fn set_button_status(&mut self, button: JoypadButton, pressed: bool) {
        self.status.set(button, pressed);
    }
}