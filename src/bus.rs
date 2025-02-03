use crate::cartridge::Rom;
use crate::apu::APU;
use crate::ppu::PPU;
use crate::joypad::Joypad;
use crate::Frame;

use crate::mappers::{mapper::Mapper, mapper0::Mapper0};

use std::rc::Rc;
use std::cell::RefCell;
use rand::Rng;

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
    frame: Frame,

    cycles: usize,
    gameloop: Box<dyn FnMut(&Frame, &PPU, &mut APU, &mut Joypad, &mut u8) + 'call>,
    corruption: u8,
}

#[allow(dead_code)]
impl<'a> Bus<'a> {
    pub fn new<'call, F>(rom: Rom, gameloop: F) -> Bus<'call>
    where
        F: FnMut(&Frame, &PPU, &mut APU, &mut Joypad, &mut u8) + 'call,
    {
        // wrap prg rom in rc/refcell to allow mutable sharing
        let prg_rom = Rc::new(RefCell::new(rom.prg_rom));
        // ppu uses chr rom; cloning is fine
        let ppu = PPU::new(rom.chr_rom.clone(), rom.mirroring);
        // pass shared prg rom to apu
        let apu = APU::new(prg_rom.clone());
        // create mapper with shared prg rom so writes mutate memory
        let mapper: Box<dyn Mapper> = match rom.mapper {
            0 => Box::new(Mapper0::new(
                prg_rom.clone(),
                rom.chr_rom.clone(),
                rom.mirroring,
            )),

            _ => unimplemented!("[BUS] unsupported mapper {}", rom.mapper),
        };

        Bus {
            cpu_vram: [0; 2048],
            mapper,
            ppu,
            apu,
            joypad1: Joypad::new(),
            frame: Frame::new(),
            cycles: 0,
            gameloop: Box::from(gameloop),
            corruption: 0,
        }
    }

    fn read_prg_rom(&self, addr: u16) -> u8 {
        self.mapper.read_prg_rom(addr)
    }

    fn write_prg_rom(&mut self, addr: u16, data: u8) {
        self.mapper.write_prg_rom(addr, data);
    }

    fn read_chr_rom(&self, addr: u16) -> u8 {
        self.mapper.read_chr_rom(addr)
    }

    fn write_chr_rom(&mut self, addr: u16, data: u8) {
        self.mapper.write_chr_rom(addr, data);
    }

    pub fn tick(&mut self, cycles: u8) {
        for _ in 0..cycles {
            self.cycles += 1;
            self.apu.tick(self.cycles);
        }

        let new_frame = self.ppu.tick(cycles * 3, &mut self.frame, self.mapper.mirroring());

        if new_frame {
            (self.gameloop)(&self.frame, &self.ppu, &mut self.apu, &mut self.joypad1, &mut self.corruption);
        }
    }

    pub fn poll_nmi_status(&mut self) -> Option<u8> {
        self.ppu.nmi_interrupt.take()
    }

    pub fn get_apu_buffer(&self) -> Vec<i16> {
        self.apu.buffer.clone()
    }

    pub fn set_apu_buffer(&mut self, buffer: Vec<i16>) {
        self.apu.buffer = buffer;
    }

    pub fn pop_apu_buffer(&mut self) -> Option<i16> {
        self.apu.buffer.pop()
    }
}

impl Mem for Bus<'_> {
    fn mem_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                // mirror down cpu vram
                let mirror_down_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirror_down_addr as usize]
            }
            0x2000 | 0x2001 | 0x2003 | 0x2005 | 0x2006 | 0x4014 => {
                println!("[BUS] read from write-only ppu regs");
                0
            }
            0x2002 => self.ppu.read_status(),
            0x2004 => self.ppu.read_oam_data(),
            0x2007 => self.ppu.read_data(),
            0x4015 => self.apu.read(),
            0x4016 => self.joypad1.read(),
            0x4017 => 0, // ignore joypad2
            0x2008..=0x3FFF => {
                // mirror down ppu regs
                let mirror_down_addr = addr & 0b00100000_00000111;
                self.mem_read(mirror_down_addr)
            }
            0x6000..=0x7FFF => self.mapper.read_prg_ram(addr - 0x6000),
            0x8000..=0xFFFF => self.read_prg_rom(addr),
            _ => 0,
        }
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        let mut corrupted = data;
        if self.corruption != 0 {
            corrupted = corrupted.wrapping_add(rand::rng().random_range(0..self.corruption));
        }
        match addr {
            0x0000..=0x1FFF => {
                // mirror down cpu vram
                let mirror_down_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirror_down_addr as usize] = data;
            }
            0x2000 => self.ppu.write_to_control(data),
            0x2001 => self.ppu.write_to_mask(corrupted),
            0x2002 => println!("[BUS] write to ppu status"),
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
                // oam dma
                let mut buffer: [u8; 256] = [0; 256];
                let hi = (data as u16) << 8;
                for i in 0..256u16 {
                    buffer[i as usize] = self.mem_read(hi + i);
                }
                self.ppu.write_oam_dma(&buffer);
            }
            0x2008..=0x3FFF => {
                // mirror down ppu regs
                let mirror_down_addr = addr & 0b00100000_00000111;
                self.mem_write(mirror_down_addr, corrupted);
            }
            0x6000..=0x7FFF => self.mapper.write_prg_ram(addr - 0x6000, data),
            0x8000..=0xFFFF => {
                // write to prg rom/ram via mapper
                self.write_prg_rom(addr, data);
            }
            _ => println!("[BUS] ignoring write to unknown addr 0x{:04x}", addr),
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
