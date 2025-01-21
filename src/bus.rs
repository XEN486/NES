use crate::apu::APU;
use crate::{cartridge::Rom, ppu::PPU};
use crate::joypad::Joypad;
use rand::Rng;


pub trait Mem {
    fn mem_read(&mut self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos.wrapping_add(1)) as u16;
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
    pub apu: APU,
    joypad1: Joypad,

    cycles: usize,
    gameloop: Box<dyn FnMut(&PPU, &mut APU, &mut Joypad, &mut u8, &mut u8) + 'call>,
    corruption: u8,
    ram_corruption: u8,
}

impl<'a> Bus<'a> {
    pub fn new<'call, F>(rom: Rom, gameloop: F) -> Bus<'call> where F: FnMut(&PPU, &mut APU, &mut Joypad, &mut u8, &mut u8) + 'call {
        let ppu = PPU::new(rom.chr_rom, rom.screen_mirroring);
        let apu = APU::new(rom.prg_rom.clone());

        Bus {
            cpu_vram: [0; 2048],
            prg_rom: rom.prg_rom.clone(),
            ppu: ppu,
            apu: apu,
            joypad1: Joypad::new(),

            cycles: 0,
            gameloop: Box::from(gameloop),
            corruption: 0,
            ram_corruption: 0,
        }
    }

    fn read_prg_rom(&self, mut addr: u16) -> u8 {
        addr -= 0x8000; // PRG rom starts from 0x8000 in ROM
        if self.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            addr %= 0x4000; // 0x4000 in size
        }

        self.prg_rom[addr as usize]
    }

    pub fn tick(&mut self, cycles: u8) {
        self.cycles += cycles as usize;

        self.apu.dmc.reset_stall_cycles(); // reset stall cycles on the DMC
        self.apu.tick(self.cycles); // 1 APU cycle = 1 CPU cycle

        let new_frame: bool = self.ppu.tick(cycles * 3); // 1 PPU cycle = 3 CPU cycles

        // if the PPU has finished a new frame, we should call the function to update the screen
        if new_frame {
            (self.gameloop)(&self.ppu, &mut self.apu, &mut self.joypad1, &mut self.corruption, &mut self.ram_corruption);
        }
    }

    pub fn get_apu_samples(&self) -> &Vec<i16> {
        &self.apu.buffer
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

            0x4015 => self.apu.read(),
            0x4016 => self.joypad1.read(),
            
            0x4017 => {
                //println!("[BUS] attempted to read from joypad 2");
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
        let mut corrupted = data;
        let mut ram_corrupted = data;

        if self.corruption != 0 {
            corrupted = corrupted.wrapping_add(rand::thread_rng().gen_range(0, self.corruption));
        }

        match addr {
            // RAM & RAM mirrors
            0x0000 ..= 0x1FFF => {
                if self.ram_corruption != 0 {
                    ram_corrupted = ram_corrupted.wrapping_pow(rand::thread_rng().gen_range(0, self.ram_corruption) as u32);
                }
                let mirror_down_addr = addr & 0b11111111111;
                self.cpu_vram[mirror_down_addr as usize] = ram_corrupted;
                ram_corrupted = data;
                self.ram_corruption = 0;
            }

            // PPU registers
            0x2000 => self.ppu.write_to_control(data),
            0x2001 => self.ppu.write_to_mask(corrupted),
            0x2003 => self.ppu.write_to_oam_address(data),
            0x2004 => self.ppu.write_to_oam_data(corrupted),
            0x2005 => self.ppu.write_to_scroll(data),
            0x2006 => self.ppu.write_to_ppu_address(corrupted),
            0x2007 => self.ppu.write_to_data(corrupted),

            0x4000 ..= 0x4003 => self.apu.write_to_pulse_0(addr, data),
            0x4004 ..= 0x4007 => self.apu.write_to_pulse_1(addr, data),
            0x4008 ..= 0x400B => self.apu.write_to_triangle(addr, data),
            0x400C ..= 0x400F => self.apu.write_to_noise(addr, data),
            0x4010 ..= 0x4013 => self.apu.write_to_dmc(addr, data),
            0x4015 => self.apu.set_status(data),
            0x4017 => self.apu.set_frame_counter(data, self.cycles),

            0x4016 => self.joypad1.write(data),

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
                self.mem_write(mirror_down_addr, corrupted);
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