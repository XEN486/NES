use bitfield::bitfield;

bitfield! {
    pub struct EnvelopeControl(u8);
    pub constant_level,   _: 3, 0;
    pub decay_period,     _: 3, 0;
    pub constant,         _:    4;
    pub looping,          _:    5;
}

pub struct Envelope {
    control: EnvelopeControl,
    counter: u8,
    level: u8,
    start: bool,
}

impl Envelope {
    pub fn new() -> Envelope {
        Envelope {
            counter: 0,
            level: 0,
            control: EnvelopeControl(0),
            start: false,
        }
    }

    pub fn tick(&mut self) {
        if self.start {
            self.start = false;
            self.set_level(0x0f);
            return;
        }

        if self.counter > 0 {
            self.counter -= 1;
            return;
        }

        if self.level > 0 {
            self.set_level(self.level - 1);
            return;
        }

        if self.control.looping() {
            self.set_level(0x0f);
        }
    }

    fn set_level(&mut self, v: u8) {
        self.level = v & 0x0f;
        self.counter = self.control.decay_period();
    }

    pub fn write(&mut self, data: u8) {
        self.control = EnvelopeControl(data);
    }

    pub fn start(&mut self) {
        self.start = true;
    }

    pub fn get_volume(&self) -> u8 {
        if self.control.constant() {
            self.control.constant_level()
        } else {
            self.level
        }
    }
}