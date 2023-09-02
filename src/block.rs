use macroquad::prelude::Color;

#[derive(Copy, Clone, Debug)]
pub struct Block {
    pub color: Color,
}

impl Block {
    pub fn new(color: Color) -> Self {
        Block { color }
    }
}
