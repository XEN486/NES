use bitflags::bitflags;

bitflags! {
    pub struct MaskRegister : u8 {
        const Greyscale                 = 0b00000001;
        const Leftmost8PixelBackground  = 0b00000010;
        const Leftmost8PixelSprite      = 0b00000100;
        const ShowBackground            = 0b00001000;
        const ShowSprites               = 0b00010000;
        const EmphasiseRed              = 0b00100000;
        const EmphasiseGreen            = 0b01000000;
        const EmphasiseBlue             = 0b10000000;
    }
}

pub enum Color {
    Red,
    Green,
    Blue
}

impl MaskRegister {
    pub fn new() -> MaskRegister {
        MaskRegister::from_bits_truncate(0b00000000)
    }

    pub fn is_greyscale(&self) -> bool {
        self.contains(MaskRegister::Greyscale)
    }
    
    pub fn leftmost_8pixel_background(&self) -> bool {
        self.contains(MaskRegister::Leftmost8PixelBackground)
    }

    pub fn leftmost_8pixel_sprite(&self) -> bool {
        self.contains(MaskRegister::Leftmost8PixelSprite)
    }

    pub fn show_background(&self) -> bool {
        self.contains(MaskRegister::ShowBackground)
    }

    pub fn show_sprites(&self) -> bool {
        self.contains(MaskRegister::ShowSprites)
    }

    pub fn emphasize(&self) -> Vec<Color> {
        let mut result = Vec::<Color>::new();

        if self.contains(MaskRegister::EmphasiseRed) {
            result.push(Color::Red);
        }

        if self.contains(MaskRegister::EmphasiseGreen) {
            result.push(Color::Green);
        }

        if self.contains(MaskRegister::EmphasiseBlue) {
            result.push(Color::Blue);
        }

        result
    }

    pub fn update(&mut self, data: u8) {
        *self = MaskRegister::from_bits_truncate(data);
    }
}