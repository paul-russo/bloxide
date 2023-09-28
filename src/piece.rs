use crate::block::Block;
use crate::grid::GRID_COUNT_COLS;
use macroquad::prelude::Color;
use std::fmt::Display;

#[derive(Copy, Clone, Debug, Default)]
pub struct OrientationDef {
    pub blocks: [[usize; 5]; 5],
    pub offsets: [(isize, isize); 5],
    pub bounds_x: (usize, usize),
    pub bounds_y: (usize, usize),
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
    pub fn get_blocks(&self, orientation: usize, trimmed: bool) -> Vec<Vec<Option<Block>>> {
        let orientation_def = self.orientations[orientation % 4];
        let mut blocks_vec: Vec<Vec<Option<Block>>> = Vec::new();

        let canvas_bounds_y = if trimmed {
            orientation_def.bounds_y
        } else {
            (0, self.bounds_height)
        };

        let canvas_bounds_x = if trimmed {
            orientation_def.bounds_x
        } else {
            (0, self.bounds_width)
        };

        for canvas_row_id in canvas_bounds_y.0..canvas_bounds_y.1 {
            let mut blocks_vec_row: Vec<Option<Block>> = Vec::new();

            for canvas_col_id in canvas_bounds_x.0..canvas_bounds_x.1 {
                blocks_vec_row.push(match orientation_def.blocks[canvas_row_id][canvas_col_id] {
                    0 => None,
                    _ => Some(Block::new(self.color)),
                })
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
                bounds_x: (1, 5),
                bounds_y: (2, 3),
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
                bounds_x: (2, 3),
                bounds_y: (1, 5),
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
                bounds_x: (0, 4),
                bounds_y: (2, 3),
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
                bounds_x: (2, 3),
                bounds_y: (0, 4),
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
                bounds_x: (0, 3),
                bounds_y: (0, 2),
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
                bounds_x: (1, 3),
                bounds_y: (0, 3),
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
                bounds_x: (0, 3),
                bounds_y: (1, 3),
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
                bounds_x: (0, 2),
                bounds_y: (0, 3),
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
                bounds_x: (0, 3),
                bounds_y: (0, 2),
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
                bounds_x: (1, 3),
                bounds_y: (0, 3),
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
                bounds_x: (0, 3),
                bounds_y: (1, 3),
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
                bounds_x: (0, 2),
                bounds_y: (0, 3),
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
                bounds_x: (1, 3),
                bounds_y: (0, 2),
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
                bounds_x: (1, 3),
                bounds_y: (1, 3),
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
                bounds_x: (0, 2),
                bounds_y: (1, 3),
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
                bounds_x: (0, 2),
                bounds_y: (0, 2),
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
                bounds_x: (0, 3),
                bounds_y: (0, 2),
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
                bounds_x: (1, 3),
                bounds_y: (0, 3),
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
                bounds_x: (0, 3),
                bounds_y: (1, 3),
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
                bounds_x: (0, 2),
                bounds_y: (0, 3),
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
                bounds_x: (0, 3),
                bounds_y: (0, 2),
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
                bounds_x: (1, 3),
                bounds_y: (0, 3),
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
                bounds_x: (0, 3),
                bounds_y: (1, 3),
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
                bounds_x: (0, 2),
                bounds_y: (0, 3),
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
                bounds_x: (0, 3),
                bounds_y: (0, 2),
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
                bounds_x: (1, 3),
                bounds_y: (0, 3),
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
                bounds_x: (0, 3),
                bounds_y: (1, 3),
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
                bounds_x: (0, 2),
                bounds_y: (0, 3),
                offsets: [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
            },
        ],
    };
}
