pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl Frame {
    pub fn new(width: usize, height: usize) -> Frame {
        Frame {
            width,
            height,
            data: vec![0; width * height * 3],
        }
    }
    
    pub fn set_pixel(&mut self, x: usize, y: usize, rgb: (u8, u8, u8)) {
        let index = y * 3 * self.width + x * 3;
        if index + 2 < self.data.len() {
            self.data[index + 0] = rgb.0;
            self.data[index + 1] = rgb.1;
            self.data[index + 2] = rgb.2;
        }
    }
}