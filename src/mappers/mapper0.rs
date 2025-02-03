use crate::mappers::mapper::Mapper;
use crate::cartridge::Mirroring;

use std::cell::RefCell;
use std::rc::Rc;

pub struct Mapper0 {
    prg_rom: Rc<RefCell<Vec<u8>>>,
    chr_rom: Vec<u8>,
    mirroring: Mirroring,
}

impl Mapper0 {
    pub fn new(prg_rom: Rc<RefCell<Vec<u8>>>, chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
        Mapper0 {
            prg_rom,
            chr_rom,
            mirroring,
        }
    }
}

impl Mapper for Mapper0 {
    fn read_prg_rom(&self, addr: u16) -> u8 {
        let addr = addr - 0x8000;
        if self.prg_rom.borrow().len() == 0x4000 && addr >= 0x4000 {
            self.prg_rom.borrow()[addr as usize % 0x4000]
        } else {
            self.prg_rom.borrow()[addr as usize]
        }
    }

    fn write_prg_rom(&mut self, addr: u16, _data: u8) {
        println!("[MAPPER 0] write to PRG ROM attempted at address 0x{:04x}", addr);
    }

    fn read_chr_rom(&self, addr: u16) -> u8 {
        self.chr_rom[addr as usize % self.chr_rom.len()]
    }

    fn write_chr_rom(&mut self, addr: u16, _data: u8) {
        println!("[MAPPER 0] write to CHR ROM attempted at address 0x{:04x}", addr);
    }

    fn read_prg_ram(&self, addr: u16) -> u8 {
        println!("[MAPPER 0] read from PRG RAM attempted at address 0x{:04x}", addr);
        0
    }

    fn write_prg_ram(&mut self, addr: u16, _data: u8) {
        println!("[MAPPER 0] write to PRG RAM attempted at address 0x{:04x}", addr);
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring.clone()
    }
}