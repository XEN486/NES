import csv

successes = """
    cpu_test::cpu_test::test_adc_69
    cpu_test::cpu_test::test_adc_65
    cpu_test::cpu_test::test_adc_75
    cpu_test::cpu_test::test_adc_6d
    cpu_test::cpu_test::test_adc_7d
    cpu_test::cpu_test::test_adc_79
    cpu_test::cpu_test::test_adc_61
    cpu_test::cpu_test::test_adc_71
    cpu_test::cpu_test::test_anc_0b
    cpu_test::cpu_test::test_anc_2b
    cpu_test::cpu_test::test_and_29
    cpu_test::cpu_test::test_and_25
    cpu_test::cpu_test::test_and_35
    cpu_test::cpu_test::test_and_2d
    cpu_test::cpu_test::test_and_3d
    cpu_test::cpu_test::test_and_39
    cpu_test::cpu_test::test_and_21
    cpu_test::cpu_test::test_and_31
    cpu_test::cpu_test::test_asl_0a
    cpu_test::cpu_test::test_asl_06
    cpu_test::cpu_test::test_asl_16
    cpu_test::cpu_test::test_asl_0e
    cpu_test::cpu_test::test_asl_1e
    cpu_test::cpu_test::test_bcc_90
    cpu_test::cpu_test::test_bcs_B0
    cpu_test::cpu_test::test_beq_F0
    cpu_test::cpu_test::test_bmi_30
    cpu_test::cpu_test::test_bne_D0
    cpu_test::cpu_test::test_bpl_10
    cpu_test::cpu_test::test_bvc_50
    cpu_test::cpu_test::test_bvs_70
    cpu_test::cpu_test::test_bit_24
    cpu_test::cpu_test::test_bit_2c
    cpu_test::cpu_test::test_clc_18
    cpu_test::cpu_test::test_cld_d8
    cpu_test::cpu_test::test_cli_58
    cpu_test::cpu_test::test_clv_b8
    cpu_test::cpu_test::test_nop_ea
    cpu_test::cpu_test::test_pha_48
    cpu_test::cpu_test::test_pla_68
    cpu_test::cpu_test::test_php_08
    cpu_test::cpu_test::test_plp_28
    cpu_test::cpu_test::test_rti_40
    cpu_test::cpu_test::test_rts_60
    cpu_test::cpu_test::test_sec_38
    cpu_test::cpu_test::test_sed_f8
    cpu_test::cpu_test::test_sei_78
    cpu_test::cpu_test::test_tax_aa
    cpu_test::cpu_test::test_txa_8a
    cpu_test::cpu_test::test_tay_a8
    cpu_test::cpu_test::test_tya_98
    cpu_test::cpu_test::test_tsx_ba
    cpu_test::cpu_test::test_txs_9a
    cpu_test::cpu_test::test_cmp_c9
    cpu_test::cpu_test::test_cmp_c5
    cpu_test::cpu_test::test_cmp_d5
    cpu_test::cpu_test::test_cmp_cd
    cpu_test::cpu_test::test_cmp_dd
    cpu_test::cpu_test::test_cmp_d9
    cpu_test::cpu_test::test_cmp_c1
    cpu_test::cpu_test::test_cmp_d1
    cpu_test::cpu_test::test_cpx_e0
    cpu_test::cpu_test::test_cpx_e4
    cpu_test::cpu_test::test_cpx_ec
    cpu_test::cpu_test::test_cpy_c0
    cpu_test::cpu_test::test_cpy_c4
    cpu_test::cpu_test::test_cpy_cc
    cpu_test::cpu_test::test_dec_c6
    cpu_test::cpu_test::test_dec_d6
    cpu_test::cpu_test::test_dec_ce
    cpu_test::cpu_test::test_dec_de
    cpu_test::cpu_test::test_dex_ca
    cpu_test::cpu_test::test_dey_88
    cpu_test::cpu_test::test_inx_e8
    cpu_test::cpu_test::test_iny_c8
    cpu_test::cpu_test::test_eor_49
    cpu_test::cpu_test::test_eor_45
    cpu_test::cpu_test::test_eor_55
    cpu_test::cpu_test::test_eor_4d
    cpu_test::cpu_test::test_eor_5d
    cpu_test::cpu_test::test_eor_59
    cpu_test::cpu_test::test_eor_41
    cpu_test::cpu_test::test_eor_51
    cpu_test::cpu_test::test_inc_e6
    cpu_test::cpu_test::test_inc_f6
    cpu_test::cpu_test::test_inc_ee
    cpu_test::cpu_test::test_inc_fe
    cpu_test::cpu_test::test_jmp_4c
    cpu_test::cpu_test::test_jmp_6c
    cpu_test::cpu_test::test_jsr_20
    cpu_test::cpu_test::test_lda_a9
    cpu_test::cpu_test::test_lda_a5
    cpu_test::cpu_test::test_lda_b5
    cpu_test::cpu_test::test_lda_ad
    cpu_test::cpu_test::test_lda_bd
    cpu_test::cpu_test::test_lda_b9
    cpu_test::cpu_test::test_lda_a1
    cpu_test::cpu_test::test_lda_b1
    cpu_test::cpu_test::test_ldx_a2
    cpu_test::cpu_test::test_ldx_a6
    cpu_test::cpu_test::test_ldx_b6
    cpu_test::cpu_test::test_ldx_ae
    cpu_test::cpu_test::test_ldx_be
    cpu_test::cpu_test::test_ldy_a0
    cpu_test::cpu_test::test_ldy_a4
    cpu_test::cpu_test::test_ldy_b4
    cpu_test::cpu_test::test_ldy_ac
    cpu_test::cpu_test::test_ldy_bc
    cpu_test::cpu_test::test_lsr_4a
    cpu_test::cpu_test::test_lsr_46
    cpu_test::cpu_test::test_lsr_56
    cpu_test::cpu_test::test_lsr_4e
    cpu_test::cpu_test::test_lsr_5e
    cpu_test::cpu_test::test_ora_09
    cpu_test::cpu_test::test_ora_05
    cpu_test::cpu_test::test_ora_15
    cpu_test::cpu_test::test_ora_0d
    cpu_test::cpu_test::test_ora_1d
    cpu_test::cpu_test::test_ora_19
    cpu_test::cpu_test::test_ora_01
    cpu_test::cpu_test::test_ora_11
    cpu_test::cpu_test::test_rol_2a
    cpu_test::cpu_test::test_rol_26
    cpu_test::cpu_test::test_rol_36
    cpu_test::cpu_test::test_rol_2e
    cpu_test::cpu_test::test_rol_3e
    cpu_test::cpu_test::test_ror_6a
    cpu_test::cpu_test::test_ror_66
    cpu_test::cpu_test::test_ror_76
    cpu_test::cpu_test::test_ror_6e
    cpu_test::cpu_test::test_ror_7e
    cpu_test::cpu_test::test_sbc_e9
    cpu_test::cpu_test::test_sbc_e5
    cpu_test::cpu_test::test_sbc_f5
    cpu_test::cpu_test::test_sbc_ed
    cpu_test::cpu_test::test_sbc_fd
    cpu_test::cpu_test::test_sbc_f9
    cpu_test::cpu_test::test_sbc_e1
    cpu_test::cpu_test::test_sbc_f1
    cpu_test::cpu_test::test_sta_85
    cpu_test::cpu_test::test_sta_95
    cpu_test::cpu_test::test_sta_8d
    cpu_test::cpu_test::test_sta_9d
    cpu_test::cpu_test::test_sta_99
    cpu_test::cpu_test::test_sta_81
    cpu_test::cpu_test::test_sta_91
    cpu_test::cpu_test::test_stx_86
    cpu_test::cpu_test::test_stx_96
    cpu_test::cpu_test::test_stx_8e
    cpu_test::cpu_test::test_sty_84
    cpu_test::cpu_test::test_sty_94
    cpu_test::cpu_test::test_sty_8c
    cpu_test::cpu_test::test_nop_04
    cpu_test::cpu_test::test_nop_44
    cpu_test::cpu_test::test_nop_64
    cpu_test::cpu_test::test_nop_14
    cpu_test::cpu_test::test_nop_34
    cpu_test::cpu_test::test_nop_54
    cpu_test::cpu_test::test_nop_74
    cpu_test::cpu_test::test_nop_d4
    cpu_test::cpu_test::test_nop_f4
    cpu_test::cpu_test::test_nop_0c
    cpu_test::cpu_test::test_nop_1c
    cpu_test::cpu_test::test_nop_3c
    cpu_test::cpu_test::test_nop_5c
    cpu_test::cpu_test::test_nop_7c
    cpu_test::cpu_test::test_nop_dc
    cpu_test::cpu_test::test_nop_fc
    cpu_test::cpu_test::test_nop_1a
    cpu_test::cpu_test::test_nop_3a
    cpu_test::cpu_test::test_nop_5a
    cpu_test::cpu_test::test_nop_7a
    cpu_test::cpu_test::test_nop_da
    cpu_test::cpu_test::test_nop_fa
    cpu_test::cpu_test::test_hlt_02
    cpu_test::cpu_test::test_hlt_12
    cpu_test::cpu_test::test_hlt_22
    cpu_test::cpu_test::test_hlt_32
    cpu_test::cpu_test::test_hlt_42
    cpu_test::cpu_test::test_hlt_52
    cpu_test::cpu_test::test_hlt_62
    cpu_test::cpu_test::test_hlt_72
    cpu_test::cpu_test::test_hlt_92
    cpu_test::cpu_test::test_hlt_b2
    cpu_test::cpu_test::test_hlt_d2
    cpu_test::cpu_test::test_hlt_f2
    cpu_test::cpu_test::test_skb_80
    cpu_test::cpu_test::test_skb_82
    cpu_test::cpu_test::test_skb_89
    cpu_test::cpu_test::test_skb_c2
    cpu_test::cpu_test::test_skb_e2
    cpu_test::cpu_test::test_slo_07
    cpu_test::cpu_test::test_slo_17
    cpu_test::cpu_test::test_slo_0f
    cpu_test::cpu_test::test_slo_1f
    cpu_test::cpu_test::test_slo_1b
    cpu_test::cpu_test::test_slo_03
    cpu_test::cpu_test::test_slo_13
    cpu_test::cpu_test::test_isc_e7
    cpu_test::cpu_test::test_isc_f7
    cpu_test::cpu_test::test_isc_ef
    cpu_test::cpu_test::test_isc_ff
    cpu_test::cpu_test::test_isc_fb
    cpu_test::cpu_test::test_isc_e3
    cpu_test::cpu_test::test_isc_f3
    cpu_test::cpu_test::test_sre_47
    cpu_test::cpu_test::test_sre_57
    cpu_test::cpu_test::test_sre_4f
    cpu_test::cpu_test::test_sre_5f
    cpu_test::cpu_test::test_sre_5b
    cpu_test::cpu_test::test_sre_43
    cpu_test::cpu_test::test_sre_53
    cpu_test::cpu_test::test_rra_67
    cpu_test::cpu_test::test_rra_77
    cpu_test::cpu_test::test_rra_6f
    cpu_test::cpu_test::test_rra_7f
    cpu_test::cpu_test::test_rra_7b
    cpu_test::cpu_test::test_rra_63
    cpu_test::cpu_test::test_rra_73
""".split('\n')
#instructions = {opcode: 'unk' for opcode in range(0x00, 0xFF)}
instructions = {}
with open('6502ops.csv', 'r') as file:
    reader = csv.reader(file)
    for row in reader:
        if not row or f'    cpu_test::cpu_test::test_{row[1].lower()}_{row[0][2:]}' in successes:
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

for i in range(1, 0xFF):
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