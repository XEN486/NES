use crate::cartridge::Mirroring;

pub trait Mapper {
    fn read_prg_rom(&self, addr: u16) -> u8;
    fn write_prg_rom(&mut self, addr: u16, data: u8);

    fn read_chr_rom(&self, addr: u16) -> u8;
    fn write_chr_rom(&mut self, addr: u16, data: u8);
    
    fn read_prg_ram(&self, addr: u16) -> u8;
    fn write_prg_ram(&mut self, addr: u16, data: u8);

    fn mirroring(&self) -> Mirroring;
}