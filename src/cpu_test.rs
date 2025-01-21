
use std::marker::PhantomData;

use bitflags::bitflags;
use crate::interrupt::{Interrupt, BRK};
use crate::bus::Mem;

bitflags! {
    #[derive(Debug)]
    pub struct StatusFlags: u8 {
        const Carry = 0b0000_0001;
        const Zero = 0b0000_0010;
        const InterruptDisable = 0b0000_0100;
        const DecimalUnused = 0b0000_1000;
        const Break = 0b0001_0000;
        const Break2 = 0b0010_0000;
        const Overflow = 0b0100_0000;
        const Negative = 0b1000_0000;
    }
}

#[derive(Debug)]
pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    Implied,
    Accumulator,
    Relative,
}
pub struct Registers {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8, // stack pointer
}

pub struct CPU<'a> {
    pub registers: Registers,
    pub status: u8,
    pub pc: u16,
    pub memory: [u8; 0x10000],
    marker: PhantomData<&'a ()>,
}

impl Mem for CPU<'_> {
    fn mem_read(&mut self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }
}

impl<'a> CPU<'a> {
    pub fn new() -> CPU<'a> {
        CPU {
            registers: Registers { a: 0, x: 0, y: 0, s: 0xFD },
            status: 0,
            pc: 0,
            memory: [0; 0x10000],
            marker: PhantomData
        }
    }

    fn stack_push(&mut self, data: u8) {
        self.mem_write(0x100 + self.registers.s as u16, data);
        self.registers.s = self.registers.s.wrapping_sub(1);
    }

    fn stack_pop(&mut self) -> u8 {
        self.registers.s = self.registers.s.wrapping_add(1);
        self.mem_read(0x100 + self.registers.s as u16)
    }

    fn stack_push_u16(&mut self, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.stack_push(hi);
        self.stack_push(lo);
    }

    fn stack_pop_u16(&mut self) -> u16 {
        let lo = self.stack_pop() as u16;
        let hi = self.stack_pop() as u16;
        (hi << 8) | lo
    }

    pub fn set_flag(&mut self, flag: StatusFlags) {
        self.status |= flag.bits();
    }

    pub fn clear_flag(&mut self, flag: StatusFlags) {
        self.status &= !flag.bits();
    }

    fn set_flag_else_clear(&mut self, flag: StatusFlags, expression: bool) {
        if expression {
            self.set_flag(flag);
        } else {
            self.clear_flag(flag);
        }
    }

    pub fn get_operand_address(&mut self, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Immediate => self.pc,
            AddressingMode::ZeroPage => self.mem_read(self.pc) as u16,

            AddressingMode::Absolute => self.mem_read_u16(self.pc),

            AddressingMode::ZeroPageX => {
                let pos = self.mem_read(self.pc);
                let addr = pos.wrapping_add(self.registers.x) as u16;
                addr
            }
            AddressingMode::ZeroPageY => {
                let pos = self.mem_read(self.pc);
                let addr = pos.wrapping_add(self.registers.y) as u16;
                addr
            }

            AddressingMode::AbsoluteX => {
                let base = self.mem_read_u16(self.pc);
                let addr = base.wrapping_add(self.registers.x as u16);
                addr
            }
            AddressingMode::AbsoluteY => {
                let base = self.mem_read_u16(self.pc);
                let addr = base.wrapping_add(self.registers.y as u16);
                addr
            }

            AddressingMode::IndirectX => {
                let base = self.mem_read(self.pc);

                let ptr: u8 = (base as u8).wrapping_add(self.registers.x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                (hi as u16) << 8 | (lo as u16)
            }
            AddressingMode::IndirectY => {
                let base = self.mem_read(self.pc);

                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.registers.y as u16);
                deref
            }

            AddressingMode::Relative => (self.pc as i32 + self.mem_read(self.pc) as i8 as i32) as u16,
            AddressingMode::Implied => self.pc,
            AddressingMode::Accumulator => self.pc,
            AddressingMode::Indirect => self.mem_read_u16(self.pc),
        }
    }

    fn pagecross_penalty(&mut self, mode: &AddressingMode) -> u8 {
        match mode {
            AddressingMode::AbsoluteX => {
                let base = self.get_operand_address(mode);
                let effective = base.wrapping_add(self.registers.x as u16);
                (base & 0xFF00 != effective & 0xFF00) as u8
            }
            AddressingMode::AbsoluteY => {
                let base = self.get_operand_address(mode);
                let effective = base.wrapping_add(self.registers.y as u16);
                (base & 0xFF00 != effective & 0xFF00) as u8
            }
            AddressingMode::Relative => {
                let offset = self.mem_read(self.pc) as i8 as i16; // Signed offset
                let new_pc = self.pc.wrapping_add(1).wrapping_add(offset as u16);
                (self.pc.wrapping_add(1) & 0xFF00 != new_pc & 0xFF00) as u8
            }
            AddressingMode::IndirectY => {
                let addr = self.get_operand_address(mode);
                let base = self.mem_read(addr);
                let lo = self.mem_read(base as u16) as u16;
                let hi = self.mem_read(base.wrapping_add(1) as u16) as u16;
                let deref_base = (hi << 8) | lo;
                let effective = deref_base.wrapping_add(self.registers.y as u16);
                (deref_base & 0xFF00 != effective & 0xFF00) as u8
            }
            _ => 0,
        }
    }

    fn interrupt(&mut self, interrupt: Interrupt) {
        self.stack_push_u16(self.pc);
        let mut flag = self.status.clone();

        if interrupt.flag_mask & 0b0001_0000 != 0 {
            flag |= StatusFlags::Break2.bits();
        }

        if interrupt.flag_mask & 0b0010_0000 != 0 {
            flag &= !StatusFlags::Break.bits();
        }

        self.stack_push(flag);
        self.set_flag(StatusFlags::InterruptDisable);

        //self.bus.tick(interrupt.cycles);
        self.pc = self.mem_read_u16(interrupt.vector_address);
    }
    
    pub fn step_with_callback<F>(&mut self, mut callback: F) -> u8 where F: FnMut(&mut CPU) {
        callback(self);

        //if let Some(_nmi) = self.bus.poll_nmi_status() {
        //    self.interrupt(NMI);
        //}

        let opcode = self.mem_read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        
        let mut cycles = self.get_cycles_for_opcode(opcode);
        if self.opmatch(opcode) {
            cycles += 1; // branch succeeded
        }
        
        //self.bus.tick(cycles);
        cycles
    }

    pub fn step(&mut self) -> u8 {
        self.step_with_callback(|_| {})
    }

    pub fn reset(&mut self) {
        self.registers.a = 0;
        self.registers.x = 0;
        self.registers.y = 0;
        self.registers.s = 0xFD;
        self.status = 0b0010_0100;
        self.pc = self.mem_read_u16(0xFFFC);
    }
}

