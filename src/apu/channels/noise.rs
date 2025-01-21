use crate::apu::envelope::Envelope;
use crate::apu::length_counter::LengthCounter;

const PERIODS: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

pub struct NoiseChannel {
    envelope: Envelope,
    length_counter: LengthCounter,
    mode: bool,
    period: u16,
    counter: u16,
    shift: u16,
}

impl NoiseChannel {
    pub fn new() -> NoiseChannel {
        NoiseChannel {
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
            mode: false,
            period: 0,
            counter: 0,
            shift: 1,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x400C => {
                self.length_counter.set_halt(data & 0b0010_0000 != 0);
                self.envelope.write(data);
            }

            0x400D => {},

            0x400E => {
                self.mode = data & 0b1000_0000 != 0;
                self.period = PERIODS[data as usize & 0b1111];
            }

            0x400F => {
                self.length_counter.write(data);
                self.envelope.start();
            }

            _ => println!("[APU] unknown noise channel address"),
        }
    }

    pub fn sample(&self) -> u8 {
        if self.length_counter.active() && self.shift & 1 == 0 {
            self.envelope.get_volume()
        } else {
            0
        }
    }

    pub fn tick_sequencer(&mut self) {
        if self.counter > 0 {
            self.counter -= 1;
            return;
        }

        self.counter = self.period;
        let b1 = (self.shift >> (if self.mode { 6 } else { 1 })) & 1;
        let b2 = self.shift & 1;
        self.shift = (self.shift >> 1) | (b1 ^ b2) << 14;
    }

    pub fn tick_quarter(&mut self) {
        self.envelope.tick();
    }

    pub fn tick_half(&mut self) {
        self.length_counter.tick();
    }

    pub fn playing(&mut self) -> bool {
        self.length_counter.playing()
    }

    pub fn set_enabled(&mut self, value: bool) {
        self.length_counter.set_enabled(value);
    }

    pub fn update(&mut self) {
        self.length_counter.update();
    }
}