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

    pub fn apply_emphasis(&self, rgb: &mut (u8, u8, u8), emphasis: (bool, bool, bool)) {
        let (emphasize_red, emphasize_green, emphasize_blue) = emphasis;
        let mut r = rgb.0 as f32;
        let mut g = rgb.1 as f32;
        let mut b = rgb.2 as f32;

        if emphasize_red {
            r *= 1.2;
            g *= 0.9;
            b *= 0.9;
        }

        if emphasize_green {
            r *= 0.9;
            g *= 1.2;
            b *= 0.9;
        }

        if emphasize_blue {
            r *= 0.9;
            g *= 0.9;
            b *= 1.2;
        }

        *rgb = (
            r.clamp(0.0, 255.0) as u8,
            g.clamp(0.0, 255.0) as u8,
            b.clamp(0.0, 255.0) as u8,
        );
    }

    pub fn apply_greyscale(&self, rgb: &mut (u8, u8, u8), greyscale: bool) {
        if !greyscale {
            return;
        }

        let luminance = (rgb.0 as u16 * 30 + rgb.1 as u16 * 59 + rgb.2 as u16 * 11) / 100;
        *rgb = (luminance as u8, luminance as u8, luminance as u8);
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