pub const PERIODS: [u8; 16] = [
    214, 190, 170, 160, 143, 127, 113, 107, 95, 80, 71, 64, 53, 42, 36, 27
];

pub struct DMCChannel {
    prg_rom: Vec<u8>,
    pub irq_enabled: bool,
    pub irq_flag: bool,
    enabled: bool,
    output: u8,
    sample_address: u16,
    sample_length: u16,
    current_address: u16,
    current_length: u16,
    shift_register: u8,
    bit_count: u8,
    period: u8,
    counter: u8,
    looping: bool,
    cpu_stall_cycles: u8,
}

impl DMCChannel {
    pub fn new(prg_rom: Vec<u8>) -> DMCChannel {
        DMCChannel {
            prg_rom: prg_rom,
            irq_enabled: false,
            irq_flag: false,
            enabled: false,
            output: 0,
            sample_address: 0,
            sample_length: 0,
            current_address: 0,
            current_length: 0,
            shift_register: 0,
            bit_count: 0,
            period: 0,
            counter: 0,
            looping: false,
            cpu_stall_cycles: 0,
        }
    }

    pub fn reset_stall_cycles(&mut self) -> u8 {
        let c = self.cpu_stall_cycles;
        self.cpu_stall_cycles = 0;
        c
    }

    pub fn sample(&self) -> u8 {
        self.output
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x4010 => {
                self.irq_enabled = data & 0b1000_0000 != 0;
                self.irq_flag &= self.irq_enabled;
                self.looping = data & 0b1000_0000 != 0;
                self.period = PERIODS[data as usize & 0x0F]
            }
            
            0x4011 => self.output = data & 0b0111_1111,
            0x4012 => self.sample_address = 0xC000 + (data as u16 * 64),
            0x4013 => self.sample_length = 1 + (data as u16 * 16),

            _ => println!("[APU] unknown dmc channel address"),
        }
    }

    pub fn set_enabled(&mut self, value: bool) {
        self.irq_flag = false;
        self.enabled = value;

        if !self.enabled {
            self.current_length = 0;
            return;
        }

        if self.current_length == 0 {
            self.restart();
        }
    }

    pub fn restart(&mut self) {
        self.current_address = self.sample_address;
        self.current_length = self.sample_length;
    }

    pub fn tick_sequencer(&mut self) {
        if self.enabled {
            self.tick_read();
            self.tick_shift();
        }
    }

    fn tick_read(&mut self) {
        if self.current_length <= 0 || self.bit_count != 0 {
            return;
        }

        self.cpu_stall_cycles = 4;
        let a: u16 = self.current_address;

        self.shift_register = self.prg_rom[a as usize];
        self.bit_count = 8;
        self.current_address = self.current_address.wrapping_add(1);

        if self.current_address == 0 {
            self.current_address = 0x8000;
        }

        self.current_length -= 1;
        if self.current_length == 0 && self.looping {
            self.restart();
            return;
        }

        if self.current_length == 0 && self.irq_enabled {
            self.irq_flag = true;
        }
    }

    fn tick_shift(&mut self) {
        if self.counter != 0 {
            self.counter -= 1;
            return;
        }

        self.counter = self.period - 1;
        if self.bit_count == 0 {
            return;
        }

        if self.shift_register & 1 == 1 {
            if self.output <= 125 {
                self.output += 2;
            }
        } else {
            if self.output >= 2 {
                self.output -= 2;
            }
        }

        self.shift_register >>= 1;
        self.bit_count -= 1;
    }

    pub fn playing(&self) -> bool {
        self.current_length > 0
    }
}