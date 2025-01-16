impl CPU {
    fn adc(&mut self, mode: &AddressingMode) {
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
    }

    fn and(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        
        self.registers.a &= value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);
    }

    fn asl(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.mem_write(addr, value << 1 & 0b1111_1110);
        self.set_flag_else_clear(StatusFlags::Carry, (value & 0x80) != 0);
        self.set_flag_else_clear(StatusFlags::Zero, value == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (value & 0x80) != 0);
    }

    fn asl_accumulator(&mut self, _mode: &AddressingMode) {
        let value = self.registers.a;
        self.registers.a = (value << 1) & 0b1111_1110;

        self.set_flag_else_clear(StatusFlags::Carry, (value & 0x80) != 0);
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);
    }

    fn bcc(&mut self, mode: &AddressingMode) {
        if self.status & StatusFlags::Carry.bits() == 0 {
            self.pc = self.get_operand_address(mode);
        }
    }

    fn bcs(&mut self, mode: &AddressingMode) {
        if self.status & StatusFlags::Carry.bits() != 0 {
            self.pc = self.get_operand_address(mode);
        }
    }

    fn beq(&mut self, mode: &AddressingMode) {
        if self.status & StatusFlags::Zero.bits() != 0 {
            self.pc = self.get_operand_address(mode);
        }
    }

    fn bit(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        let and = self.registers.a & data;

        self.set_flag_else_clear(StatusFlags::Zero, and == 0);
        self.set_flag_else_clear(StatusFlags::Negative, data & 0b10000000 > 0);
        self.set_flag_else_clear(StatusFlags::Overflow, data & 0b01000000 > 0);
    }

    fn bmi(&mut self, mode: &AddressingMode) {
        if self.status & StatusFlags::Negative.bits() != 0 {
            self.pc = self.get_operand_address(mode);
        }
    }

    fn bne(&mut self, mode: &AddressingMode) {
        if self.status & StatusFlags::Zero.bits() == 0 {
            self.pc = self.get_operand_address(mode);
        }
    }

    fn bpl(&mut self, mode: &AddressingMode) {
        if self.status & StatusFlags::Negative.bits() == 0 {
            self.pc = self.get_operand_address(mode);
        }
    }

    fn bvc(&mut self, mode: &AddressingMode) {
        if self.status & StatusFlags::Overflow.bits() == 0 {
            self.pc = self.get_operand_address(mode);
        }
    }

    fn bvs(&mut self, mode: &AddressingMode) {
        if self.status & StatusFlags::Overflow.bits() != 0 {
            self.pc = self.get_operand_address(mode);
        }
    }

    fn cmp(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.set_flag_else_clear(StatusFlags::Carry, data <= self.registers.a);

        let subbed = self.registers.a.wrapping_sub(data);
        self.set_flag_else_clear(StatusFlags::Zero, subbed == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (subbed >> 7) == 1);
    }

    fn cpx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.set_flag_else_clear(StatusFlags::Carry, data <= self.registers.x);

        let subbed = self.registers.x.wrapping_sub(data);
        self.set_flag_else_clear(StatusFlags::Zero, subbed == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (subbed >> 7) == 1);
    }

    fn cpy(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.set_flag_else_clear(StatusFlags::Carry, data <= self.registers.y);

        let subbed = self.registers.y.wrapping_sub(data);
        self.set_flag_else_clear(StatusFlags::Zero, subbed == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (subbed >> 7) == 1);
    }

    fn dec(&mut self, mode: &AddressingMode) {
        let addr: u16 = self.get_operand_address(mode);
        let value: u8 = self.mem_read(addr);
        let result: u8 = value.wrapping_sub(1);

        self.mem_write(addr, result);
        self.set_flag_else_clear(StatusFlags::Zero, result == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (result & 0x80) != 0);
    }

    fn dex(&mut self, _mode: &AddressingMode) {
        self.registers.x = self.registers.x.wrapping_sub(1);

        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.x & 0x80) != 0);
    }

    fn dey(&mut self, _mode: &AddressingMode) {
        self.registers.y = self.registers.y.wrapping_sub(1);

        self.set_flag_else_clear(StatusFlags::Zero, self.registers.y == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.y & 0x80) != 0);
    }

    fn eor(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        
        self.registers.a ^= value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);
    }

    fn inc(&mut self, mode: &AddressingMode) {
        let addr: u16 = self.get_operand_address(mode);
        let value: u8 = self.mem_read(addr);
        let result: u8 = value.wrapping_add(1);

        self.mem_write(addr, result);
        self.set_flag_else_clear(StatusFlags::Zero, result == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (result & 0x80) != 0);
    }

    fn inx(&mut self, _mode: &AddressingMode) {
        self.registers.x = self.registers.x.wrapping_add(1);
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.x & 0x80 != 0);
    }

    fn iny(&mut self, _mode: &AddressingMode) {
        self.registers.y = self.registers.y.wrapping_add(1);
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.x & 0x80 != 0);
    }

    fn jmp(&mut self, mode: &AddressingMode) {
        let addr: u16 = self.get_operand_address(mode);
        self.pc = addr - 2; // 1 word added on after this function runs
    }

    fn jsr(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);

        self.stack_push_u16(self.pc + 2 - 1);
        self.pc = addr - 2; // 1 word added on after this function runs
    }

    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.registers.a = value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);
    }

    fn ldx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.registers.x = value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.x & 0x80) != 0);
    }

    fn ldy(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.registers.y = value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.y == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.y & 0x80) != 0);
    }

    fn lsr(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.mem_write(addr, (value >> 1) & 0b0111_1111);
        self.set_flag_else_clear(StatusFlags::Carry, (value & 0b0000_0001) != 0);
        self.set_flag_else_clear(StatusFlags::Zero, value == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (value & 0x80) != 0);
    }

    fn lsr_accumulator(&mut self, _mode: &AddressingMode) {
        let value = self.registers.a;
        self.registers.a = (value >> 1) & 0b0111_1111;

        self.set_flag_else_clear(StatusFlags::Carry, (value & 0b0000_0001) != 0);
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);
    }

    fn ora(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        
        self.registers.a |= value;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);
    }

    fn pla(&mut self) {
        self.registers.a = self.stack_pop();
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);
    }

    fn plp(&mut self) {
        self.status = self.stack_pop();
        self.clear_flag(StatusFlags::Break);
        self.set_flag(StatusFlags::Break2);
    }

    fn php(&mut self) {
        self.set_flag(StatusFlags::Break);
        self.set_flag(StatusFlags::Break2);
        self.stack_push(self.status.clone());
    }

    fn rol(&mut self, mode: &AddressingMode) {
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
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (value & 0x80) != 0);
    }

    fn rol_accumulator(&mut self, _mode: &AddressingMode) {
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
    }

    fn ror_accumulator(&mut self, _mode: &AddressingMode) {
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
    }

    fn ror(&mut self, mode: &AddressingMode) {
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
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (value & 0x80) != 0);
    }

    fn rti(&mut self, _mode: &AddressingMode) {
        self.status = self.stack_pop();
        self.clear_flag(StatusFlags::Break);
        self.set_flag(StatusFlags::Break2);

        self.pc = self.stack_pop_u16();
    }

    fn rts(&mut self, _mode: &AddressingMode) {
        self.pc = self.stack_pop_u16() + 1;
    }

    fn sbc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = !self.mem_read(addr) + 1; // -B = !B + 1
        let carry = if self.status & StatusFlags::Carry.bits() != 0 { 1 } else { 0 };
    
        let result = self.registers.a as u16 + value as u16 - ( 1 - carry ) as u16;
        self.set_flag_else_clear(StatusFlags::Carry, result > 0xFF);
        
        let result_u8 = result as u8;
        self.set_flag_else_clear(StatusFlags::Overflow, (self.registers.a ^ result_u8) & (value ^ result_u8) & 0x80 != 0);
    
        self.registers.a = result_u8;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, (self.registers.a & 0x80) != 0);
    }

    fn sta(&mut self, mode: &AddressingMode) {
        self.mem_write(self.get_operand_address(mode), self.registers.a);
    }

    fn stx(&mut self, mode: &AddressingMode) {
        self.mem_write(self.get_operand_address(mode), self.registers.x);
    }

    fn sty(&mut self, mode: &AddressingMode) {
        self.mem_write(self.get_operand_address(mode), self.registers.y);
    }

    fn tax(&mut self, _mode: &AddressingMode) {
        self.registers.x = self.registers.a;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.x & 0x80 != 0);
    }

    fn tay(&mut self, _mode: &AddressingMode) {
        self.registers.y = self.registers.a;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.y == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.y & 0x80 != 0);
    }

    fn tsx(&mut self, _mode: &AddressingMode) {
        self.registers.x = self.registers.s;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.x == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.x & 0x80 != 0);
    }

    fn txa(&mut self, _mode: &AddressingMode) {
        self.registers.a = self.registers.x;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.a & 0x80 != 0);
    }

    fn txs(&mut self, _mode: &AddressingMode) {
        self.registers.s = self.registers.x;
    }

    fn tya(&mut self, _mode: &AddressingMode) {
        self.registers.a = self.registers.y;
        self.set_flag_else_clear(StatusFlags::Zero, self.registers.a == 0);
        self.set_flag_else_clear(StatusFlags::Negative, self.registers.a & 0x80 != 0);
    }
}