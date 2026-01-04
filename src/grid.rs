use crate::block::Block;
use crate::piece::BlockCanvas;

pub const VISIBLE_GRID_COUNT_ROWS: usize = 20;
pub const GRID_COUNT_ROWS: usize = 22;
pub const FIRST_VISIBLE_ROW_ID: usize = GRID_COUNT_ROWS - VISIBLE_GRID_COUNT_ROWS;
pub const GRID_COUNT_COLS: usize = 10;

#[derive(Clone, Copy, Debug)]
pub struct Grid {
    rows: [[Option<Block>; GRID_COUNT_COLS]; GRID_COUNT_ROWS],
}

impl Grid {
    pub fn new() -> Self {
        let rows: [[Option<Block>; GRID_COUNT_COLS]; GRID_COUNT_ROWS] =
            std::array::from_fn(|_| std::array::from_fn(|_| None));

        Self { rows }
    }

    pub fn clear(&mut self) -> &mut Self {
        for row_id in 0..GRID_COUNT_ROWS {
            for col_id in 0..GRID_COUNT_COLS {
                self.rows[row_id][col_id] = None
            }
        }

        self
    }

    pub fn set_cell(&mut self, row_id: usize, col_id: usize, value: Option<Block>) -> &mut Self {
        if row_id >= GRID_COUNT_ROWS || col_id >= GRID_COUNT_COLS {
            return self;
        }

        self.rows[row_id][col_id] = value;
        self
    }

    pub fn set_cells(
        &mut self,
        row_offset: isize,
        col_offset: isize,
        canvas: &BlockCanvas,
        bounds_height: usize,
        bounds_width: usize,
    ) -> &mut Self {
        for canvas_row_id in 0..bounds_height {
            for canvas_col_id in 0..bounds_width {
                let canvas_cell = canvas[canvas_row_id][canvas_col_id];
                if canvas_cell.is_some() {
                    let grid_col_id = canvas_col_id as isize + col_offset;
                    let grid_row_id = canvas_row_id as isize + row_offset;

                    if grid_row_id < 0
                        || grid_col_id < 0
                        || grid_row_id >= GRID_COUNT_ROWS as isize
                        || grid_col_id >= GRID_COUNT_COLS as isize
                    {
                        panic!(
                            "Attempt to set cell that is out of bounds: ({}, {})",
                            grid_row_id, grid_col_id
                        );
                    }

                    self.set_cell(grid_row_id as usize, grid_col_id as usize, canvas_cell);
                }
            }
        }

        self
    }

    pub fn collision_check(
        &self,
        row_offset: isize,
        col_offset: isize,
        canvas: &BlockCanvas,
        bounds_height: usize,
        bounds_width: usize,
    ) -> bool {
        for canvas_row_id in 0..bounds_height {
            for canvas_col_id in 0..bounds_width {
                if canvas[canvas_row_id][canvas_col_id].is_some() {
                    let grid_col_id = canvas_col_id as isize + col_offset;
                    let grid_row_id = canvas_row_id as isize + row_offset;

                    if (grid_row_id < 0
                        || grid_col_id < 0
                        || grid_row_id >= GRID_COUNT_ROWS as isize
                        || grid_col_id >= GRID_COUNT_COLS as isize)
                        || self.has_block_at_cell(grid_row_id as usize, grid_col_id as usize)
                    {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if the given canvas, positioned at the given row_offset, would be entirely outside of
    /// the visible bounds of the playfield.
    pub fn invisible_check(
        &self,
        row_offset: isize,
        canvas: &BlockCanvas,
        bounds_height: usize,
        bounds_width: usize,
    ) -> bool {
        for canvas_row_id in 0..bounds_height {
            for canvas_col_id in 0..bounds_width {
                if canvas[canvas_row_id][canvas_col_id].is_some() {
                    let grid_row_id = canvas_row_id as isize + row_offset;

                    // If any row ID would be at or below the first visible row, then this canvas would not be
                    // entirely invisible, and we can return false.
                    if grid_row_id >= FIRST_VISIBLE_ROW_ID as isize {
                        return false;
                    }
                }
            }
        }

        true
    }

    pub fn find_landing_row(
        &self,
        row_offset: isize,
        col_offset: isize,
        canvas: &BlockCanvas,
        bounds_height: usize,
        bounds_width: usize,
    ) -> isize {
        for next_row_offset in row_offset..GRID_COUNT_ROWS as isize {
            let has_collision =
                self.collision_check(next_row_offset, col_offset, canvas, bounds_height, bounds_width);

            if has_collision {
                return next_row_offset - 1;
            }
        }

        GRID_COUNT_ROWS as isize
    }

    pub fn get_cell(&self, row_id: usize, col_id: usize) -> Option<Block> {
        if row_id >= GRID_COUNT_ROWS || col_id >= GRID_COUNT_COLS {
            return None;
        }

        self.rows[row_id][col_id]
    }

    pub fn has_block_at_cell(&self, row_id: usize, col_id: usize) -> bool {
        if row_id >= GRID_COUNT_ROWS || col_id >= GRID_COUNT_COLS {
            return false;
        }

        self.rows[row_id][col_id].is_some()
    }

    pub fn clear_row(&mut self, row_id: usize) -> &mut Self {
        if row_id >= GRID_COUNT_ROWS {
            return self;
        }

        self.rows[row_id].fill(None);

        self
    }

    pub fn is_row_filled(&self, row_id: usize) -> bool {
        if row_id >= GRID_COUNT_ROWS {
            return false;
        }

        for col_id in 0..GRID_COUNT_COLS {
            match self.get_cell(row_id, col_id) {
                Some(_block) => (),
                None => return false,
            }
        }

        true
    }

    // Clears all filled rows (if any), returning the number of rows cleared.
    pub fn clear_all_filled_rows(&mut self) -> usize {
        let mut cleared_row_ids: Vec<usize> = Vec::new();

        for row_id in 0..GRID_COUNT_ROWS {
            if self.is_row_filled(row_id) {
                self.clear_row(row_id);
                cleared_row_ids.push(row_id);
            }
        }

        let cleared_row_count = cleared_row_ids.len();

        for row_id in cleared_row_ids {
            if row_id == 0 {
                continue;
            }

            // Bubble the cleared row up to the top of the grid. This has the effect of shifting down
            // all non-blank rows.
            for swap_row_id in (0..row_id).rev() {
                self.rows.swap(swap_row_id, swap_row_id + 1);
            }
        }

        cleared_row_count
    }
}
