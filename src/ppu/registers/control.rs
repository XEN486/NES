use bitflags::bitflags;

bitflags! {
    pub struct ControlRegister: u8 {
        const NameTable1                = 0b00000001;
        const NameTable2                = 0b00000010;
        const VRAMAddIncrement          = 0b00000100;
        const SpritePatternAddress      = 0b00001000;
        const BackgroundPatternAddress  = 0b00010000;
        const SpriteSize                = 0b00100000;
        const MasterSlaveSelect         = 0b01000000;
        const GenerateNMI               = 0b10000000;
    }
}

impl ControlRegister {
    pub fn new() -> ControlRegister {
        ControlRegister::from_bits_truncate(0b00000000)
    }

    pub fn vram_address_increment(&self) -> u8 {
        if !self.contains(ControlRegister::VRAMAddIncrement) {
            1
        } else {
            32
        }
    }

    pub fn sprt_pattern_address(&self) -> u16 {
        if !self.contains(ControlRegister::SpritePatternAddress) {
            0
        } else {
            0x1000
        }
    }

    pub fn bknd_pattern_address(&self) -> u16 {
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

    pub fn master_slave_select(&self) -> u8 {
        if !self.contains(ControlRegister::MasterSlaveSelect) {
            0
        } else {
            1
        }
    }

    pub fn generate_vblank_nmi(&self) -> bool {
        self.contains(ControlRegister::GenerateNMI)
    }

    pub fn update(&mut self, data: u8) {
        *self = ControlRegister::from_bits_truncate(data);
    }
}