use crate::apu::sweep::{Sweep, SweepNegationMode};
use crate::apu::sequencer::Sequencer;
use crate::apu::envelope::Envelope;
use crate::apu::length_counter::LengthCounter;

pub const PULSE_WAVEFORM: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];

pub struct PulseChannel {
    sweep: Sweep,
    envelope: Envelope,
    sequencer: Sequencer,
    length_counter: LengthCounter,
    duty_cycle: usize,
}

impl PulseChannel {
    pub fn new(sweep_neg_mode: SweepNegationMode) -> PulseChannel {
        PulseChannel {
            sweep: Sweep::new(sweep_neg_mode),
            envelope: Envelope::new(),
            sequencer: Sequencer::new(PULSE_WAVEFORM[0].len()),
            length_counter: LengthCounter::new(),
            duty_cycle: 0
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr % 4 {
            0 => {
                self.duty_cycle = data as usize >> 6;
                self.envelope.write(data);
                self.length_counter.set_halt(data & 0b0010_0000 != 0);
            }

            1 => self.sweep.write(data),
            2 => self.sequencer.set_period_lo(data),

            3 => {
                self.length_counter.write(data);
                self.sequencer.set_period_hi(data);
                self.envelope.start();
            }

            _ => println!("[APU] pulse channel address impossible"),
        }
    }

    pub fn sample(&self) -> u8 {
        if self.length_counter.active() && self.sequencer.period >= 8 {
            PULSE_WAVEFORM[self.duty_cycle][self.sequencer.get_step()] * self.envelope.get_volume()
        } else {
            0
        }
    }

    pub fn tick_quarter(&mut self) {
        self.envelope.tick();
    }

    pub fn tick_half(&mut self) {
        self.length_counter.tick();
        self.sweep.tick(&mut self.sequencer);
    }

    pub fn tick_sequencer(&mut self) {
        self.sequencer.tick(true);
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