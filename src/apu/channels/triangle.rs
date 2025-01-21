use crate::apu::length_counter::LengthCounter;
use crate::apu::sequencer::Sequencer;

pub const TRIANGLE_WAVEFORM: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

pub struct TriangleChannel {
    length_counter: LengthCounter,
    sequencer: Sequencer,

    linear_counter_start: bool,
    linear_counter_period: u8,
    linear_counter: u8,

    control_flag: bool,
}

impl TriangleChannel {
    pub fn new() -> TriangleChannel {
        TriangleChannel {
            length_counter: LengthCounter::new(),
            sequencer: Sequencer::new(TRIANGLE_WAVEFORM.len()),

            linear_counter_start: false,
            linear_counter_period: 0,
            linear_counter: 0,

            control_flag: false,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x4008 => {
                self.control_flag = data & 0b1000_0000 != 0;
                self.length_counter.set_halt(data & 0b1000_0000 != 0);
                self.linear_counter_period = data & 0b0111_1111;
            }

            0x4009 => {}
            0x400A => self.sequencer.set_period_lo(data),

            0x400B => {
                self.length_counter.write(data);
                self.sequencer.set_period_hi(data & 0b111);
                self.linear_counter_start = true;
            }

            _ => println!("[APU] attempted to write to unknown triangle register"),
        }
    }

    pub fn sample(&self) -> u8 {
        if self.active() && self.sequencer.period > 2 {
            TRIANGLE_WAVEFORM[self.sequencer.get_step()]
        } else {
            0
        }
    }

    pub fn tick_sequencer(&mut self) {
        self.sequencer.tick(self.active());
    }

    pub fn tick_quarter(&mut self) {
        if !self.control_flag {
            self.linear_counter_start = false;
        }

        if self.linear_counter_start {
            self.linear_counter = self.linear_counter_period;
            return;
        }

        if self.linear_counter > 0 {
            self.linear_counter_start = false;
        }
    }

    pub fn tick_half(&mut self) {
        self.length_counter.tick();
    }

    fn active(&self) -> bool {
        self.length_counter.active() && self.linear_counter > 0
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