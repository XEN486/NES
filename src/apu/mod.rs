pub mod length_counter;
pub mod frame_counter;
pub mod sequencer;
pub mod sweep;
pub mod envelope;
pub mod filter;
pub mod channels;

use frame_counter::{FrameCounter, FrameCounterResult};
use channels::{pulse::PulseChannel, triangle::TriangleChannel, noise::NoiseChannel, dmc::DMCChannel};

use filter::FirstOrderFilter;
use sweep::SweepNegationMode;

use std::sync::Arc;
use std::sync::Mutex;

pub struct APU {
    pub buffer: Arc<Mutex<Vec<f32>>>,
    frame_counter: FrameCounter,
    pulse_0: PulseChannel,
    pulse_1: PulseChannel,
    triangle: TriangleChannel,
    noise: NoiseChannel,
    pub dmc: DMCChannel,
    filters: [FirstOrderFilter; 3],
}

impl APU {
    pub fn new(prg_rom: Vec<u8>) -> APU {
        APU {
            buffer: Arc::new(Mutex::new(Vec::new())),
            frame_counter: FrameCounter::new(),

            pulse_0: PulseChannel::new(SweepNegationMode::OnesCompliment),
            pulse_1: PulseChannel::new(SweepNegationMode::TwosCompliment),
            triangle: TriangleChannel::new(),
            noise: NoiseChannel::new(),
            dmc: DMCChannel::new(prg_rom),

            filters: [
                FirstOrderFilter::high_pass(44100.0, 90.0),
                FirstOrderFilter::high_pass(44100.0, 440.0),
                FirstOrderFilter::low_pass(44100.0, 14000.0),
            ],
        }
    }

    pub fn read(&mut self) -> u8 {
        let mut result = 0;
        if self.dmc.irq_flag {
            result |= 0b1000_0000;
        }
        if self.frame_counter.prv_irq_flag {
            result |= 0b0100_0000;
        }
        if self.dmc.playing() {
            result |= 0b0001_0000;
        }
        if self.noise.playing() {
            result |= 0b0000_1000;
        }
        if self.triangle.playing() {
            result |= 0b0000_0100;
        }
        if self.pulse_1.playing() {
            result |= 0b0000_0010;
        }
        if self.pulse_0.playing() {
            result |= 0b0000_0001;
        }

        self.frame_counter.prv_irq_flag = false;
        self.frame_counter.pub_irq_flag = false;
        result
    }

    pub fn write_to_pulse_0(&mut self, addr: u16, data: u8) {
        self.pulse_0.write(addr, data);
    }

    pub fn write_to_pulse_1(&mut self, addr: u16, data: u8) {
        self.pulse_1.write(addr, data);
    }

    pub fn write_to_triangle(&mut self, addr: u16, data: u8) {
        self.triangle.write(addr, data);
    }

    pub fn write_to_noise(&mut self, addr: u16, data: u8) {
        self.noise.write(addr, data);
    }

    pub fn write_to_dmc(&mut self, addr: u16, data: u8) {
        self.dmc.write(addr, data);
    }

    pub fn set_status(&mut self, data: u8) {
        self.pulse_0.set_enabled(data & 0b0000_0001 != 0);
        self.pulse_1.set_enabled(data & 0b0000_0010 != 0);
        self.triangle.set_enabled(data & 0b0000_0100 != 0);
        self.noise.set_enabled(data & 0b0000_1000 != 0);
        self.dmc.set_enabled(data & 0b001_0000 != 0);
    }

    pub fn set_frame_counter(&mut self, data: u8, cycles: usize) {
        let result: FrameCounterResult = self.frame_counter.write(data, cycles);
        self.update(result);
    }

    fn update(&mut self, result: FrameCounterResult) {
        match result {
            FrameCounterResult::Quarter => {
                self.pulse_0.tick_quarter();
                self.pulse_1.tick_quarter();
                self.triangle.tick_quarter();
            }
    
            FrameCounterResult::Half => {
                self.pulse_0.tick_quarter();
                self.pulse_1.tick_quarter();
                self.triangle.tick_quarter();
                self.noise.tick_quarter();
    
                self.pulse_0.tick_half();
                self.pulse_1.tick_half();
                self.triangle.tick_half();
                self.noise.tick_half();
            }
    
            FrameCounterResult::None => {}
        }
    }

    pub fn tick(&mut self, cycles: usize) {
        self.triangle.tick_sequencer();

        if cycles % 2 == 1 {
            self.pulse_0.tick_sequencer();
            self.pulse_1.tick_sequencer();
            self.noise.tick_sequencer();
            self.dmc.tick_sequencer();
        }

        let r = self.frame_counter.tick();
        self.update(r);

        self.pulse_0.update();
        self.pulse_1.update();
        self.triangle.update();
        self.noise.update();

        if cycles % 40 == 0 {
            let s = self.sample();
            self.buffer.lock().expect("Failed to get buffer").push(s);
        }
    }

    fn sample(&mut self) -> f32 {
        // sample each channel
        let p0 = self.pulse_0.sample() as f64;
        let p1 = self.pulse_1.sample() as f64;
        let t = self.triangle.sample() as f64;
        let n = self.noise.sample() as f64;
        let d = self.dmc.sample() as f64;
    
        // combine
        let pulse_out = 95.88 / ((8218.0 / (p0 + p1)) + 100.0);
        let tnd_out = 159.79 / ((1.0 / (t / 8227.0 + n / 12241.0 + d / 22638.0)) + 100.0);
    
        // combine pulse and tnd outputs
        let mut output = pulse_out + tnd_out;
    
        // apply filters
        for i in 0..self.filters.len() {
            output = self.filters[i].tick(output);
        }
    
        // return the normalized output as f32
        output as f32
    }
}