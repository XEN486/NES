
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

            AddressingMode::Indirect => {
                let mem_address = self.mem_read_u16(self.pc);

                if mem_address & 0x00FF == 0x00FF {
                    let lo = self.mem_read(mem_address);
                    let hi = self.mem_read(mem_address & 0xFF00);
                    return (hi as u16) << 8 | (lo as u16)
                }
                self.mem_read_u16(mem_address)
            }

            AddressingMode::Immediate => self.pc,
            AddressingMode::Relative => (self.pc as i32 + self.mem_read(self.pc) as i8 as i32) as u16,
            AddressingMode::Implied => self.pc,
            AddressingMode::Accumulator => self.pc,
        }
    }

    fn pagecross_penalty(&mut self, mode: &AddressingMode) -> u8 {
        match mode {
            AddressingMode::AbsoluteX => {
                let base = self.mem_read_u16(self.pc);
                let addr = base.wrapping_add(self.registers.x as u16);
                (base & 0xFF00 != addr & 0xFF00) as u8
            }
            AddressingMode::AbsoluteY => {
                let base = self.mem_read_u16(self.pc);
                let addr = base.wrapping_add(self.registers.y as u16);
                (base & 0xFF00 != addr & 0xFF00) as u8
            }
            AddressingMode::Relative => {
                let offset = self.mem_read(self.pc) as i8 as i16; // Signed offset
                let new_pc = self.pc.wrapping_add(1).wrapping_add(offset as u16);
                (self.pc.wrapping_add(1) & 0xFF00 != new_pc & 0xFF00) as u8
            }
            AddressingMode::IndirectY => {
                let base = self.mem_read(self.pc);

                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.registers.y as u16);
                (deref & 0xFF00 != deref_base & 0xFF00) as u8
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
    fn test_ora_01() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/01.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_02() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/02.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_slo_03() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/03.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_04() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/04.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ora_05() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/05.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_asl_06() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/06.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_slo_07() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/07.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_php_08() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/08.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ora_09() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/09.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_asl_0a() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/0a.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_anc_0b() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/0b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_0c() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/0c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ora_0d() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/0d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_asl_0e() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/0e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_slo_0f() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/0f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_bpl_10() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/10.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ora_11() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/11.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_12() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/12.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_slo_13() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/13.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_14() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/14.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ora_15() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/15.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_asl_16() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/16.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_slo_17() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/17.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_clc_18() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/18.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ora_19() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/19.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_1a() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/1a.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_slo_1b() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/1b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_1c() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/1c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ora_1d() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/1d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_asl_1e() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/1e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_slo_1f() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/1f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_jsr_20() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/20.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_and_21() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/21.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_22() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/22.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_bit_24() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/24.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_and_25() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/25.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rol_26() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/26.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_plp_28() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/28.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_and_29() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/29.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rol_2a() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/2a.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_anc_2b() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/2b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_bit_2c() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/2c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_and_2d() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/2d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rol_2e() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/2e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_bmi_30() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/30.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_and_31() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/31.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_32() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/32.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_34() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/34.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_and_35() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/35.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rol_36() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/36.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sec_38() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/38.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_and_39() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/39.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_3a() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/3a.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_3c() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/3c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_and_3d() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/3d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rol_3e() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/3e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rti_40() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/40.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_eor_41() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/41.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_42() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/42.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sre_43() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/43.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_44() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/44.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_eor_45() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/45.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lsr_46() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/46.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sre_47() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/47.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_pha_48() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/48.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_eor_49() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/49.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lsr_4a() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/4a.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_jmp_4c() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/4c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_eor_4d() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/4d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lsr_4e() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/4e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sre_4f() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/4f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_bvc_50() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/50.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_eor_51() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/51.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_52() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/52.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sre_53() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/53.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_54() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/54.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_eor_55() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/55.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lsr_56() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/56.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sre_57() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/57.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cli_58() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/58.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_eor_59() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/59.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_5a() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/5a.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sre_5b() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/5b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_5c() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/5c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_eor_5d() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/5d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lsr_5e() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/5e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sre_5f() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/5f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rts_60() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/60.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_adc_61() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/61.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_62() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/62.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rra_63() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/63.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_64() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/64.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_adc_65() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/65.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ror_66() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/66.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rra_67() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/67.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_pla_68() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/68.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_adc_69() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/69.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ror_6a() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/6a.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_jmp_6c() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/6c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_adc_6d() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/6d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ror_6e() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/6e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rra_6f() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/6f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_bvs_70() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/70.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_adc_71() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/71.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_72() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/72.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rra_73() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/73.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_74() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/74.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_adc_75() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/75.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ror_76() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/76.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rra_77() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/77.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sei_78() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/78.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_adc_79() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/79.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_7a() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/7a.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rra_7b() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/7b.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_7c() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/7c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_adc_7d() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/7d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ror_7e() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/7e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_rra_7f() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/7f.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_skb_80() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/80.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sta_81() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/81.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_skb_82() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/82.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sty_84() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/84.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sta_85() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/85.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_stx_86() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/86.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_dey_88() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/88.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_skb_89() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/89.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_txa_8a() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/8a.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sty_8c() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/8c.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sta_8d() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/8d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_stx_8e() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/8e.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_bcc_90() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/90.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sta_91() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/91.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_92() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/92.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sty_94() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/94.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sta_95() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/95.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_stx_96() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/96.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_tya_98() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/98.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sta_99() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/99.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_txs_9a() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/9a.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sta_9d() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/9d.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ldy_a0() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/a0.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lda_a1() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/a1.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ldx_a2() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/a2.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ldy_a4() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/a4.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lda_a5() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/a5.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ldx_a6() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/a6.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_tay_a8() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/a8.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lda_a9() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/a9.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_tax_aa() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/aa.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ldy_ac() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ac.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lda_ad() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ad.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ldx_ae() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ae.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_bcs_b0() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/b0.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lda_b1() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/b1.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_b2() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/b2.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ldy_b4() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/b4.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lda_b5() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/b5.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ldx_b6() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/b6.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_clv_b8() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/b8.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lda_b9() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/b9.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_tsx_ba() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ba.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ldy_bc() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/bc.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_lda_bd() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/bd.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_ldx_be() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/be.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cpy_c0() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/c0.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cmp_c1() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/c1.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_skb_c2() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/c2.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cpy_c4() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/c4.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cmp_c5() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/c5.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_dec_c6() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/c6.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_iny_c8() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/c8.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cmp_c9() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/c9.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_dex_ca() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ca.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cpy_cc() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/cc.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cmp_cd() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/cd.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_dec_ce() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ce.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_bne_d0() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/d0.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cmp_d1() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/d1.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_d2() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/d2.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_d4() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/d4.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cmp_d5() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/d5.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_dec_d6() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/d6.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cld_d8() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/d8.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cmp_d9() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/d9.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_da() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/da.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_dc() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/dc.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cmp_dd() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/dd.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_dec_de() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/de.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cpx_e0() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/e0.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sbc_e1() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/e1.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_skb_e2() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/e2.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_isc_e3() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/e3.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cpx_e4() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/e4.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sbc_e5() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/e5.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_inc_e6() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/e6.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_isc_e7() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/e7.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_inx_e8() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/e8.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sbc_e9() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/e9.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_ea() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ea.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_cpx_ec() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ec.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sbc_ed() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ed.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_inc_ee() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ee.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_isc_ef() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/ef.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_beq_f0() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/f0.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sbc_f1() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/f1.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    #[should_panic(expected = "[CPU] Halten sie!")]
    fn test_hlt_f2() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/f2.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_isc_f3() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/f3.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_f4() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/f4.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sbc_f5() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/f5.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_inc_f6() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/f6.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_isc_f7() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/f7.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sed_f8() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/f8.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sbc_f9() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/f9.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_fa() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/fa.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_isc_fb() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/fb.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_nop_fc() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/fc.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_sbc_fd() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/fd.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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
    fn test_inc_fe() {
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/fe.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {
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

            assert_eq!(cpu.pc, entry.r#final.pc);
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