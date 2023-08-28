use macroquad::prelude::Color;

#[derive(Copy, Clone, Debug)]
pub struct Block {
    pub color: Color,
    pub is_locked: bool,
}

impl Block {
    pub fn new(color: Color, is_locked: bool) -> Self {
        Block { color, is_locked }
    }
}
