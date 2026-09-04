use crate::block::Block;
use crate::piece::BlockCanvas;
use macroquad::prelude::Color;

pub const VISIBLE_GRID_COUNT_ROWS: usize = 20;
pub const HIDDEN_GRID_COUNT_ROWS: usize = 20;
pub const GRID_COUNT_ROWS: usize = VISIBLE_GRID_COUNT_ROWS + HIDDEN_GRID_COUNT_ROWS;
pub const FIRST_VISIBLE_ROW_ID: usize = GRID_COUNT_ROWS - VISIBLE_GRID_COUNT_ROWS;
pub const GRID_COUNT_COLS: usize = 10;
pub const MAX_CLEARED_CELLS: usize = 40;

#[derive(Copy, Clone, Debug)]
pub struct ClearedBlock {
    pub visible_row: usize,
    pub col: usize,
    pub color: Color,
}

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
            let has_collision = self.collision_check(
                next_row_offset,
                col_offset,
                canvas,
                bounds_height,
                bounds_width,
            );

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

    /// Spin corners treat the outer storage boundary as occupied. Hidden rows
    /// inside that boundary are normal cells, not a wall at the visible skyline.
    pub fn occupied_or_outside(&self, row: isize, col: isize) -> bool {
        if row < 0 || col < 0 || row >= GRID_COUNT_ROWS as isize || col >= GRID_COUNT_COLS as isize
        {
            return true;
        }

        self.has_block_at_cell(row as usize, col as usize)
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

    /// Clears all filled rows (if any), returning the number of rows cleared along
    /// with an array of cleared visible block details for particle and audio effects.
    pub fn clear_all_filled_rows_detailed(
        &mut self,
    ) -> (usize, [Option<ClearedBlock>; MAX_CLEARED_CELLS], usize) {
        let mut cleared_row_ids: Vec<usize> = Vec::new();
        let mut cleared_blocks = [None; MAX_CLEARED_CELLS];
        let mut cleared_count = 0;

        for row_id in 0..GRID_COUNT_ROWS {
            if self.is_row_filled(row_id) {
                if row_id >= FIRST_VISIBLE_ROW_ID {
                    let visible_row = row_id - FIRST_VISIBLE_ROW_ID;

                    for col_id in 0..GRID_COUNT_COLS {
                        if let Some(block) = self.get_cell(row_id, col_id) {
                            if cleared_count < MAX_CLEARED_CELLS {
                                cleared_blocks[cleared_count] = Some(ClearedBlock {
                                    visible_row,
                                    col: col_id,
                                    color: block.color,
                                });
                                cleared_count += 1;
                            }
                        }
                    }
                }

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

        (cleared_row_count, cleared_blocks, cleared_count)
    }

    /// Clears all filled rows (if any), returning the number of rows cleared.
    #[allow(dead_code)]
    pub fn clear_all_filled_rows(&mut self) -> usize {
        let (count, _, _) = self.clear_all_filled_rows_detailed();
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::RED;

    #[test]
    fn spin_occupancy_counts_storage_boundaries_but_not_the_visible_skyline() {
        let mut grid = Grid::new();

        for (row, col) in [
            (-1, 4),
            (GRID_COUNT_ROWS as isize, 4),
            (FIRST_VISIBLE_ROW_ID as isize, -1),
            (FIRST_VISIBLE_ROW_ID as isize, GRID_COUNT_COLS as isize),
        ] {
            assert!(grid.occupied_or_outside(row, col));
        }

        for row in [
            0,
            FIRST_VISIBLE_ROW_ID - 1,
            FIRST_VISIBLE_ROW_ID,
            GRID_COUNT_ROWS - 1,
        ] {
            assert!(!grid.occupied_or_outside(row as isize, 4));
        }

        grid.set_cell(FIRST_VISIBLE_ROW_ID - 1, 4, Some(Block::new(RED)));

        assert!(grid.occupied_or_outside(FIRST_VISIBLE_ROW_ID as isize - 1, 4));
    }

    #[test]
    fn clear_all_filled_rows_detailed_reports_cleared_cells() {
        let mut grid = Grid::new();
        let block = Some(Block::new(RED));

        // Fill the bottom-most visible row.
        for col in 0..GRID_COUNT_COLS {
            grid.set_cell(GRID_COUNT_ROWS - 1, col, block);
        }

        let (cleared_rows, cleared_blocks, cleared_count) = grid.clear_all_filled_rows_detailed();
        assert_eq!(cleared_rows, 1);
        assert_eq!(cleared_count, GRID_COUNT_COLS);

        for (col, cleared) in cleared_blocks[..cleared_count].iter().enumerate() {
            let cleared = cleared.expect("Expected cleared block");
            assert_eq!(cleared.visible_row, VISIBLE_GRID_COUNT_ROWS - 1);
            assert_eq!(cleared.col, col);
            assert_eq!(cleared.color, RED);
        }

        assert_eq!(grid.clear_all_filled_rows(), 0);
    }

    #[test]
    fn clearing_the_first_visible_row_pulls_hidden_blocks_into_view() {
        let mut grid = Grid::new();
        let block = Some(Block::new(RED));
        grid.set_cell(0, 1, block);
        grid.set_cell(FIRST_VISIBLE_ROW_ID - 1, 0, block);

        for col in 0..GRID_COUNT_COLS {
            grid.set_cell(FIRST_VISIBLE_ROW_ID, col, block);
        }

        let (cleared_rows, cleared_blocks, cleared_count) = grid.clear_all_filled_rows_detailed();

        assert_eq!(cleared_rows, 1);
        assert_eq!(cleared_count, GRID_COUNT_COLS);
        assert!(cleared_blocks[..cleared_count]
            .iter()
            .flatten()
            .all(|cleared| cleared.visible_row == 0));
        assert!(grid.has_block_at_cell(FIRST_VISIBLE_ROW_ID, 0));
        assert!(!grid.has_block_at_cell(FIRST_VISIBLE_ROW_ID - 1, 0));
        assert!(grid.has_block_at_cell(1, 1));
        assert!(!grid.has_block_at_cell(0, 1));
    }

    #[test]
    fn clearing_a_hidden_row_does_not_report_visible_debris_or_move_rows_below_it() {
        let mut grid = Grid::new();
        let block = Some(Block::new(RED));
        grid.set_cell(FIRST_VISIBLE_ROW_ID - 2, 2, block);
        grid.set_cell(FIRST_VISIBLE_ROW_ID, 4, block);

        for col in 0..GRID_COUNT_COLS {
            grid.set_cell(FIRST_VISIBLE_ROW_ID - 1, col, block);
        }

        let (cleared_rows, cleared_blocks, cleared_count) = grid.clear_all_filled_rows_detailed();

        assert_eq!(cleared_rows, 1);
        assert_eq!(cleared_count, 0);
        assert!(cleared_blocks.iter().all(Option::is_none));
        assert!(grid.has_block_at_cell(FIRST_VISIBLE_ROW_ID - 1, 2));
        assert!(!grid.has_block_at_cell(FIRST_VISIBLE_ROW_ID - 2, 2));
        assert!(grid.has_block_at_cell(FIRST_VISIBLE_ROW_ID, 4));
    }
}
