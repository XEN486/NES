import csv

def get_addressing_mode(mode):
    suffix = ''

    if mode == 'IMM':
        return suffix, '&AddressingMode::Immediate'
    elif mode == 'ZP':
        return suffix, '&AddressingMode::ZeroPage'
    elif mode == 'ZPX':
        return suffix, '&AddressingMode::ZeroPageX'
    elif mode == 'ZPY':
        return suffix, '&AddressingMode::ZeroPageY'
    elif mode == 'ABS':
        return suffix, '&AddressingMode::Absolute'
    elif mode == 'ABSX':
        return suffix, '&AddressingMode::AbsoluteX'
    elif mode == 'ABSY':
        return suffix, '&AddressingMode::AbsoluteY'
    elif mode == 'IND':
        return suffix, '&AddressingMode::Indirect'
    elif mode == 'INDX':
        return suffix, '&AddressingMode::IndirectX'
    elif mode == 'INDY':
        return suffix, '&AddressingMode::IndirectY'
    elif mode == 'IMP':
        return suffix, '&AddressingMode::Implied'
    elif mode == 'ACC':
        suffix = '_accumulator'
        return suffix, '&AddressingMode::Accumulator'
    elif mode == 'REL':
        return suffix, '&AddressingMode::Relative'

def generate_rust_trace(csv_file_path, output_file_path):
    with open(csv_file_path, 'r') as csv_file:
        reader = csv.DictReader(csv_file)
        instructions = []

        for row in reader:
            opcode = row['opcode']
            mnemonic = row['mnemonic']
            mode = row['addressing mode']
            byte_count = int(row['bytes'])

            byte_values = [f"self.mem_read(self.pc.wrapping_add({i}))" for i in range(byte_count)]
            addr_prefix = ''
            addr2_prefix = ''
            stored_prefix = ''
            jmp_addr = ''
            if mode == 'IMM':
                rust_mode = '&AddressingMode::Immediate'
                addr2_prefix = '_'
                stored_prefix = '_'
                operands = 'format!("#${:02X}", address)'
            elif mode == 'ZP':
                rust_mode = '&AddressingMode::ZeroPage'
                addr_prefix = '_'
                addr2_prefix = '_'
                operands = 'format!("${:02X} = {:02X}", mem_addr, stored_value)'
            elif mode == 'ZPX':
                rust_mode = '&AddressingMode::ZeroPageX'
                addr2_prefix = '_'
                operands = 'format!("${:02X},X @ {:02X} = {:02X}", address, mem_addr, stored_value)'
            elif mode == 'ZPY':
                rust_mode = '&AddressingMode::ZeroPageY'
                addr2_prefix = '_'
                operands = 'format!("${:02X},Y @ {:02X} = {:02X}", address, mem_addr, stored_value)'
            elif mode == 'ABS':
                rust_mode = '&AddressingMode::Absolute'
                addr_prefix = '_'
                addr2_prefix = '_'
                stored_prefix = '_'
                addr1_prefix = '_'
                operands = 'format!("${:04X}", mem_addr)'
            elif mode == 'ABSX':
                rust_mode = '&AddressingMode::AbsoluteX'
                addr2_prefix = '_'
                operands = 'format!("${:04X},X @ {:04X} = {:02X}", address, mem_addr, stored_value)'
            elif mode == 'ABSY':
                rust_mode = '&AddressingMode::AbsoluteY'
                addr2_prefix = '_'
                operands = 'format!("${:04X},Y @ {:04X} = {:02X}", address, mem_addr, stored_value)'
            elif mode == 'IND':
                rust_mode = '&AddressingMode::Indirect'
                jmp_addr = f'''let jmp_addr = if address & 0x00FF == 0x00FF {{
                    let lo = self.mem_read(address2);
                    let hi = self.mem_read(address2 & 0xFF00);
                    (hi as u16) << 8 | (lo as u16)
                }} else {{
                    self.mem_read_u16(address2)
                }};'''
                
                stored_prefix = '_'
                operands = 'format!("(${:04x}) = {:04x}", address, jmp_addr)'
            elif mode == 'INDX':
                rust_mode = '&AddressingMode::IndirectX'
                addr2_prefix = '_'
                operands = 'format!("(${:02X},X) @ {:02X} = {:04X} = {:02X}", address, (address.wrapping_add(self.registers.x)), mem_addr, stored_value)'
            elif mode == 'INDY':
                rust_mode = '&AddressingMode::IndirectY'
                addr2_prefix = '_'
                operands = 'format!("(${:02X},Y) @ {:02X} = {:04X} = {:02X}", address, (address.wrapping_add(self.registers.y)), mem_addr, stored_value)'
            elif mode == 'IMP':
                rust_mode = '&AddressingMode::Implied'
                addr_prefix = '_'
                addr2_prefix = '_'
                stored_prefix = '_'
                
                operands = '""'
            elif mode == 'ACC':
                rust_mode = '&AddressingMode::Accumulator'
                addr_prefix = '_'
                addr2_prefix = '_'
                stored_prefix = '_'
                
                operands = '"A "'
            elif mode == 'REL':
                rust_mode = '&AddressingMode::Relative'
                addr2_prefix = '_'
                stored_prefix = '_'
                operands = 'format!("${:04X}", (self.pc as usize + 2).wrapping_add((address as i8) as usize))'


            trace_template = f"""
            0x{opcode[2:]} => {{ // {mnemonic} {mode}
                let byte_values: Vec<u8> = vec![{', '.join(byte_values)}];

                let {addr_prefix}address: u8 = self.mem_read(self.pc.wrapping_add(1));
                
                self.pc = self.pc.wrapping_add(1);
                let mem_addr: u16 = self.get_operand_address({rust_mode});
                self.pc = self.pc.wrapping_sub(1);
                
                let {stored_prefix}stored_value: u8 = self.mem_read(mem_addr);
                let {addr2_prefix}address2: u16 = self.mem_read_u16(self.pc.wrapping_add(1));
                {jmp_addr}
                let instruction: String = format!("{mnemonic} {{}}", {operands});
                
                format!(
                    "{{:04X}}  {{:<9}} {{:<31}} A:{{:02X}} X:{{:02X}} Y:{{:02X}} P:{{:02X}} SP:{{:02X}}", 
                    self.pc,
                    byte_values.iter().map(|&b| format!("{{:02X}} ", b)).collect::<String>(), 
                    instruction, 
                    self.registers.a, 
                    self.registers.x, 
                    self.registers.y,
                    self.status,
                    self.registers.s,
                )
            }}"""

            instructions.append(trace_template)

    rust_code = f"""// This was generated by a dumb python script! Don't edit!! //

impl<'a> CPU<'a> {{
    #[allow(dead_code)]
    pub fn trace(&mut self) -> String {{
        let op = self.mem_read(self.pc);
        match op {{{"".join(instructions)}
            _ => String::from("unknown")
        }}
    }}
}}
    """

    with open(output_file_path, 'w') as rust_file:
        rust_file.write(rust_code)

