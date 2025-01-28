use crate::cartridge::Mirroring;
use registers::{address::AddressRegister, control::ControlRegister, mask::MaskRegister, scroll::ScrollRegister, status::StatusRegister};

mod registers;

pub struct PPU {
    pub chr_rom: Vec<u8>,
    pub vram: [u8; 2048],
    pub mirroring: Mirroring,

    // registers
    pub address: AddressRegister,
    pub control: ControlRegister,
    pub status: StatusRegister,
    pub scroll: ScrollRegister,
    pub mask: MaskRegister,

    // oam
    pub oam_addr: u8,
    pub oam_data: [u8; 256],
    pub palette_table: [u8; 32],

    // nmi
    pub nmi_interrupt: Option<u8>,

    // internal
    internal_data_buffer: u8,
    scanline: u16,
    cycles: usize,
}

impl PPU {
    pub fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> PPU {
        PPU {
            chr_rom: chr_rom,
            vram: [0; 2048],
            mirroring: mirroring,

            address: AddressRegister::new(),
            control: ControlRegister::new(),
            status: StatusRegister::new(),
            scroll: ScrollRegister::new(),
            mask: MaskRegister::new(),

            oam_addr: 0,
            oam_data: [0; 256],
            palette_table: [0; 32],

            nmi_interrupt: None,

            internal_data_buffer: 0,
            scanline: 0,
            cycles: 0,
        }
    }

    pub fn write_to_mask(&mut self, value: u8) {
        self.mask.update(value);
    }

    pub fn write_to_ppu_address(&mut self, value: u8) {
        self.address.update(value);
    }

    pub fn write_to_control(&mut self, value: u8) {
        let before_nmi= self.control.generate_vblank_nmi();
        self.control.update(value);
        if !before_nmi && self.control.generate_vblank_nmi() && self.status.is_in_vblank() {
            self.nmi_interrupt = Some(1);
        }
    }

    fn increment_vram_addr(&mut self) {
        self.address.increment(self.control.vram_address_increment());
    }

    pub fn read_data(&mut self) -> u8 {
        let addr = self.address.get();
        self.increment_vram_addr();

        match addr {
            // CHR ROM
            0 ..= 0x1FFF => {
                let result = self.internal_data_buffer;
                self.internal_data_buffer = self.chr_rom[addr as usize];
                result
            }

            // VRAM
            0x2000 ..= 0x2fff => {
                let result = self.internal_data_buffer;
                self.internal_data_buffer = self.vram[self.mirror_vram_address(addr) as usize];
                result
            }

            // Reserved
            0x3000 ..= 0x3eff => {
                println!("[PPU] address space 0x3000 -> 0x3eff shouldn't be used. address requested: 0x{:04x}", addr);
                0
            }

            // Palette Table Mirrors
            0x3f10 | 0x3f14 | 0x3f18 | 0x3f1c => {
                let add_mirror = addr - 0x10;
                self.palette_table[(add_mirror - 0x3f00) as usize]
            }
            
            // Palette Table
            0x3f00 ..= 0x3fff => self.palette_table[(addr - 0x3f00) as usize % 32],

            // Unknown
            _ => panic!("[PPU] unexpected access to mirrored space {}", addr),
        }
    }

    pub fn mirror_vram_address(&self, addr: u16) -> u16 {
        let mirrored_vram = addr & 0b10111111111111; // mirror down
        let vram_index = mirrored_vram - 0x2000;
        let name_table = vram_index / 0x400;
        match (&self.mirroring, name_table) {
            (Mirroring::Vertical, 2) | (Mirroring::Vertical, 3) => vram_index - 0x800,
            (Mirroring::Horizontal, 2) => vram_index - 0x400,
            (Mirroring::Horizontal, 1) => vram_index - 0x400,
            (Mirroring::Horizontal, 3) => vram_index - 0x800,
            _ => vram_index,
        }
    }

    pub fn write_to_data(&mut self, value: u8) {
        let addr = self.address.get();
        match addr {
            0x0000 ..= 0x1fff => println!("[PPU] attempted to write to CHR ROM"),
            0x2000 ..= 0x2fff => self.vram[self.mirror_vram_address(addr) as usize] = value,
            0x3000 ..= 0x3eff => println!("[PPU] attempted to write to 0x{:04x}!", addr),

            0x3f10 | 0x3f14 | 0x3f18 | 0x3f1c => {
                let add_mirror = addr - 0x10;
                self.palette_table[(add_mirror - 0x3f00) as usize] = value;
            }

            0x3f00 ..= 0x3fff => self.palette_table[(addr - 0x3f00) as usize % 32] = value,
            _ => println!("[PPU] unexpected access to mirror space"),
        }

        self.increment_vram_addr();
    }

    pub fn read_status(&mut self) -> u8 {
        let data = self.status.bits();
        self.status.reset_vblank_status();
        self.address.reset_latch();
        self.scroll.reset_latch();
        data
    }

    pub fn write_to_scroll(&mut self, value: u8) {
        self.scroll.write(value);
    }

    pub fn write_to_oam_address(&mut self, value: u8) {
        self.oam_addr = value;
    }

    pub fn write_to_oam_data(&mut self, value: u8) {
        self.oam_data[self.oam_addr as usize] = value;
        self.oam_addr = self.oam_addr.wrapping_add(1);
    }

    pub fn read_oam_data(&self) -> u8 {
        self.oam_data[self.oam_addr as usize]
    }

    pub fn write_oam_dma(&mut self, data: &[u8; 256]) {
        for x in data.iter() {
            self.oam_data[self.oam_addr as usize] = *x;
            self.oam_addr = self.oam_addr.wrapping_add(1);
        }
    }

    pub fn tick(&mut self, cycles: u8) -> bool {
        self.cycles += cycles as usize;

        // wait until 341 cycles pass (341 PPU cycles = 1 scanline)
        if self.cycles >= 341 {
            if self.is_sprite_0_hit(self.cycles) {
                self.status.set_sprite_zero_hit(true);
            }

            self.cycles -= 341;
            self.scanline += 1;

            // set vblank status on scanline 241
            if self.scanline == 241 {
                self.status.set_vblank_status(true);
                self.status.set_sprite_zero_hit(false);

                if self.control.generate_vblank_nmi() {
                    self.nmi_interrupt = Some(1);
                }
            }

            // reset vblank status on scanline 262 (should this not be 261?)
            if self.scanline >= 262 {
                self.scanline = 0;
                self.nmi_interrupt = None;
                self.status.set_sprite_zero_hit(false);
                self.status.reset_vblank_status();
                return true;
            }
        }
        false
    }

    fn is_sprite_0_hit(&self, cycle: usize) -> bool {
        let y = self.oam_data[0] as usize;
        let x = self.oam_data[3] as usize;
        (y == self.scanline as usize) && x <= cycle && self.mask.show_sprites()
    }
}