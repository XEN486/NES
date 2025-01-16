use bitflags::bitflags;
use crate::bus::*;

macro_rules! new_op {
    ($self:expr, $method:ident, $mode:expr, $pc_inc:expr) => {{
        $self.$method($mode);
        $self.pc += $pc_inc;
    }};
}

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
enum AddressingMode {
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

pub struct CPU {
    pub registers: Registers,
    pub status: u8,
    pub pc: u16,
    pub bus: Bus,
}

impl Mem for CPU {
    fn mem_read(&self, addr: u16) -> u8 {
        self.bus.mem_read(addr)
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.bus.mem_write(addr, data);
    }

    fn mem_read_u16(&self, addr: u16) -> u16 {
        self.bus.mem_read_u16(addr)
    }

    fn mem_write_u16(&mut self, addr: u16, data: u16) {
        self.bus.mem_write_u16(addr, data);
    }
}

impl CPU {
    pub fn new(bus: Bus) -> Self {
        CPU {
            registers: Registers { a: 0, x: 0, y: 0, s: 0xFD },
            status: 0,
            pc: 0,
            bus: bus,
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

    fn get_operand_address(&self, mode: &AddressingMode) -> u16 {
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
                let ptr = base.wrapping_add(self.registers.x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                (hi as u16) << 8 | (lo as u16)
            }
            AddressingMode::IndirectY => {
                let base = self.mem_read(self.pc);
                let ptr = base.wrapping_add(self.registers.y);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                (hi as u16) << 8 | (lo as u16)
            }
            AddressingMode::Relative => (self.pc as i32 + self.mem_read(self.pc) as i8 as i32) as u16,
            AddressingMode::Implied => self.pc,
            AddressingMode::Accumulator => self.pc,
            AddressingMode::Indirect => self.mem_read_u16(self.pc),
        }
    }
    fn step_with_callback<F>(&mut self, mut callback: F) -> bool where F: FnMut(&mut CPU) {
        let opcode = self.mem_read(self.pc);
        self.pc = self.pc.wrapping_add(1);

        match opcode {
            0x00 => return false,

            0x69 => new_op!(self, adc, &AddressingMode::Immediate, 1),
            0x65 => new_op!(self, adc, &AddressingMode::ZeroPage, 1),
            0x75 => new_op!(self, adc, &AddressingMode::ZeroPageX, 1),
            0x6D => new_op!(self, adc, &AddressingMode::Absolute, 2),
            0x7D => new_op!(self, adc, &AddressingMode::AbsoluteX, 2),
            0x79 => new_op!(self, adc, &AddressingMode::AbsoluteY, 2),
            0x61 => new_op!(self, adc, &AddressingMode::IndirectX, 1),
            0x71 => new_op!(self, adc, &AddressingMode::IndirectY, 1),

            0x29 => new_op!(self, and, &AddressingMode::Immediate, 1),
            0x25 => new_op!(self, and, &AddressingMode::ZeroPage, 1),
            0x35 => new_op!(self, and, &AddressingMode::ZeroPageX, 1),
            0x2D => new_op!(self, and, &AddressingMode::Absolute, 2),
            0x3D => new_op!(self, and, &AddressingMode::AbsoluteX, 2),
            0x39 => new_op!(self, and, &AddressingMode::AbsoluteY, 2),
            0x21 => new_op!(self, and, &AddressingMode::IndirectX, 1),
            0x31 => new_op!(self, and, &AddressingMode::IndirectY, 1),

            0x0A => new_op!(self, asl_accumulator, &AddressingMode::Accumulator, 0),
            0x06 => new_op!(self, asl, &AddressingMode::ZeroPage, 1),
            0x16 => new_op!(self, asl, &AddressingMode::ZeroPageX, 1),
            0x0E => new_op!(self, asl, &AddressingMode::Absolute, 2),
            0x1E => new_op!(self, asl, &AddressingMode::AbsoluteX, 2),

            0x90 => new_op!(self, bcc, &AddressingMode::Relative, 1),
            0xB0 => new_op!(self, bcs, &AddressingMode::Relative, 1),
            0xF0 => new_op!(self, beq, &AddressingMode::Relative, 1),

            0x24 => new_op!(self, bit, &AddressingMode::ZeroPage, 1),
            0x2C => new_op!(self, bit, &AddressingMode::Absolute, 2),

            0x30 => new_op!(self, bmi, &AddressingMode::Relative, 1),
            0xD0 => new_op!(self, bne, &AddressingMode::Relative, 1),
            0x10 => new_op!(self, bpl, &AddressingMode::Relative, 1),
            0x50 => new_op!(self, bvc, &AddressingMode::Relative, 1),
            0x70 => new_op!(self, bvs, &AddressingMode::Relative, 1),

            0x18 => self.clear_flag(StatusFlags::Carry),
            0xD8 => self.clear_flag(StatusFlags::DecimalUnused),
            0x58 => self.clear_flag(StatusFlags::InterruptDisable),
            0xB8 => self.clear_flag(StatusFlags::Overflow),

            0xC9 => new_op!(self, cmp, &AddressingMode::Immediate, 1),
            0xC5 => new_op!(self, cmp, &AddressingMode::ZeroPage, 1),
            0xD5 => new_op!(self, cmp, &AddressingMode::ZeroPageX, 1),
            0xCD => new_op!(self, cmp, &AddressingMode::Absolute, 2),
            0xDD => new_op!(self, cmp, &AddressingMode::AbsoluteX, 2),
            0xD9 => new_op!(self, cmp, &AddressingMode::AbsoluteY, 2),
            0xC1 => new_op!(self, cmp, &AddressingMode::IndirectX, 1),
            0xD1 => new_op!(self, cmp, &AddressingMode::IndirectY, 1),

            0xE0 => new_op!(self, cpx, &AddressingMode::Immediate, 1),
            0xE4 => new_op!(self, cpx, &AddressingMode::ZeroPage, 1),
            0xEC => new_op!(self, cpx, &AddressingMode::Absolute, 2),

            0xC0 => new_op!(self, cpy, &AddressingMode::Immediate, 1),
            0xC4 => new_op!(self, cpy, &AddressingMode::ZeroPage, 1),
            0xCC => new_op!(self, cpy, &AddressingMode::Absolute, 2),

            0xC6 => new_op!(self, dec, &AddressingMode::ZeroPage, 1),
            0xD6 => new_op!(self, dec, &AddressingMode::ZeroPageX, 1),
            0xCE => new_op!(self, dec, &AddressingMode::Absolute, 2),
            0xDE => new_op!(self, dec, &AddressingMode::AbsoluteX, 2),

            0xCA => new_op!(self, dex, &AddressingMode::Implied, 0),
            0x88 => new_op!(self, dey, &AddressingMode::Implied, 0),

            0x49 => new_op!(self, eor, &AddressingMode::Immediate, 1),
            0x45 => new_op!(self, eor, &AddressingMode::ZeroPage, 1),
            0x55 => new_op!(self, eor, &AddressingMode::ZeroPageX, 1),
            0x4D => new_op!(self, eor, &AddressingMode::Absolute, 2),
            0x5D => new_op!(self, eor, &AddressingMode::AbsoluteX, 2),
            0x59 => new_op!(self, eor, &AddressingMode::AbsoluteY, 2),
            0x41 => new_op!(self, eor, &AddressingMode::IndirectX, 1),
            0x51 => new_op!(self, eor, &AddressingMode::IndirectY, 1),

            0xE6 => new_op!(self, inc, &AddressingMode::ZeroPage, 1),
            0xF6 => new_op!(self, inc, &AddressingMode::ZeroPageX, 1),
            0xEE => new_op!(self, inc, &AddressingMode::Absolute, 2),
            0xFE => new_op!(self, inc, &AddressingMode::AbsoluteX, 2),

            0xE8 => new_op!(self, inx, &AddressingMode::Implied, 0),
            0xC8 => new_op!(self, iny, &AddressingMode::Implied, 0),

            0x4C => new_op!(self, jmp, &AddressingMode::Absolute, 2),
            0x6C => new_op!(self, jmp, &AddressingMode::Indirect, 2),

            0x20 => new_op!(self, jsr, &AddressingMode::Absolute, 2),

            0xA9 => new_op!(self, lda, &AddressingMode::Immediate, 1),
            0xA5 => new_op!(self, lda, &AddressingMode::ZeroPage, 1),
            0xB5 => new_op!(self, lda, &AddressingMode::ZeroPageX, 1),
            0xAD => new_op!(self, lda, &AddressingMode::Absolute, 2),
            0xBD => new_op!(self, lda, &AddressingMode::AbsoluteX, 2),
            0xB9 => new_op!(self, lda, &AddressingMode::AbsoluteY, 2),
            0xA1 => new_op!(self, lda, &AddressingMode::IndirectX, 1),
            0xB1 => new_op!(self, lda, &AddressingMode::IndirectY, 1),

            0xA2 => new_op!(self, ldx, &AddressingMode::Immediate, 1),
            0xA6 => new_op!(self, ldx, &AddressingMode::ZeroPage, 1),
            0xB6 => new_op!(self, ldx, &AddressingMode::ZeroPageY, 1),
            0xAE => new_op!(self, ldx, &AddressingMode::Absolute, 2),
            0xBE => new_op!(self, ldx, &AddressingMode::AbsoluteY, 2),

            0xA0 => new_op!(self, ldy, &AddressingMode::Immediate, 1),
            0xA4 => new_op!(self, ldy, &AddressingMode::ZeroPage, 1),
            0xB4 => new_op!(self, ldy, &AddressingMode::ZeroPageX, 1),
            0xAC => new_op!(self, ldy, &AddressingMode::Absolute, 2),
            0xBC => new_op!(self, ldy, &AddressingMode::AbsoluteX, 2),

            0x4A => new_op!(self, lsr_accumulator, &AddressingMode::Accumulator, 0),
            0x46 => new_op!(self, lsr, &AddressingMode::ZeroPage, 1),
            0x56 => new_op!(self, lsr, &AddressingMode::ZeroPageX, 1),
            0x4E => new_op!(self, lsr, &AddressingMode::Absolute, 2),
            0x5E => new_op!(self, lsr, &AddressingMode::AbsoluteX, 2),

            0xEA => {},

            0x09 => new_op!(self, ora, &AddressingMode::Immediate, 1),
            0x05 => new_op!(self, ora, &AddressingMode::ZeroPage, 1),
            0x15 => new_op!(self, ora, &AddressingMode::ZeroPageX, 1),
            0x0D => new_op!(self, ora, &AddressingMode::Absolute, 2),
            0x1D => new_op!(self, ora, &AddressingMode::AbsoluteX, 2),
            0x19 => new_op!(self, ora, &AddressingMode::AbsoluteY, 2),
            0x01 => new_op!(self, ora, &AddressingMode::IndirectX, 1),
            0x11 => new_op!(self, ora, &AddressingMode::IndirectY, 1),

            0x48 => self.stack_push(self.registers.a),
            0x08 => self.php(),
            0x68 => self.pla(),
            0x28 => self.plp(),

            0x2A => new_op!(self, rol_accumulator, &AddressingMode::Accumulator, 0),
            0x26 => new_op!(self, rol, &AddressingMode::ZeroPage, 1),
            0x36 => new_op!(self, rol, &AddressingMode::ZeroPageX, 1),
            0x2E => new_op!(self, rol, &AddressingMode::Absolute, 2),
            0x3E => new_op!(self, rol, &AddressingMode::AbsoluteX, 2),

            0x6A => new_op!(self, ror_accumulator, &AddressingMode::Accumulator, 0),
            0x66 => new_op!(self, ror, &AddressingMode::ZeroPage, 1),
            0x67 => new_op!(self, ror, &AddressingMode::ZeroPageX, 1),
            0x6E => new_op!(self, ror, &AddressingMode::Absolute, 2),
            0x7E => new_op!(self, ror, &AddressingMode::AbsoluteX, 2),

            0x40 => new_op!(self, rti, &AddressingMode::Implied, 0),
            0x60 => new_op!(self, rts, &AddressingMode::Implied, 0),

            0xE9 => new_op!(self, sbc, &AddressingMode::Immediate, 1),
            0xE5 => new_op!(self, sbc, &AddressingMode::ZeroPage, 1),
            0xF5 => new_op!(self, sbc, &AddressingMode::ZeroPageX, 1),
            0xED => new_op!(self, sbc, &AddressingMode::Absolute, 2),
            0xFD => new_op!(self, sbc, &AddressingMode::AbsoluteX, 2),
            0xF9 => new_op!(self, sbc, &AddressingMode::AbsoluteY, 2),
            0xE1 => new_op!(self, sbc, &AddressingMode::IndirectX, 1),
            0xF1 => new_op!(self, sbc, &AddressingMode::IndirectY, 1),

            0x38 => self.set_flag(StatusFlags::Carry),
            0xF8 => self.set_flag(StatusFlags::DecimalUnused),
            0x78 => self.set_flag(StatusFlags::InterruptDisable),

            0x85 => new_op!(self, sta, &AddressingMode::ZeroPage, 1),
            0x95 => new_op!(self, sta, &AddressingMode::ZeroPageX, 1),
            0x8D => new_op!(self, sta, &AddressingMode::Absolute, 2),
            0x9D => new_op!(self, sta, &AddressingMode::AbsoluteX, 2),
            0x99 => new_op!(self, sta, &AddressingMode::AbsoluteY, 2),
            0x81 => new_op!(self, sta, &AddressingMode::IndirectX, 1),
            0x91 => new_op!(self, sta, &AddressingMode::IndirectY, 1),

            0x86 => new_op!(self, stx, &AddressingMode::ZeroPage, 1),
            0x96 => new_op!(self, stx, &AddressingMode::ZeroPageY, 1),
            0x8E => new_op!(self, stx, &AddressingMode::Absolute, 2),

            0x84 => new_op!(self, sty, &AddressingMode::ZeroPage, 1),
            0x94 => new_op!(self, sty, &AddressingMode::ZeroPageX, 1),
            0x8C => new_op!(self, sty, &AddressingMode::Absolute, 2),

            0xAA => new_op!(self, tax, &AddressingMode::Implied, 0),
            0xA8 => new_op!(self, tay, &AddressingMode::Implied, 0),
            0xBA => new_op!(self, tsx, &AddressingMode::Implied, 0),
            0x8A => new_op!(self, txa, &AddressingMode::Implied, 0),
            0x9A => new_op!(self, txs, &AddressingMode::Implied, 0),
            0x98 => new_op!(self, tya, &AddressingMode::Implied, 0),

            _ => todo!("[CPU] unimplemented opcode! 0x{:02x}", opcode),
        }

        callback(self);
        true
    }

    fn step(&mut self) -> bool {
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

    pub fn load(&mut self, program: Vec<u8>) {
        for i in 0..(program.len() as u16) {
            self.mem_write(0x8600 + i, program[i as usize]);
        }
        self.mem_write_u16(0xFFFC, 0x8600);
    }

    pub fn run_with_callback<F>(&mut self, mut callback: F) where F: FnMut(&mut CPU) {
        while self.step_with_callback(&mut callback) {}
    }

    pub fn run(&mut self) {
        while self.step() {}
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }
}

include!("./opcodes.rs");