pub trait Mapper {
    fn read_prg_rom(&self, addr: u16) -> u8;
    fn write_prg_rom(&mut self, addr: u16, data: u8);
}

pub struct Mapper0 {
    prg_rom: Vec<u8>,
}

impl Mapper0 {
    pub fn new(prg_rom: Vec<u8>) -> Self {
        Mapper0 { prg_rom }
    }
}

impl Mapper for Mapper0 {
    fn read_prg_rom(&self, addr: u16) -> u8 {
        let addr = addr - 0x8000;
        if self.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            self.prg_rom[addr as usize % 0x4000]
        } else {
            self.prg_rom[addr as usize]
        }
    }

    fn write_prg_rom(&mut self, addr: u16, _data: u8) {
        println!("[MAPPER 0] Write to PRG ROM attempted at address 0x{:04x}", addr);
    }
}