#[allow(dead_code)]
pub struct Rect {
    pub x1: usize,
    pub y1: usize,
    pub x2: usize,
    pub y2: usize,
}

impl Rect {
    pub fn new(x1: usize, y1: usize, x2: usize, y2: usize) -> Rect {
        Rect {
            x1,
            y1,
            x2,
            y2,
        }
    }
}