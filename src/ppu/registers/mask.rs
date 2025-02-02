use bitflags::bitflags;

bitflags! {
    pub struct MaskRegister: u8 {
        const Greyscale                = 0b00000001;
        const Leftmost8PixelBackground = 0b00000010;
        const Leftmost8PixelSprite     = 0b00000100;
        const ShowBackground           = 0b00001000;
        const ShowSprites              = 0b00010000;
        const EmphasizeRed             = 0b00100000;
        const EmphasizeGreen           = 0b01000000;
        const EmphasizeBlue            = 0b10000000;
    }
}

impl MaskRegister {
    pub fn new() -> Self {
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
    
    pub fn emphasis(&self) -> (bool, bool, bool) {
        (
            self.contains(MaskRegister::EmphasizeRed),
            self.contains(MaskRegister::EmphasizeGreen),
            self.contains(MaskRegister::EmphasizeBlue),
        )
    }

    pub fn update(&mut self, data: u8) {
        *self = MaskRegister::from_bits_truncate(data);
    }
}