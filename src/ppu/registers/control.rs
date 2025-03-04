use bitflags::bitflags;

bitflags! {
    pub struct ControlRegister: u8 {
        const Nametable1               = 0b00000001;
        const Nametable2               = 0b00000010;
        const VRAMAddressIncrement     = 0b00000100;
        const SpritePatternAddress     = 0b00001000;
        const BackgroundPatternAddress = 0b00010000;
        const SpriteSize               = 0b00100000;
        const MasterSlaveSelect        = 0b01000000;
        const GenerateNMI              = 0b10000000;
    }
}

impl ControlRegister {
    pub fn new() -> Self {
        ControlRegister::from_bits_truncate(0b0000_0000)
    }

    pub fn nametable_address(&self) -> u16 {
        match self.bits() & 0b11 {
            0 => 0x2000,
            1 => 0x2400,
            2 => 0x2800,
            3 => 0x2c00,
            _ => panic!("not possible"),
        }
    }

    pub fn vram_address_increment(&self) -> u8 {
        if !self.contains(ControlRegister::VRAMAddressIncrement) {
            1
        } else {
            32
        }
    }

    pub fn sprite_pattern_address(&self) -> u16 {
        if !self.contains(ControlRegister::SpritePatternAddress) {
            0
        } else {
            0x1000
        }
    }

    pub fn background_pattern_address(&self) -> u16 {
        if !self.contains(ControlRegister::BackgroundPatternAddress) {
            0
        } else {
            0x1000
        }
    }

    pub fn sprite_size(&self) -> u8 {
        if !self.contains(ControlRegister::SpriteSize) {
            8
        } else {
            16
        }
    }

    // UNUSED PPU FEATURE
    #[allow(dead_code)]
    pub fn master_slave_select(&self) -> u8 {
        if !self.contains(ControlRegister::SpriteSize) {
            0
        } else {
            1
        }
    }

    pub fn generate_vblank_nmi(&self) -> bool {
        return self.contains(ControlRegister::GenerateNMI);
    }

    pub fn update(&mut self, data: u8) {
        *self = ControlRegister::from_bits_truncate(data);
    }
}