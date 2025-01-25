import csv

failures = """
    cpu_test::cpu_test::test_ror_6e
    cpu_test::cpu_test::test_ror_7e
    cpu_test::cpu_test::test_slo_03
    cpu_test::cpu_test::test_slo_1b
    cpu_test::cpu_test::test_sre_53
""".split('\n')
#instructions = {opcode: 'unk' for opcode in range(0x00, 0xFF)}
instructions = {}
with open('6502ops.csv', 'r') as file:
    reader = csv.reader(file)
    for row in reader:
        if not row or f'    cpu_test::cpu_test::test_{row[1].lower()}_{row[0][2:]}' not in failures:
            continue
        
        try:
            instructions[int(row[0], 16)] = row[1].lower()
        except:
            pass


boilerplate = """
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
        self.stack_push_u16(self.pc + 2);
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

"""

for i in range(0, 0xFF):
    try:
        boilerplate += """
    #[test]{}
    fn test_{}_{:02x}() {{
        let mut cpu = CPU::new();
        cpu.reset();

        let file = File::open("v1/{:02x}.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {{
            for r in entry.initial.ram.iter() {{
                cpu.memory[r.0 as usize] = r.1;
            }}

            cpu.pc = entry.initial.pc;
            cpu.registers.s = entry.initial.s;
            cpu.registers.a = entry.initial.a;
            cpu.registers.x = entry.initial.x;
            cpu.registers.y = entry.initial.y;
            cpu.status = entry.initial.p;
            
            let mut cycles_executed = 0;
            while cycles_executed < entry.cycles.len() {{
                let cycles = cpu.step();
                cycles_executed += cycles as usize;
            }}

            assert_eq!(cpu.pc, entry.r#final.pc);
            assert_eq!(cpu.registers.s, entry.r#final.s);
            assert_eq!(cpu.registers.a, entry.r#final.a);
            assert_eq!(cpu.registers.x, entry.r#final.x);
            assert_eq!(cpu.registers.y, entry.r#final.y);
            assert_eq!(cpu.status, entry.r#final.p);

            for r in entry.r#final.ram.iter() {{
                assert_eq!(cpu.memory[r.0 as usize], r.1);
            }}
        }}
    }}

        """.format('\n    #[should_panic(expected = "[CPU] Halten sie!")]' if instructions[i] == 'hlt' else '', instructions[i], i, i)
    except:
        pass

boilerplate += "\n}"
with open('src/cpu_test.rs', 'w') as f:
    f.write(boilerplate)