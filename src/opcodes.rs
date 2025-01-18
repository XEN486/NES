impl<'a> CPU<'a> {
    fn adc(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        let carry = if self.status & StatusFlags::Carry.bits() != 0 { 1 } else { 0 };
    
        let result = self.registers.a as u16 + value as u16 + carry as u16;
        self.set_flag_else_clear(StatusFlags::Carry, result > 0xFF);
        
        let result_u8 = result as u8;
        self.set_flag_else_clear(StatusFlags::Overflow, (self.registers.a ^ result_u8) & (value ^ result_u8) & 0x80 != 0);
    
        self.registers.a = result_u8;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);

        false
    }

    fn and(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        
        self.registers.a &= value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);

        false
    }

    fn asl(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.mem_write(addr, value << 1 & 0b1111_1110);
        self.set_flag_else_clear(StatusFlags::Carry, (value & 0x80) != 0);
        self.set_flag_else_clear(StatusFlags::Zero, (value << 1) == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (value & 0x80) != 0);

        false
    }

    fn asl_accumulator(&mut self, _mode: &AddressingMode) -> bool {
        let value = self.registers.a;
        self.registers.a = (value << 1) & 0b1111_1110;

        self.set_flag_else_clear(StatusFlags::Carry, (value & 0x80) != 0);
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);

        false
    }

    fn bcc(&mut self, mode: &AddressingMode) -> bool {
        if self.status & StatusFlags::Carry.bits() == 0 {
            self.pc = self.get_operand_address(mode);
        }

        false
    }

    fn bcs(&mut self, mode: &AddressingMode) -> bool {
        if self.status & StatusFlags::Carry.bits() != 0 {
            self.pc = self.get_operand_address(mode);
        }

        false
    }

    fn beq(&mut self, mode: &AddressingMode) -> bool {
        if self.status & StatusFlags::Zero.bits() != 0 {
            self.pc = self.get_operand_address(mode);
        }

        false
    }

    fn bit(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        let and = self.registers.a & data;

        self.set_flag_else_clear(StatusFlags::Zero, and == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (data & 0b10000000) != 0);
        self.set_flag_else_clear(StatusFlags::Overflow, (data & 0b01000000) != 0);

        false
    }

    fn bmi(&mut self, mode: &AddressingMode) -> bool {
        if self.status & StatusFlags::Negative.bits() != 0 {
            self.pc = self.get_operand_address(mode);
        }

        false
    }

    fn bne(&mut self, mode: &AddressingMode) -> bool {
        if self.status & StatusFlags::Zero.bits() == 0 {
            self.pc = self.get_operand_address(mode);
        }

        false
    }

    fn bpl(&mut self, mode: &AddressingMode) -> bool {
        if self.status & StatusFlags::Negative.bits() == 0 {
            self.pc = self.get_operand_address(mode);
        }

        false
    }

    fn brk(&mut self, _mode: &AddressingMode) -> bool {
        self.pc += 1;
        
        if self.status & StatusFlags::InterruptDisable.bits() == 0 {
            self.interrupt(BRK);
        }

        false
    }

    fn bvc(&mut self, mode: &AddressingMode) -> bool {
        if self.status & StatusFlags::Overflow.bits() == 0 {
            self.pc = self.get_operand_address(mode);
        }

        false
    }

    fn bvs(&mut self, mode: &AddressingMode) -> bool {
        if self.status & StatusFlags::Overflow.bits() != 0 {
            self.pc = self.get_operand_address(mode);
        }

        false
    }

    fn clc(&mut self, _mode: &AddressingMode) -> bool {
        self.clear_flag(StatusFlags::Carry);

        false
    }

    fn cld(&mut self, _mode: &AddressingMode) -> bool {
        self.clear_flag(StatusFlags::DecimalUnused);
        
        false
    }

    fn cli(&mut self, _mode: &AddressingMode) -> bool {
        self.clear_flag(StatusFlags::InterruptDisable);
        
        false
    }

    fn clv(&mut self, _mode: &AddressingMode) -> bool {
        self.clear_flag(StatusFlags::Overflow);
        
        false
    }

    fn cmp(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.set_flag_else_clear(StatusFlags::Carry, data <= self.registers.a);

        let subbed = self.registers.a.wrapping_sub(data);
        self.set_flag_else_clear(StatusFlags::Zero, subbed == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (subbed >> 7) == 1);

        false
    }

    fn cpx(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.set_flag_else_clear(StatusFlags::Carry, data <= self.registers.x);

        let subbed = self.registers.x.wrapping_sub(data);
        self.set_flag_else_clear(StatusFlags::Zero, subbed == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (subbed >> 7) == 1);

        false
    }

    fn cpy(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.set_flag_else_clear(StatusFlags::Carry, data <= self.registers.y);

        let subbed = self.registers.y.wrapping_sub(data);
        self.set_flag_else_clear(StatusFlags::Zero, subbed == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (subbed >> 7) == 1);

        false
    }

    fn dec(&mut self, mode: &AddressingMode) -> bool {
        let addr: u16 = self.get_operand_address(mode);
        let value: u8 = self.mem_read(addr);
        let result: u8 = value.wrapping_sub(1);

        self.mem_write(addr, result);
        self.set_flag_else_clear(StatusFlags::Zero, result == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (result & 0x80) != 0);

        false
    }

    fn dex(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.x = self.registers.x.wrapping_sub(1);

        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.x & 0x80) != 0);

        false
    }

    fn dey(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.y = self.registers.y.wrapping_sub(1);

        self.set_flag_else_clear(StatusFlags::Zero, self.registers.y == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.y & 0x80) != 0);

        false
    }

    fn eor(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        
        self.registers.a ^= value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);

        false
    }

    fn inc(&mut self, mode: &AddressingMode) -> bool {
        let addr: u16 = self.get_operand_address(mode);
        let value: u8 = self.mem_read(addr);
        let result: u8 = value.wrapping_add(1);

        self.mem_write(addr, result);
        self.set_flag_else_clear(StatusFlags::Zero, result == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (result & 0x80) != 0);

        false
    }

    fn inx(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.x = self.registers.x.wrapping_add(1);
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.x & 0x80 != 0);

        false
    }

    fn iny(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.y = self.registers.y.wrapping_add(1);
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.y == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.y & 0x80 != 0);

        false
    }

    fn isc(&mut self, mode: &AddressingMode) -> bool {
        self.inc(mode);
        self.sbc(mode);
        false
    }

    fn jmp(&mut self, mode: &AddressingMode) -> bool {
        let addr: u16 = self.get_operand_address(mode);
        self.pc = addr - 2; // 1 word added on after this function runs

        false
    }

    fn jsr(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);

        self.stack_push_u16(self.pc + 2 - 1);
        self.pc = addr - 2; // 1 word added on after this function runs

        false
    }

    fn lda(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.registers.a = value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);

        false
    }

    fn ldx(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.registers.x = value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.x & 0x80) != 0);

        false
    }

    fn ldy(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.registers.y = value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.y == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.y & 0x80) != 0);

        false
    }

    fn lsr(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.mem_write(addr, (value >> 1) & 0b0111_1111);
        self.set_flag_else_clear(StatusFlags::Carry, (value & 0b0000_0001) != 0);
        self.set_flag_else_clear(StatusFlags::Zero, value == 0);

        false
    }

    fn lsr_accumulator(&mut self, _mode: &AddressingMode) -> bool {
        let value = self.registers.a;
        self.registers.a = (value >> 1) & 0b0111_1111;

        self.set_flag_else_clear(StatusFlags::Carry, (value & 0b0000_0001) != 0);
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);

        false
    }

    fn nop(&mut self, _mode: &AddressingMode) -> bool {
        false
    }

    fn ora(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        
        self.registers.a |= value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);

        false
    }

    fn pha(&mut self, _mode: &AddressingMode) -> bool {
        self.stack_push(self.registers.a);
        
        false
    }

    fn php(&mut self, _mode: &AddressingMode) -> bool {
        let status = self.status & !StatusFlags::Break.bits();
        self.stack_push(status);

        false
    }
    
    fn pla(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.a = self.stack_pop();
        false
    }

    fn plp(&mut self, _mode: &AddressingMode) -> bool {
        self.status = self.stack_pop();
        self.clear_flag(StatusFlags::Break);
        self.set_flag(StatusFlags::Break2);

        false
    }

    fn rol(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        let old_b7 = value & 1;

        value <<= 1;
        if self.status & StatusFlags::Carry.bits() != 0 {
            value |= 0b0000_0001;
        } else {
            value &= 0b1111_1110;
        }

        self.mem_write(addr, value);
        self.set_flag_else_clear(StatusFlags::Carry, old_b7 == 1);
        self.set_flag_else_clear(StatusFlags::Zero, value == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (value & 0x80) != 0);

        false
    }

    fn rol_accumulator(&mut self, _mode: &AddressingMode) -> bool {
        let old_b7 = self.registers.a & 1;

        self.registers.a <<= 1;
        if self.status & StatusFlags::Carry.bits() != 0 {
            self.registers.a |= 0b0000_0001;
        } else {
            self.registers.a &= 0b1111_1110;
        }

        self.set_flag_else_clear(StatusFlags::Carry, old_b7 == 1);
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);

        false
    }

    fn ror_accumulator(&mut self, _mode: &AddressingMode) -> bool {
        let old_b0 = self.registers.a & 0b0000_0001;

        self.registers.a >>= 1;
        if self.status & StatusFlags::Carry.bits() == 1 {
            self.registers.a |= 0b1000_0000;
        } else {
            self.registers.a &= 0b0111_1111;
        }

        self.set_flag_else_clear(StatusFlags::Carry, old_b0 != 0);
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);

        false
    }

    fn ror(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        let old_b0 = value & 0b1000_0000;

        value >>= 1;
        if self.status & StatusFlags::Carry.bits() == 1 {
            value |= 0b1000_0000;
        } else {
            value &= 0b0111_1111;
        }

        self.mem_write(addr, value);
        self.set_flag_else_clear(StatusFlags::Carry, old_b0 != 0);
        self.set_flag_else_clear(StatusFlags::Zero, value == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (value & 0x80) != 0);

        false
    }

    fn rti(&mut self, _mode: &AddressingMode) -> bool {
        self.status = self.stack_pop();
        self.clear_flag(StatusFlags::Break);
        self.set_flag(StatusFlags::Break2);

        self.pc = self.stack_pop_u16();

        false
    }

    fn rts(&mut self, _mode: &AddressingMode) -> bool {
        self.pc = self.stack_pop_u16() + 1;

        false
    }

    fn sbc(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        let carry = if self.status & StatusFlags::Carry.bits() != 0 { 0 } else { 1 };
        let result = self.registers.a.wrapping_sub(value).wrapping_sub(carry);
    
        self.set_flag_else_clear(StatusFlags::Carry, self.registers.a >= value + carry);
        self.set_flag_else_clear(StatusFlags::Overflow, ((self.registers.a ^ value) & (self.registers.a ^ result) & 0x80) != 0);
    
        self.registers.a = result;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);

        false
    }

    fn sec(&mut self, _mode: &AddressingMode) -> bool {
        self.set_flag(StatusFlags::Carry);

        false
    }

    fn sed(&mut self, _mode: &AddressingMode) -> bool {
        self.set_flag(StatusFlags::DecimalUnused);
        
        false
    }

    fn sei(&mut self, _mode: &AddressingMode) -> bool {
        self.set_flag(StatusFlags::InterruptDisable);
        
        false
    }

    fn skb(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        let _ = self.mem_read(addr);

        false
    }

    fn slo(&mut self, mode: &AddressingMode) -> bool {
        self.asl(&mode);
        self.ora(&mode);
        
        false
    }

    fn sre(&mut self, mode: &AddressingMode) -> bool {
        self.lsr(mode);
        self.eor(mode);

        false
    }

    fn sta(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.registers.a);
        
        false
    }

    fn stx(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.registers.x);

        false
    }

    fn sty(&mut self, mode: &AddressingMode) -> bool {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.registers.y);

        false
    }

    fn tax(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.x = self.registers.a;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.x & 0x80 != 0);

        false
    }

    fn tay(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.y = self.registers.a;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.y == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.y & 0x80 != 0);

        false
    }

    fn tsx(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.x = self.registers.s;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.x & 0x80 != 0);

        false
    }

    fn txa(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.a = self.registers.x;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.a & 0x80 != 0);

        false
    }

    fn txs(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.s = self.registers.x;

        false
    }

    fn tya(&mut self, _mode: &AddressingMode) -> bool {
        self.registers.a = self.registers.y;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.a & 0x80 != 0);

        false
    }
}