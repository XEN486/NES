use crate::cartridge::Rom;

pub trait Mem {
    fn mem_read(&self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&self, pos: u16) -> u16 {
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos + 1) as u16;
        (hi << 8) | (lo as u16)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.mem_write(pos, lo);
        self.mem_write(pos + 1, hi);
    }
}

pub struct Bus {
    cpu_vram: [u8; 2048],
    rom: Rom,
}

impl Bus {
    pub fn new(rom: Rom) -> Bus {
        Bus {
            cpu_vram: [0; 2048],
            rom: rom,
        }
    }

    fn read_prg_rom(&self, mut addr: u16) -> u8 {
        addr -= 0x8000;
        if self.rom.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            addr %= 0x4000;
        }

        self.rom.prg_rom[addr as usize]
    }
}

impl Mem for Bus {
    fn mem_read(&self, addr: u16) -> u8 {
        match addr {
            // RAM & RAM mirrors
            0x0000 ..= 0x1FFF => {
                let mirror_down_addr: u16 = addr & 0b00000111_11111111; // zero out first 5 bits to mirror back
                self.cpu_vram[mirror_down_addr as usize]
            }

            // PPU registers & PPU mirrors
            0x2000 ..= 0x3FFF => {
                let _mirror_down_addr = addr & 0b00100000_00000111;
                todo!("[PPU] ppu not supported yet");
            }

            // Cartridge space
            0x8000 ..= 0xFFFF => self.read_prg_rom(addr),

            // Unknown
            _ => {
                println!("[BUS] reading from unknown memory @ 0x{:04x}", addr);
                0
            }
        }
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        match addr {
            // RAM & RAM mirrors
            0x0000 ..= 0x1FFF => {
                let mirror_down_addr = addr & 0b11111111111;
                self.cpu_vram[mirror_down_addr as usize] = data;
            }

            // PPU registers & PPU mirrors
            0x2000 ..= 0x3FFF => {
                let _mirror_down_addr = addr & 0b00100000_00000111;
                todo!("[PPU] ppu not supported yet");
            }

            // Cartridge space
            0x8000 ..= 0xFFFF => println!("[BUS] program attempted to write to cartridge rom @ 0x{:04x}", addr),

            // Unknown
            _ => {
                println!("[BUS] writing to unknown memory @ 0x{:04x}", addr);
            }
        }
    }
}