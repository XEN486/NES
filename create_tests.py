s = """
    cpu_test::cpu_test::test_11
    cpu_test::cpu_test::test_12
    cpu_test::cpu_test::test_19
    cpu_test::cpu_test::test_1b
    cpu_test::cpu_test::test_1c
    cpu_test::cpu_test::test_1d
    cpu_test::cpu_test::test_22
    cpu_test::cpu_test::test_23
    cpu_test::cpu_test::test_27
    cpu_test::cpu_test::test_2f
    cpu_test::cpu_test::test_31
    cpu_test::cpu_test::test_32
    cpu_test::cpu_test::test_33
    cpu_test::cpu_test::test_37
    cpu_test::cpu_test::test_39
    cpu_test::cpu_test::test_3b
    cpu_test::cpu_test::test_3c
    cpu_test::cpu_test::test_3d
    cpu_test::cpu_test::test_3f
    cpu_test::cpu_test::test_42
    cpu_test::cpu_test::test_46
    cpu_test::cpu_test::test_4b
    cpu_test::cpu_test::test_4e
    cpu_test::cpu_test::test_51
    cpu_test::cpu_test::test_52
    cpu_test::cpu_test::test_53
    cpu_test::cpu_test::test_56
    cpu_test::cpu_test::test_59
    cpu_test::cpu_test::test_5c
    cpu_test::cpu_test::test_5d
    cpu_test::cpu_test::test_5e
    cpu_test::cpu_test::test_62
    cpu_test::cpu_test::test_63
    cpu_test::cpu_test::test_66
    cpu_test::cpu_test::test_67
    cpu_test::cpu_test::test_68
    cpu_test::cpu_test::test_6b
    cpu_test::cpu_test::test_6e
    cpu_test::cpu_test::test_6f
    cpu_test::cpu_test::test_71
    cpu_test::cpu_test::test_72
    cpu_test::cpu_test::test_73
    cpu_test::cpu_test::test_76
    cpu_test::cpu_test::test_77
    cpu_test::cpu_test::test_79
    cpu_test::cpu_test::test_7b
    cpu_test::cpu_test::test_7c
    cpu_test::cpu_test::test_7d
    cpu_test::cpu_test::test_7e
    cpu_test::cpu_test::test_7f
    cpu_test::cpu_test::test_83
    cpu_test::cpu_test::test_87
    cpu_test::cpu_test::test_8b
    cpu_test::cpu_test::test_8f
    cpu_test::cpu_test::test_92
    cpu_test::cpu_test::test_93
    cpu_test::cpu_test::test_97
    cpu_test::cpu_test::test_9b
    cpu_test::cpu_test::test_9c
    cpu_test::cpu_test::test_9e
    cpu_test::cpu_test::test_9f
    cpu_test::cpu_test::test_a3
    cpu_test::cpu_test::test_a7
    cpu_test::cpu_test::test_ab
    cpu_test::cpu_test::test_af
    cpu_test::cpu_test::test_b1
    cpu_test::cpu_test::test_b2
    cpu_test::cpu_test::test_b3
    cpu_test::cpu_test::test_b7
    cpu_test::cpu_test::test_b9
    cpu_test::cpu_test::test_bb
    cpu_test::cpu_test::test_bc
    cpu_test::cpu_test::test_bd
    cpu_test::cpu_test::test_be
    cpu_test::cpu_test::test_bf
    cpu_test::cpu_test::test_c3
    cpu_test::cpu_test::test_c7
    cpu_test::cpu_test::test_cb
    cpu_test::cpu_test::test_cf
    cpu_test::cpu_test::test_d1
    cpu_test::cpu_test::test_d2
    cpu_test::cpu_test::test_d3
    cpu_test::cpu_test::test_d7
    cpu_test::cpu_test::test_d9
    cpu_test::cpu_test::test_db
    cpu_test::cpu_test::test_dc
    cpu_test::cpu_test::test_dd
    cpu_test::cpu_test::test_df
    cpu_test::cpu_test::test_e3
    cpu_test::cpu_test::test_eb
    cpu_test::cpu_test::test_f1
    cpu_test::cpu_test::test_f2
    cpu_test::cpu_test::test_f3
    cpu_test::cpu_test::test_f9
    cpu_test::cpu_test::test_fc
    cpu_test::cpu_test::test_fd
"""
a = []
for i in s.split('\n'):
    a.append(i[-2:])

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

"""

for i in range(0, 0xFF):
    if hex(i)[2:] not in a:
        continue

    boilerplate += """
    #[test]
    fn test_{:02x}() {{
        let mut cpu = CPU::new();
        cpu.reset();

        println!("parsing file");
        let file = File::open("v1/{:02x}.json").expect("failed to open file");
        let data: Vec<Data> = serde_json::from_reader(file).expect("failed parsing json");

        for entry in data {{
            //println!("test: {{}}", entry.name);
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

            //assert_eq!(cpu.pc, entry.r#final.pc);
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

    """.format(i, i)

boilerplate += "\n}"
with open('src/cpu_test.rs', 'w') as f:
    f.write(boilerplate)