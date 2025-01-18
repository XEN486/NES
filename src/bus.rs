use crate::{cartridge::Rom, ppu::PPU};
use crate::joypad::Joypad;

pub trait Mem {
    fn mem_read(&mut self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
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

pub struct Bus<'call> {
    cpu_vram: [u8; 2048],
    prg_rom: Vec<u8>,
    ppu: PPU,
    joypad1: Joypad,

    cycles: usize,
    gameloop: Box<dyn FnMut(&PPU, &mut Joypad) + 'call>,
}

impl<'a> Bus<'a> {
    pub fn new<'call, F>(rom: Rom, gameloop: F) -> Bus<'call> where F: FnMut(&PPU, &mut Joypad) + 'call {
        let ppu = PPU::new(rom.chr_rom, rom.screen_mirroring);

        Bus {
            cpu_vram: [0; 2048],
            prg_rom: rom.prg_rom,
            ppu: ppu,
            joypad1: Joypad::new(),

            cycles: 0,
            gameloop: Box::from(gameloop),
        }
    }

    fn read_prg_rom(&self, mut addr: u16) -> u8 {
        addr -= 0x8000;
        if self.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            addr %= 0x4000;
        }

        self.prg_rom[addr as usize]
    }

    pub fn tick(&mut self, cycles: u8) {
        self.cycles += cycles as usize;

        let new_frame: bool = self.ppu.tick(cycles * 3);
        if new_frame {
            (self.gameloop)(&self.ppu, &mut self.joypad1);
        }
    }

    pub fn poll_nmi_status(&mut self) -> Option<u8> {
        self.ppu.nmi_interrupt.take()
    }
}

impl Mem for Bus<'_> {
    fn mem_read(&mut self, addr: u16) -> u8 {
        match addr {
            // RAM & RAM mirrors
            0x0000 ..= 0x1FFF => {
                let mirror_down_addr: u16 = addr & 0b00000111_11111111; // zero out first 5 bits to mirror back
                self.cpu_vram[mirror_down_addr as usize]
            }

            // PPU registers & PPU mirrors
            0x2000 | 0x2001 | 0x2003 | 0x2005 | 0x2006 | 0x4014 => {
                println!("[BUS] attempted to read from write-only PPU registers");
                0
            }

            0x2002 => self.ppu.read_status(),
            0x2004 => self.ppu.read_oam_data(),
            0x2007 => self.ppu.read_data(),

            0x4000 ..= 0x4015 => {
                println!("[BUS] attempted to read from APU address");
                0
            }

            0x4016 => self.joypad1.read(),
            
            0x4017 => {
                println!("[BUS] attempted to read from joypad 2");
                0
            }

            0x2008 ..= 0x3FFF => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                self.mem_read(mirror_down_addr)
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

            // PPU registers
            0x2000 => self.ppu.write_to_control(data),
            0x2001 => self.ppu.write_to_mask(data),
            0x2003 => self.ppu.write_to_oam_address(data),
            0x2004 => self.ppu.write_to_oam_data(data),
            0x2005 => self.ppu.write_to_scroll(data),
            0x2006 => self.ppu.write_to_ppu_address(data),
            0x2007 => self.ppu.write_to_data(data),

            0x4000 ..= 0x4013 | 0x4015 => {
                println!("[BUS] attempted to write to APU address");
            }

            0x4016 => self.joypad1.write(data),
            
            0x4017 => {
                println!("[BUS] attempted to write to joypad 2");
            }

            0x4014 => {
                let mut buffer: [u8; 256] = [0; 256];
                let hi = (data as u16) << 8;
                
                for i in 0..256u16 {
                    buffer[i as usize] = self.mem_read(hi + i);
                }

                self.ppu.write_oam_dma(&buffer);
            }

            // PPU mirrors
            0x2008 ..= 0x3FFF => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                self.mem_write(mirror_down_addr, data);
            }

            // Cartridge ROM
            0x8000 ..= 0xFFFF => println!("[BUS] program attempted to write to cartridge rom @ 0x{:04x}", addr),

            // Unknown
            _ => {
                println!("[BUS] attempting to write to unknown memory @ 0x{:04x}", addr);
            }
        }
    }
}