def generate_rust_cycles(csv_file_path, output_file_path):
    with open(csv_file_path, 'r') as csv_file:
        reader = csv.DictReader(csv_file)
        instructions = []

        for row in reader:
            opcode = row['opcode']
            mnemonic = row['mnemonic']
            mode = row['addressing mode']
            cycles = row['cycles'].split('/')[0]
            page_cross = row['page cross']

            suffix, rust_mode = get_addressing_mode(mode)
            rust_pagecross = ''
            if page_cross == 'yes':
                rust_pagecross = f' + self.pagecross_penalty({rust_mode})'
            
            trace_template = f'            0x{opcode[2:]} => {cycles}{rust_pagecross}, // {mnemonic} {mode}\n'
            instructions.append(trace_template)

    rust_code = f"""// This was generated by a dumb python script! Don't edit!! //

impl<'a> CPU<'a> {{
    pub fn get_cycles_for_opcode(&mut self, op: u8) -> u8 {{
        match op {{
{"".join(instructions)}            _ => 2,
        }}
    }}
}}
    """

    with open(output_file_path, 'w') as rust_file:
        rust_file.write(rust_code)

def generate_rust_opmatch(csv_file_path, output_file_path):
    with open(csv_file_path, 'r') as csv_file:
        reader = csv.DictReader(csv_file)
        instructions = []

        for row in reader:
            opcode = row['opcode']
            mnemonic = row['mnemonic']
            mode = row['addressing mode']
            cycles = row['cycles'].split('/')[0]
            byte_count = int(row['bytes'])

            suffix, rust_mode = get_addressing_mode(mode)
            trace_template = f'''            0x{opcode[2:]} => {{ // {mnemonic} {mode}
                let result: bool = self.{mnemonic.lower()}{suffix}({rust_mode});
                self.pc = self.pc.wrapping_add({byte_count - 1});
                result
            }}

'''

            instructions.append(trace_template)

    rust_code = f"""// This was generated by a dumb python script! Don't edit!! //

impl<'a> CPU<'a> {{
    pub fn opmatch(&mut self, op: u8) -> bool {{
        match op {{
{"".join(instructions)}            _ => {{
                println!("[CPU] unimplemented opcode! 0x{{:02x}}", op);
                false
            }}
        }}
    }}
}}
    """

    with open(output_file_path, 'w') as rust_file:
        rust_file.write(rust_code)
        
generate_rust_trace('6502ops.csv', 'src/trace.rs')
generate_rust_cycles('6502ops.csv', 'src/cycles.rs')
generate_rust_opmatch('6502ops.csv', 'src/opmatch.rs')
