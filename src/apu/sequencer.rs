pub struct Sequencer {
    pub counter: u16,
    pub period: u16,
    pub step: usize,
    steps: usize,
}

impl Sequencer {
    pub fn new(steps: usize) -> Sequencer {
        Sequencer {
            counter: 0,
            period: 0,
            step: 0,
            steps,
        }
    }

    pub fn tick(&mut self, step_enabled: bool) -> bool {
        if self.counter == 0 {
            self.counter = self.period;
            if step_enabled {
                self.step = (self.step + 1) % self.steps;
            }
            true
        } else {
            self.counter -= 1;
            false
        }
    }

    pub fn set_period_lo(&mut self, value: u8) {
        self.period = (self.period & 0xFF00) | value as u16;
    }

    pub fn set_period_hi(&mut self, value: u8) {
        self.period = (self.period & 0x00FF) | ((value as u16 & 0b111) << 8);
    }

    pub fn get_step(&self) -> usize {
        self.step
    }
}