use crate::block::Block;
use crate::grid::GRID_COUNT_COLS;
use macroquad::prelude::Color;
use std::fmt::Display;

#[derive(Copy, Clone, Debug, Default)]
pub struct OrientationDef {
    pub blocks: [[u8; 5]; 5],
    pub offsets: [(i8, i8); 5],
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Piece {
    pub name: &'static str,
    pub color: Color,
    pub bounds_width: usize,
    pub bounds_height: usize,
    pub orientations: [OrientationDef; 4],
}

impl Piece {
    /// Get a new 2d vector of the Blocks contained in the Piece.
    pub fn get_blocks(&self, orientation: usize) -> Vec<Vec<Option<Block>>> {
        let mut blocks_vec: Vec<Vec<Option<Block>>> = Vec::new();

        for canvas_row in 0..self.bounds_height {
            let mut blocks_vec_row: Vec<Option<Block>> = Vec::new();

            for canvas_col in 0..self.bounds_width {
                blocks_vec_row.push(
                    match self.orientations[orientation % 4].blocks[canvas_row][canvas_col] {
                        0 => None,
                        _ => Some(Block::new(self.color)),
                    },
                )
            }

            blocks_vec.push(blocks_vec_row);
        }

        blocks_vec
    }

    pub fn get_initial_col(&self) -> isize {
        ((GRID_COUNT_COLS - self.bounds_width) / 2) as isize
    }
}

impl Display for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub mod pieces {
    use super::{OrientationDef, Piece};
    use macroquad::{color_u8, prelude::Color};

    pub const I: Piece = Piece {
        name: "I",
        color: color_u8!(100, 196, 235, 255),
        bounds_width: 5,
        bounds_height: 5,
        orientations: [
            OrientationDef {
                blocks: [
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 1, 1, 1, 1],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (-1, 0), (2, 0), (-1, 0), (2, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 0, 0, 0],
                    [0, 0, 1, 0, 0],
                    [0, 0, 1, 0, 0],
                    [0, 0, 1, 0, 0],
                    [0, 0, 1, 0, 0],
                ],
                offsets: [(-1, 0), (0, 0), (0, 0), (0, 1), (0, -2)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [1, 1, 1, 1, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(-1, 1), (1, 1), (-2, 1), (1, 0), (-2, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 1, 0, 0],
                    [0, 0, 1, 0, 0],
                    [0, 0, 1, 0, 0],
                    [0, 0, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 1), (0, 1), (0, 1), (0, -1), (0, 2)],
            },
        ],
    };

    pub const J: Piece = Piece {
        name: "J",
        color: color_u8!(92, 101, 168, 255),
        bounds_width: 3,
        bounds_height: 3,
        orientations: [
            OrientationDef {
                blocks: [
                    [1, 0, 0, 0, 0],
                    [1, 1, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 1, 1, 0, 0],
                    [0, 1, 0, 0, 0],
                    [0, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 0, 0, 0],
                    [1, 1, 1, 0, 0],
                    [0, 0, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 1, 0, 0, 0],
                    [0, 1, 0, 0, 0],
                    [1, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
            },
        ],
    };

    pub const L: Piece = Piece {
        name: "L",
        color: color_u8!(224, 127, 58, 255),
        bounds_width: 3,
        bounds_height: 3,
        orientations: [
            OrientationDef {
                blocks: [
                    [0, 0, 1, 0, 0],
                    [1, 1, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 1, 0, 0, 0],
                    [0, 1, 0, 0, 0],
                    [0, 1, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 0, 0, 0],
                    [1, 1, 1, 0, 0],
                    [1, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [1, 1, 0, 0, 0],
                    [0, 1, 0, 0, 0],
                    [0, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
            },
        ],
    };

    pub const O: Piece = Piece {
        name: "O",
        color: color_u8!(241, 212, 72, 255),
        bounds_width: 3,
        bounds_height: 3,
        orientations: [
            OrientationDef {
                blocks: [
                    [0, 1, 1, 0, 0],
                    [0, 1, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 0, 0, 0],
                    [0, 1, 1, 0, 0],
                    [0, 1, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, -1), (0, -1), (0, -1), (0, -1), (0, -1)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 0, 0, 0],
                    [1, 1, 0, 0, 0],
                    [1, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(-1, -1), (-1, -1), (-1, -1), (-1, -1), (-1, -1)],
            },
            OrientationDef {
                blocks: [
                    [1, 1, 0, 0, 0],
                    [1, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(-1, 0), (-1, 0), (-1, 0), (-1, 0), (-1, 0)],
            },
        ],
    };

    pub const S: Piece = Piece {
        name: "S",
        color: color_u8!(100, 180, 82, 255),
        bounds_width: 3,
        bounds_height: 3,
        orientations: [
            OrientationDef {
                blocks: [
                    [0, 1, 1, 0, 0],
                    [1, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 1, 0, 0, 0],
                    [0, 1, 1, 0, 0],
                    [0, 0, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 0, 0, 0],
                    [0, 1, 1, 0, 0],
                    [1, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [1, 0, 0, 0, 0],
                    [1, 1, 0, 0, 0],
                    [0, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
            },
        ],
    };

    pub const T: Piece = Piece {
        name: "T",
        color: color_u8!(140, 26, 245, 255),
        bounds_width: 3,
        bounds_height: 3,
        orientations: [
            OrientationDef {
                blocks: [
                    [0, 1, 0, 0, 0],
                    [1, 1, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 1, 0, 0, 0],
                    [0, 1, 1, 0, 0],
                    [0, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 0, 0, 0],
                    [1, 1, 1, 0, 0],
                    [0, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 1, 0, 0, 0],
                    [1, 1, 0, 0, 0],
                    [0, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
            },
        ],
    };

    pub const Z: Piece = Piece {
        name: "Z",
        color: color_u8!(234, 51, 35, 255),
        bounds_width: 3,
        bounds_height: 3,
        orientations: [
            OrientationDef {
                blocks: [
                    [1, 1, 0, 0, 0],
                    [0, 1, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 1, 0, 0],
                    [0, 1, 1, 0, 0],
                    [0, 1, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
            },
            OrientationDef {
                blocks: [
                    [0, 0, 0, 0, 0],
                    [1, 1, 0, 0, 0],
                    [0, 1, 1, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
            },
            OrientationDef {
                blocks: [
                    [0, 1, 0, 0, 0],
                    [1, 1, 0, 0, 0],
                    [1, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                    [0, 0, 0, 0, 0],
                ],
                offsets: [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
            },
        ],
    };
}
