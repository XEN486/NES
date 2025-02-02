pub struct Frame {
    pub data: Vec<u8>,
}

impl Frame {
    pub fn new() -> Frame {
        Frame {
            data: vec![0; 256 * 240 * 3],
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, rgb: (u8, u8, u8)) {
        let index = y * 3 * 256 + x * 3;
        if index + 2 < self.data.len() {
            self.data[index + 0] = rgb.0;
            self.data[index + 1] = rgb.1;
            self.data[index + 2] = rgb.2;
        }
    }

    pub fn get_pixel(&mut self, x: usize, y: usize) -> (u8, u8, u8) {
        let index = y * 3 * 256 + x * 3;
        if index + 2 < self.data.len() {
            return (
                self.data[index + 0],
                self.data[index + 1],
                self.data[index + 2],
            );
        }
        return (0,0,0);
    }
}