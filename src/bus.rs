// bus.rs
use crate::cartridge::Rom;
use crate::apu::APU;
use crate::ppu::PPU;
use crate::joypad::Joypad;
use crate::mapper::{Mapper, Mapper0};
use rand::Rng;

use std::sync::Arc;
use std::sync::Mutex;

pub trait Mem {
    fn mem_read(&mut self, addr: u16) -> u8;
    fn mem_write(&mut self, addr: u16, data: u8);
    fn mem_read_u16(&mut self, pos: u16) -> u16;
    fn mem_write_u16(&mut self, pos: u16, data: u16);
}

pub struct Bus<'call> {
    cpu_vram: [u8; 2048],
    mapper: Box<dyn Mapper>,
    ppu: PPU,
    apu: APU,
    joypad1: Joypad,

    cycles: usize,
    gameloop: Box<dyn FnMut(&PPU, &mut APU, &mut Joypad, &mut u8) + 'call>,
    corruption: u8,
}

impl<'a> Bus<'a> {
    pub fn new<'call, F>(rom: Rom, gameloop: F) -> Bus<'call> 
    where F: FnMut(&PPU, &mut APU, &mut Joypad, &mut u8) + 'call {
        let ppu = PPU::new(rom.chr_rom.clone(), rom.mirroring);
        let apu = APU::new(rom.prg_rom.clone());

        let mapper: Box<dyn Mapper> = match rom.mapper {
            0 => Box::new(Mapper0::new(rom.prg_rom.clone())),
            _ => unimplemented!("[BUS] unsupported mapper {}", rom.mapper),
        };

        Bus {
            cpu_vram: [0; 2048],
            mapper,
            ppu,
            apu,
            joypad1: Joypad::new(),
            cycles: 0,
            gameloop: Box::from(gameloop),
            corruption: 0,
        }
    }

    fn read_prg_rom(&self, addr: u16) -> u8 {
        self.mapper.read_prg_rom(addr)
    }

    pub fn tick(&mut self, cycles: u8) {
        self.apu.dmc.reset_stall_cycles(); // reset stall cycles on the DMC
        for _ in 0..cycles {
            self.cycles += 1;
            self.apu.tick(self.cycles);
        }

        let new_frame: bool = self.ppu.tick(cycles * 3); // 1 PPU cycle = 3 CPU cycles

        if new_frame {
            (self.gameloop)(&self.ppu, &mut self.apu, &mut self.joypad1, &mut self.corruption);
        }
    }

    pub fn poll_nmi_status(&mut self) -> Option<u8> {
        self.ppu.nmi_interrupt.take()
    }

    pub fn get_apu_buffer(&self) -> Arc<Mutex<Vec<f32>>> {
        self.apu.buffer.clone()
    }
}

impl Mem for Bus<'_> {
    fn mem_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                let mirror_down_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirror_down_addr as usize]
            }
            0x2000 | 0x2001 | 0x2003 | 0x2005 | 0x2006 | 0x4014 => {
                println!("[BUS] attempted to read from write-only PPU registers");
                0
            }
            0x2002 => self.ppu.read_status(),
            0x2004 => self.ppu.read_oam_data(),
            0x2007 => self.ppu.read_data(),
            0x4015 => self.apu.read(),
            0x4016 => self.joypad1.read(),
            0x4017 => 0, // No support for joypad 2
            0x2008..=0x3FFF => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                self.mem_read(mirror_down_addr)
            }
            0x8000..=0xFFFF => self.read_prg_rom(addr),
            _ => {
                println!("[BUS] reading from unknown memory @ 0x{:04x}", addr);
                0
            }
        }
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        let mut corrupted = data;

        if self.corruption != 0 {
            corrupted = corrupted.wrapping_add(rand::rng().random_range(0..self.corruption));
        }

        match addr {
            0x0000..=0x1FFF => {
                let mirror_down_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirror_down_addr as usize] = data;
            }
            0x2000 => self.ppu.write_to_control(data),
            0x2001 => self.ppu.write_to_mask(corrupted),
            0x2002 => println!("[BUS] attempted to write to PPU status"),
            0x2003 => self.ppu.write_to_oam_address(data),
            0x2004 => self.ppu.write_to_oam_data(corrupted),
            0x2005 => self.ppu.write_to_scroll(data),
            0x2006 => self.ppu.write_to_ppu_address(corrupted),
            0x2007 => self.ppu.write_to_data(corrupted),
            0x4000..=0x4003 => self.apu.write_to_pulse_0(addr, corrupted),
            0x4004..=0x4007 => self.apu.write_to_pulse_1(addr, corrupted),
            0x4008..=0x400B => self.apu.write_to_triangle(addr, corrupted),
            0x400C..=0x400F => self.apu.write_to_noise(addr, corrupted),
            0x4010..=0x4013 => self.apu.write_to_dmc(addr, data),
            0x4015 => self.apu.set_status(corrupted),
            0x4017 => self.apu.set_frame_counter(corrupted, self.cycles),
            0x4016 => self.joypad1.write(data),
            0x4014 => {
                let mut buffer: [u8; 256] = [0; 256];
                let hi = (data as u16) << 8;

                for i in 0..256u16 {
                    buffer[i as usize] = self.mem_read(hi + i);
                }

                self.ppu.write_oam_dma(&buffer);
            }
            0x2008..=0x3FFF => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                self.mem_write(mirror_down_addr, corrupted);
            }
            0x8000..=0xFFFF => {
                self.mapper.write_prg_rom(addr, corrupted);
            }
            _ => {
                println!("[BUS] ignoring write to unknown memory @ 0x{:04x}", addr);
            }
        }
    }

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.mem_write(pos, lo);
        self.mem_write(pos + 1, hi);
    }
}