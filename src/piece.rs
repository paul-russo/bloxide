use macroquad::prelude::Color;

use crate::block::Block;

#[derive(Clone, Debug, Default)]
pub struct Piece {
    pub color: Color,
    pub bounds_width: usize,
    pub bounds_height: usize,
    pub blocks: [[u8; 5]; 5],
}

impl Piece {
    /// Get a new 2d vector of the Blocks contained in the Piece.
    pub fn get_blocks(&self) -> Vec<Vec<Option<Block>>> {
        let mut blocks_vec: Vec<Vec<Option<Block>>> = Vec::new();

        for canvas_row in 0..self.bounds_height {
            let mut blocks_vec_row: Vec<Option<Block>> = Vec::new();

            for canvas_col in 0..self.bounds_width {
                blocks_vec_row.push(match self.blocks[canvas_row][canvas_col] {
                    0 => None,
                    _ => Some(Block::new(self.color, false)),
                })
            }

            blocks_vec.push(blocks_vec_row);
        }

        blocks_vec
    }
}

pub mod pieces {
    use super::Piece;
    use macroquad::{color_u8, prelude::Color};

    pub const I: Piece = Piece {
        color: color_u8!(100, 196, 235, 255),
        bounds_width: 5,
        bounds_height: 5,
        blocks: [
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 1, 1, 1, 1],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
        ],
    };

    pub const J: Piece = Piece {
        color: color_u8!(92, 101, 168, 255),
        bounds_width: 3,
        bounds_height: 3,
        blocks: [
            [1, 0, 0, 0, 0],
            [1, 1, 1, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
        ],
    };

    pub const L: Piece = Piece {
        color: color_u8!(224, 127, 58, 255),
        bounds_width: 3,
        bounds_height: 3,
        blocks: [
            [0, 0, 1, 0, 0],
            [1, 1, 1, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
        ],
    };

    pub const O: Piece = Piece {
        color: color_u8!(241, 212, 72, 255),
        bounds_width: 3,
        bounds_height: 3,
        blocks: [
            [0, 1, 1, 0, 0],
            [0, 1, 1, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
        ],
    };

    pub const S: Piece = Piece {
        color: color_u8!(100, 180, 82, 255),
        bounds_width: 3,
        bounds_height: 3,
        blocks: [
            [0, 1, 1, 0, 0],
            [1, 1, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
        ],
    };

    pub const T: Piece = Piece {
        color: color_u8!(140, 26, 245, 255),
        bounds_width: 3,
        bounds_height: 3,
        blocks: [
            [0, 1, 0, 0, 0],
            [1, 1, 1, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
        ],
    };

    pub const Z: Piece = Piece {
        color: color_u8!(234, 51, 35, 255),
        bounds_width: 3,
        bounds_height: 3,
        blocks: [
            [1, 1, 0, 0, 0],
            [0, 1, 1, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
        ],
    };
}
