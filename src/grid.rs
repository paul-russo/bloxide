use crate::block::Block;
use std::cell::Cell;

pub const GRID_COUNT_ROWS: usize = 20;
pub const GRID_COUNT_COLS: usize = 10;

#[derive(Debug)]
pub struct Grid {
    rows: [[Cell<Option<Block>>; GRID_COUNT_COLS]; GRID_COUNT_ROWS],
}

impl Grid {
    pub fn new() -> Self {
        let rows: [[Cell<Option<Block>>; GRID_COUNT_COLS]; GRID_COUNT_ROWS] =
            std::array::from_fn(|_| std::array::from_fn(|_| Cell::new(None)));

        Self { rows }
    }

    pub fn clear(&self) {
        for row in self.rows.iter() {
            for cell in row.iter() {
                cell.set(None);
            }
        }
    }

    pub fn set_cell(&self, row: usize, col: usize, value: Option<Block>) {
        if row >= GRID_COUNT_ROWS || col >= GRID_COUNT_COLS {
            return;
        }

        self.rows[row][col].set(value);
    }

    pub fn set_cells(&self, row: usize, col: usize, canvas: Vec<Vec<Option<Block>>>) {
        for (canvas_row_index, canvas_row) in canvas.iter().enumerate() {
            for (canvas_col_index, canvas_cell) in canvas_row.iter().enumerate() {
                self.set_cell(canvas_row_index + row, canvas_col_index + col, *canvas_cell);
            }
        }
    }

    pub fn get_cell(&self, row: usize, col: usize) -> Option<Block> {
        if row >= GRID_COUNT_ROWS || col >= GRID_COUNT_COLS {
            return None;
        }

        self.rows[row][col].get()
    }

    pub fn has_block_at_cell(&self, row: usize, col: usize) -> bool {
        if row >= GRID_COUNT_ROWS || col >= GRID_COUNT_COLS {
            return false;
        }

        self.rows[row][col].get().is_some()
    }

    pub fn clear_row(&self, row: usize) {
        if row >= GRID_COUNT_ROWS {
            return;
        }

        for cell in self.rows[row].iter() {
            cell.set(None);
        }
    }

    pub fn is_row_filled(&self, row: usize) -> bool {
        if row >= GRID_COUNT_ROWS {
            return false;
        }

        for cell in self.rows[row].iter() {
            match cell.get() {
                Some(block) => {
                    if !block.is_locked {
                        return false;
                    }
                }
                None => return false,
            }
        }

        true
    }

    // Clears all filled rows (if any), returning the number of rows cleared.
    pub fn clear_all_filled_rows(&self) -> usize {
        let mut cleared_row_count: usize = 0;

        for row in 0..GRID_COUNT_ROWS {
            if self.is_row_filled(row) {
                self.clear_row(row);
                cleared_row_count += 1;
            }
        }

        cleared_row_count
    }
}
