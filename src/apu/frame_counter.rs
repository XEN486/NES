#[derive(Copy, Clone)]
enum Mode {
    Zero,
    One,
}

pub enum FrameCounterResult {
    None,
    Quarter,
    Half,
}

pub struct FrameCounter {
    pub counter: isize,
    pub irq_enabled: bool,
    pub pub_irq_flag: bool,
    pub prv_irq_flag: bool,
    mode: Mode,
}

impl FrameCounter {
    pub fn new() -> FrameCounter {
        FrameCounter {
            counter: 0,
            irq_enabled: true,
            pub_irq_flag: false,
            prv_irq_flag: false,
            mode: Mode::Zero,
        }
    }

    pub fn write(&mut self, value: u8, cycles: usize) -> FrameCounterResult {
        self.irq_enabled = value & 0b0100_0000 == 0;

        // disable irq flags
        if !self.irq_enabled {
            self.pub_irq_flag = false;
            self.prv_irq_flag = false;
        }

        if value & 0b1000_0000 == 0 {
            self.mode = Mode::Zero;
        } else {
            self.mode = Mode::One;
        }

        self.counter = if cycles & 1 == 0 { 0 } else { -1 };

        match self.mode {
            Mode::Zero => FrameCounterResult::None,
            Mode::One => FrameCounterResult::Half,
        }
    }

    pub fn tick(&mut self) -> FrameCounterResult {
        match self.counter {
            7_459 => FrameCounterResult::Quarter,
            14_915 => FrameCounterResult::Half,
            22_373 => FrameCounterResult::Quarter,
            
            29_830 => {
                self.set_irq();
                FrameCounterResult::None
            }

            29_831 => {
                self.set_irq();
                self.pub_irq_flag = self.prv_irq_flag; // publish irq
                self.counter = 2; // reset counter to 2
                FrameCounterResult::None
            }

            _ => FrameCounterResult::None,
        }
    }

    pub fn tick_mode_one(&mut self) -> FrameCounterResult {
        match self.counter {
            7_459 => FrameCounterResult::Quarter,
            14_915 => FrameCounterResult::Half,
            22_373 => FrameCounterResult::Quarter,

            37_283 => {
                self.counter = 1; // roll over and also add 1 for the half signal
                FrameCounterResult::Half
            }

            _ => FrameCounterResult::None,
        }
    }

    pub fn set_irq(&mut self) {
        // only set irq if irqs are enabled
        if self.irq_enabled {
            self.prv_irq_flag = true;
        }
    }
}