include!("./opcodes.rs");
include!("./trace.rs");
include!("./cycles.rs");
include!("./opmatch.rs");

#[cfg(test)]
mod cpu_test {
    use super::*;
    use serde::{Deserialize};
    use serde_json;
    use std::fs::File;

    #[derive(Deserialize, Debug)]
    struct Data {
        name: String,
        initial: State,
        r#final: State,
        cycles: Vec<(u16, u8, String)>,
    }

    #[derive(Deserialize, Debug)]
    struct State {
        pc: u16,
        s: u8,
        a: u8,
        x: u8,
        y: u8,
        p: u8,
        ram: Vec<(u16, u8)>,
    }


    #[test]
    fn test_11() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/11.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_12() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/12.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_19() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/19.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_1b() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/1b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_1c() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/1c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_1d() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/1d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_22() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/22.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_23() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/23.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_27() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/27.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_2f() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/2f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_31() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/31.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_32() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/32.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_33() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/33.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_37() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/37.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_39() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/39.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_3b() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/3b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_3c() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/3c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_3d() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/3d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_3f() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/3f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_42() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/42.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_46() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/46.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_4b() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/4b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_4e() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/4e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_51() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/51.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_52() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/52.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_53() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/53.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_56() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/56.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_59() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/59.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_5c() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/5c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_5d() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/5d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_5e() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/5e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_62() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/62.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_63() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/63.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_66() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/66.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_67() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/67.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_68() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/68.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_6b() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/6b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_6e() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/6e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_6f() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/6f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_71() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/71.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_72() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/72.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_73() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/73.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_76() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/76.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_77() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/77.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_79() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/79.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_7b() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/7b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_7c() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/7c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_7d() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/7d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_7e() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/7e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_7f() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/7f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_83() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/83.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_87() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/87.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_8b() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/8b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_8f() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/8f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_92() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/92.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_93() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/93.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_97() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/97.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_9b() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/9b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_9c() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/9c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_9e() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/9e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_9f() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/9f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_a3() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/a3.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_a7() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/a7.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_ab() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/ab.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_af() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/af.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_b1() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/b1.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_b2() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/b2.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_b3() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/b3.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_b7() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/b7.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_b9() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/b9.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_bb() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/bb.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_bc() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/bc.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_bd() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/bd.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_be() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/be.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_bf() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/bf.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_c3() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/c3.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_c7() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/c7.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_cb() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/cb.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_cf() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/cf.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_d1() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/d1.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_d2() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/d2.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_d3() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/d3.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_d7() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/d7.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_d9() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/d9.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_db() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/db.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_dc() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/dc.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_dd() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/dd.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_df() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/df.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_e3() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/e3.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_eb() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/eb.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_f1() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/f1.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_f2() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/f2.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_f3() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/f3.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_f9() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/f9.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_fc() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/fc.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
    #[test]
    fn test_fd() {
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/fd.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
            //println!("test: {}", entry.name);
            for r in entry.initial.ram.iter() {
                cpu.memory[r.0 as usize] = r.1;
            }

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }

            //assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }
        }
    }

    
}