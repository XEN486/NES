use bitflags::bitflags;

bitflags! {
    pub struct StatusRegister: u8 {
        const Unused1        = 0b00000001;
        const Unused2        = 0b00000010;
        const Unused3        = 0b00000100;
        const Unused4        = 0b00001000;
        const Unused5        = 0b00010000;
        const SpriteOverflow = 0b00100000;
        const SpriteZeroHit  = 0b01000000;
        const VBlankStarted  = 0b10000000;
    }
}

impl StatusRegister {
    pub fn new() -> Self {
        StatusRegister::from_bits_truncate(0b00000000)
    }

    pub fn set_vblank_status(&mut self, status: bool) {
        self.set(StatusRegister::VBlankStarted, status);
    }

    pub fn set_sprite_zero_hit(&mut self, status: bool) {
        self.set(StatusRegister::SpriteZeroHit, status);
    }

    // TODO: implement sprite overflow
    #[allow(dead_code)]
    pub fn set_sprite_overflow(&mut self, status: bool) {
        self.set(StatusRegister::SpriteOverflow, status);
    }

    pub fn reset_vblank_status(&mut self) {
        self.remove(StatusRegister::VBlankStarted);
    }

    pub fn is_in_vblank(&self) -> bool {
        self.contains(StatusRegister::VBlankStarted)
    }
}