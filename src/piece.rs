use crate::block::Block;
use crate::grid::{FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS};
use macroquad::prelude::Color;
use std::fmt::Display;

/// Fixed-size block canvas - avoids heap allocation
pub type BlockCanvas = [[Option<Block>; 5]; 5];

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
    /// Get blocks as a fixed-size array (no heap allocation).
    /// Returns the block canvas along with the actual bounds to iterate over.
    pub fn get_blocks(&self, orientation: usize) -> (BlockCanvas, usize, usize) {
        let orientation_def = self.orientations[orientation % 4];
        let mut canvas: BlockCanvas = [[None; 5]; 5];

        for row in 0..self.bounds_height {
            for col in 0..self.bounds_width {
                canvas[row][col] = match orientation_def.blocks[row][col] {
                    0 => None,
                    _ => Some(Block::new(self.color)),
                };
            }
        }

        (canvas, self.bounds_height, self.bounds_width)
    }

    /// Get the trimmed bounds for drawing (used for previews/held piece)
    pub fn get_trimmed_bounds(&self, orientation: usize) -> (usize, usize, usize, usize) {
        let orientation_def = self.orientations[orientation % 4];
        (
            orientation_def.bounds_y.0,
            orientation_def.bounds_y.1,
            orientation_def.bounds_x.0,
            orientation_def.bounds_x.1,
        )
    }

    pub fn get_initial_col(&self) -> isize {
        ((GRID_COUNT_COLS - self.bounds_width) / 2) as isize
    }

    /// Keep the bottom occupied spawn row at the top of the visible well.
    /// The I's larger canvas needs a different origin, not different SRS data.
    pub fn get_initial_row(&self) -> isize {
        FIRST_VISIBLE_ROW_ID as isize - (self.orientations[0].bounds_y.1 as isize - 1)
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

    // Restricted, earthy arena-shooter palette. The hues remain instantly
    // distinguishable, but every colour feels like it came from the same 1997
    // software renderer instead of a modern emissive display. The three
    // darkest hues (J, T, Z) are lifted a little so they do not sink into the
    // well's shadowed back wall.
    pub const PIECE_COLOR_I: Color = color_u8!(65, 151, 153, 255);
    pub const PIECE_COLOR_J: Color = color_u8!(80, 102, 150, 255);
    pub const PIECE_COLOR_L: Color = color_u8!(196, 99, 38, 255);
    pub const PIECE_COLOR_O: Color = color_u8!(202, 160, 48, 255);
    pub const PIECE_COLOR_S: Color = color_u8!(89, 132, 62, 255);
    pub const PIECE_COLOR_T: Color = color_u8!(140, 76, 142, 255);
    pub const PIECE_COLOR_Z: Color = color_u8!(176, 52, 40, 255);

    pub const I: Piece = Piece {
        name: "I",
        color: PIECE_COLOR_I,
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
        color: PIECE_COLOR_J,
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
        color: PIECE_COLOR_L,
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
        color: PIECE_COLOR_O,
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
        color: PIECE_COLOR_S,
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
        color: PIECE_COLOR_T,
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
        color: PIECE_COLOR_Z,
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

#[cfg(test)]
mod tests {
    use super::{pieces, FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS};

    #[test]
    fn all_spawn_canvases_align_their_bottom_row_and_center_their_occupied_columns() {
        for piece in [
            pieces::I,
            pieces::J,
            pieces::L,
            pieces::O,
            pieces::S,
            pieces::T,
            pieces::Z,
        ] {
            let (canvas, _, _) = piece.get_blocks(0);
            let mut occupied = Vec::new();

            for (row, cells) in canvas.iter().enumerate() {
                for (col, block) in cells.iter().enumerate() {
                    if block.is_some() {
                        occupied.push((
                            piece.get_initial_row() + row as isize,
                            piece.get_initial_col() + col as isize,
                        ));
                    }
                }
            }

            let first_visible = FIRST_VISIBLE_ROW_ID as isize;
            let top = if piece.name == "I" {
                first_visible
            } else {
                first_visible - 1
            };
            let width = match piece.name {
                "I" => 4,
                "O" => 2,
                _ => 3,
            };
            let left = ((GRID_COUNT_COLS - width) / 2) as isize;

            assert_eq!(occupied.len(), 4, "{}", piece.name);
            assert_eq!(occupied.iter().map(|cell| cell.0).min(), Some(top));
            assert_eq!(
                occupied.iter().map(|cell| cell.0).max(),
                Some(first_visible),
                "{} must not enter a row too low",
                piece.name
            );
            assert_eq!(occupied.iter().map(|cell| cell.1).min(), Some(left));
            assert_eq!(
                occupied.iter().map(|cell| cell.1).max(),
                Some(left + width as isize - 1)
            );
        }
    }
}
