use crate::{
    bag_manager::BagManager,
    grid::{Grid, GRID_COUNT_COLS, GRID_COUNT_ROWS},
    high_score_manager::HighScoreManager,
    piece::{BlockCanvas, Piece},
};
use macroquad::prelude::Color;
use std::time::Instant;

const TICKS_PER_SECOND: f32 = 60.0;
const INITIAL_GRAVITY: f32 = 1.0 / 60.0; // 1/60G. 1 row per 60 ticks (1 second)
const G_SOFT_DROP: f32 = 30.0 / 60.0; // 1/2G. 30 rows per 60 ticks (1 second)
const REPEAT_DELAY_TICKS: isize = 11; // ~183ms. Delay before repeating horizontal movement.
const REPEAT_INTERVAL_TICKS: isize = 4; // ~67ms, or 15 times per second. Repeat interval for horizontal movement.
const LOCK_DELAY_TICKS: isize = 30; // 30 ticks, 500ms. Delay after which the active piece is locked in place.
const RESET_MOVES: isize = 15; // Number of shifts or rotations allowed before lock delay can no longer be reset.
const LOCK_IMPACT_TICKS: usize = 10;
const LINE_CLEAR_EFFECT_TICKS: usize = 40;
const MAX_IMPACT_CONTACTS: usize = 4;

/// Find every downward contact made by the active piece. Coordinates are in
/// grid space: columns use cell centres and rows identify the supporting block
/// (or the floor boundary at [`GRID_COUNT_ROWS`]). A tetromino can contribute at
/// most four independent contacts.
fn impact_contact_origins(
    grid_locked: &Grid,
    active_piece_row: isize,
    active_piece_col: isize,
    canvas: &BlockCanvas,
    bounds_height: usize,
    bounds_width: usize,
) -> ([(f32, f32); MAX_IMPACT_CONTACTS], usize) {
    let mut contacts = [(0.0, 0.0); MAX_IMPACT_CONTACTS];
    let mut contact_count = 0;
    let mut occupied_col_sum = 0.0;
    let mut occupied_count = 0.0;
    let mut lowest_occupied_row = active_piece_row;

    for canvas_row in 0..bounds_height {
        for canvas_col in 0..bounds_width {
            if canvas[canvas_row][canvas_col].is_none() {
                continue;
            }

            let grid_row = active_piece_row + canvas_row as isize;
            let grid_col = active_piece_col + canvas_col as isize;
            let below_row = grid_row + 1;
            let column_center = grid_col as f32 + 0.5;

            occupied_col_sum += column_center;
            occupied_count += 1.0;
            lowest_occupied_row = lowest_occupied_row.max(grid_row);

            let touches_floor = below_row >= GRID_COUNT_ROWS as isize;
            let touches_stack = below_row >= 0
                && grid_col >= 0
                && grid_col < GRID_COUNT_COLS as isize
                && grid_locked.has_block_at_cell(below_row as usize, grid_col as usize);

            if touches_floor || touches_stack {
                contacts[contact_count] = (column_center, below_row as f32);
                contact_count += 1;
            }
        }
    }

    if contact_count == 0 && occupied_count > 0.0 {
        // Locking should always have at least one downward contact, but retain a
        // geometrically sensible fallback for malformed or future piece rules.
        contacts[0] = (
            occupied_col_sum / occupied_count,
            lowest_occupied_row as f32 + 1.0,
        );
        contact_count = 1;
    }

    (contacts, contact_count)
}

#[derive(Debug, Default)]
pub struct GameInput {
    pub soft_drop: bool,
    pub shift_left: bool,
    pub shift_right: bool,
    pub rotate_right: bool,
    pub hard_drop: bool,
    pub hold_piece: bool,
    pub toggle_pause: bool,
}

#[derive(Clone, Copy, Debug)]
enum ShiftDirection {
    Left,
    Right,
    Neither,
}

#[derive(Copy, Clone)]
pub struct GameState<'a> {
    grid_locked: Grid,
    grid_active: Grid,
    grid_ghost: Grid,
    bag_manager: BagManager,
    active_piece: Piece,
    score: usize,
    tick: usize,
    last_tick: usize,
    start: Instant,
    active_piece_col: isize,
    active_piece_row: isize,
    active_piece_orientation: usize,
    ticks_to_next_row_inc: isize,
    ticks_to_repeat: isize,
    ticks_to_lock: isize,
    lock_reset_moves_remaining: isize,
    shift_direction: ShiftDirection,
    held_piece: Option<Piece>,
    last_piece_swapped: bool,
    rows_cleared: usize,
    is_game_over: bool,
    is_paused: bool,
    high_score_manager: &'a HighScoreManager,
    // Cached block canvas to avoid repeated allocations
    cached_blocks: BlockCanvas,
    cached_bounds_height: usize,
    cached_bounds_width: usize,
    // Dirty flag to track if piece state changed
    piece_dirty: bool,
    // Cached ghost row
    cached_ghost_row: isize,
    // Short-lived presentation cues consumed by the renderer. Keeping these in
    // the game state makes effects deterministic and ensures pause freezes them.
    impact_ticks_remaining: usize,
    clear_effect_ticks_remaining: usize,
    last_clear_count: usize,
    impact_origins: [(f32, f32); MAX_IMPACT_CONTACTS],
    impact_origin_count: usize,
    impact_color: Color,
}

impl<'a> GameState<'a> {
    pub fn new(high_score_manager: &'a HighScoreManager) -> Self {
        let grid_locked = Grid::new();
        let grid_active = Grid::new();
        let grid_ghost = Grid::new();
        let mut bag_manager = BagManager::new();
        let active_piece = bag_manager.next();
        let score: usize = 0;
        let tick: usize = 0;
        let last_tick: usize = 0;
        let active_piece_col = active_piece.get_initial_col();
        let active_piece_row = 1;
        let active_piece_orientation: usize = 0;
        let gravity: f32 = INITIAL_GRAVITY;
        let ticks_to_next_row_inc: isize = (1.0 / gravity).ceil() as isize;
        let start = Instant::now();

        // Initialize cached blocks
        let (cached_blocks, cached_bounds_height, cached_bounds_width) =
            active_piece.get_blocks(active_piece_orientation);

        Self {
            grid_locked,
            grid_active,
            grid_ghost,
            bag_manager,
            active_piece,
            score,
            tick,
            last_tick,
            start,
            active_piece_col,
            active_piece_row,
            active_piece_orientation,
            ticks_to_next_row_inc,
            ticks_to_repeat: REPEAT_DELAY_TICKS,
            ticks_to_lock: LOCK_DELAY_TICKS,
            lock_reset_moves_remaining: RESET_MOVES,
            shift_direction: ShiftDirection::Neither,
            held_piece: None,
            last_piece_swapped: false,
            rows_cleared: 0,
            is_game_over: false,
            is_paused: false,
            high_score_manager,
            cached_blocks,
            cached_bounds_height,
            cached_bounds_width,
            piece_dirty: true,
            cached_ghost_row: 0,
            impact_ticks_remaining: 0,
            clear_effect_ticks_remaining: 0,
            last_clear_count: 0,
            impact_origins: [(0.0, 0.0); MAX_IMPACT_CONTACTS],
            impact_origin_count: 0,
            impact_color: active_piece.color,
        }
    }

    pub fn clean_up(&mut self) {
        if self.is_game_over || self.is_paused {
            return;
        }

        self.last_tick = self.tick;
    }

    /// Returns the number of ticks elapsed between the last rendered tick and the current one.
    /// At 60fps or higher, this should be either 1 or 0. It may be more than 1 at lower frame rates.
    fn get_tick_delta(&self) -> usize {
        self.tick - self.last_tick
    }

    fn get_new_ticks_to_next_row_inc(&self) -> isize {
        (1.0 / self.get_gravity()).ceil() as isize
    }

    fn reset_piece_state(&mut self) {
        self.active_piece_orientation = 0;
        self.active_piece_col = self.active_piece.get_initial_col();
        self.active_piece_row = 1;
        self.ticks_to_next_row_inc = self.get_new_ticks_to_next_row_inc();
        self.last_piece_swapped = false;
        self.ticks_to_lock = LOCK_DELAY_TICKS;
        self.lock_reset_moves_remaining = RESET_MOVES;
        self.refresh_cached_blocks();
    }

    fn refresh_cached_blocks(&mut self) {
        let (blocks, height, width) = self.active_piece.get_blocks(self.active_piece_orientation);
        self.cached_blocks = blocks;
        self.cached_bounds_height = height;
        self.cached_bounds_width = width;
        self.piece_dirty = true;
    }

    fn try_reset_lock_delay_for_move(&mut self) {
        if self.lock_reset_moves_remaining > 0 {
            self.lock_reset_moves_remaining -= 1;
            self.ticks_to_lock = LOCK_DELAY_TICKS;
        }
    }

    fn end_game(&mut self) {
        self.clean_up();
        self.is_game_over = true;
        self.high_score_manager.add_score(self.score);
    }

    pub fn toggle_pause(&mut self) {
        if self.is_paused {
            self.tick = 0;
            self.last_tick = 0;
            self.start = Instant::now();
            self.is_paused = false;
        } else {
            self.is_paused = true;
        }
    }

    /// Check if the active piece, if it were locked, would be entirely outside the visible bounds of the playfield.
    fn check_for_lock_out(&self) -> bool {
        self.grid_locked.invisible_check(
            self.active_piece_row,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        )
    }

    fn set_active_piece_and_reset_state(&mut self, next_active_piece: Piece) {
        self.active_piece = next_active_piece;
        self.reset_piece_state();

        // Check for a piece spawned overlapping at least one block in the playfield (Block Out)
        let is_block_out = self.collide(None, None, None);

        if is_block_out {
            self.end_game();
        }
    }

    fn next_piece(&mut self) {
        let next_active_piece = self.bag_manager.next();
        self.set_active_piece_and_reset_state(next_active_piece);
    }

    fn swap_active_piece(&mut self) {
        // Return early if the last piece was already swapped, as you can only swap once before having
        // to land a piece.
        if self.last_piece_swapped {
            return;
        }

        if let Some(held_piece) = self.held_piece {
            self.held_piece = Some(self.active_piece);
            self.set_active_piece_and_reset_state(held_piece);
        } else {
            self.held_piece = Some(self.active_piece);
            self.next_piece();
        }

        self.last_piece_swapped = true;
    }

    fn lock_active_piece_and_get_next(&mut self) {
        if self.check_for_lock_out() {
            self.end_game();
            return;
        }

        let (contact_origins, contact_count) = impact_contact_origins(
            &self.grid_locked,
            self.active_piece_row,
            self.active_piece_col,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        );
        self.impact_origins = contact_origins;
        self.impact_origin_count = contact_count;
        self.impact_color = self.active_piece.color;

        self.grid_locked.set_cells(
            self.active_piece_row,
            self.active_piece_col,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        );

        self.impact_ticks_remaining = self.impact_ticks_remaining.max(LOCK_IMPACT_TICKS);

        self.clear_filled_rows_and_update_score();

        self.next_piece();
    }

    fn hard_drop(&mut self) {
        let landing_row = self.grid_locked.find_landing_row(
            self.active_piece_row,
            self.active_piece_col,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        );

        let lines_dropped = (landing_row - self.active_piece_row).max(0);
        self.active_piece_row = landing_row;

        // Longer drops land with a little more visual weight, while the cap
        // keeps the pulse brief enough not to distract from the next piece.
        self.impact_ticks_remaining =
            (LOCK_IMPACT_TICKS + lines_dropped as usize).min(LOCK_IMPACT_TICKS + 12);

        self.score += 2 * lines_dropped as usize;

        self.lock_active_piece_and_get_next();
    }

    fn get_next_active_piece_row(&self) -> isize {
        if self.ticks_to_next_row_inc <= 0 {
            self.active_piece_row + 1
        } else {
            self.active_piece_row
        }
    }

    fn collide(
        &self,
        row_id: Option<isize>,
        col_id: Option<isize>,
        orientation: Option<usize>,
    ) -> bool {
        let orientation = orientation.unwrap_or(self.active_piece_orientation);

        // Use cached blocks if orientation matches, otherwise compute fresh
        if orientation == self.active_piece_orientation {
            self.grid_locked.collision_check(
                row_id.unwrap_or(self.active_piece_row),
                col_id.unwrap_or(self.active_piece_col),
                &self.cached_blocks,
                self.cached_bounds_height,
                self.cached_bounds_width,
            )
        } else {
            let (blocks, height, width) = self.active_piece.get_blocks(orientation);
            self.grid_locked.collision_check(
                row_id.unwrap_or(self.active_piece_row),
                col_id.unwrap_or(self.active_piece_col),
                &blocks,
                height,
                width,
            )
        }
    }

    fn try_rotate_right(&mut self) {
        let next_orientation = (self.active_piece_orientation + 1) % 4;

        let offsets_a = self.active_piece.orientations[self.active_piece_orientation].offsets;
        let offsets_b = self.active_piece.orientations[next_orientation].offsets;

        for index in 0..offsets_a.len() {
            let offset_col = offsets_a[index].0 - offsets_b[index].0;
            let offset_row = offsets_a[index].1 - offsets_b[index].1;

            // I'm subtracting the offset from active_piece_row instead of adding it here,
            // because rows are counted from the bottom in the Guideline. They're counted
            // in the other direction in my Grid implementation, so the offsets I copied
            // from the Guideline have to get applied backwards here.
            let next_active_piece_row = self.active_piece_row - offset_row;
            let next_active_piece_col = self.active_piece_col + offset_col;

            let has_collision = self.collide(
                Some(next_active_piece_row),
                Some(next_active_piece_col),
                Some(next_orientation),
            );

            if !has_collision {
                self.active_piece_orientation = next_orientation;
                self.active_piece_row = next_active_piece_row;
                self.active_piece_col = next_active_piece_col;
                self.refresh_cached_blocks();
                self.try_reset_lock_delay_for_move();

                return;
            }
        }
    }

    fn set_shift_direction_and_reset_ticks(&mut self, new_shift_direction: ShiftDirection) {
        self.ticks_to_repeat = REPEAT_DELAY_TICKS;
        self.shift_direction = new_shift_direction;
    }

    fn try_move_horizontal(&mut self, is_shift_left: bool, is_shift_right: bool) {
        let tick_delta = self.get_tick_delta();
        let mut col_offset = 0;

        if !is_shift_left && !is_shift_right {
            return self.set_shift_direction_and_reset_ticks(ShiftDirection::Neither);
        }

        match self.shift_direction {
            ShiftDirection::Left => {
                if !is_shift_left {
                    if is_shift_right {
                        self.set_shift_direction_and_reset_ticks(ShiftDirection::Right);
                        col_offset = 1;
                    }
                } else {
                    self.ticks_to_repeat -= tick_delta as isize;
                }
            }

            ShiftDirection::Right => {
                if !is_shift_right {
                    if is_shift_left {
                        self.set_shift_direction_and_reset_ticks(ShiftDirection::Left);
                        col_offset = -1;
                    }
                } else {
                    self.ticks_to_repeat -= tick_delta as isize;
                }
            }

            ShiftDirection::Neither => {
                if is_shift_left {
                    self.set_shift_direction_and_reset_ticks(ShiftDirection::Left);
                    col_offset = -1;
                } else if is_shift_right {
                    self.set_shift_direction_and_reset_ticks(ShiftDirection::Right);
                    col_offset = 1;
                }
            }
        }

        if self.ticks_to_repeat <= 0 {
            col_offset = match self.shift_direction {
                ShiftDirection::Left => -1,
                ShiftDirection::Right => 1,
                ShiftDirection::Neither => unreachable!(),
            };

            self.ticks_to_repeat = REPEAT_INTERVAL_TICKS;
        }

        let next_active_piece_col = self.active_piece_col + col_offset;

        // Horizontal collision check
        if next_active_piece_col != self.active_piece_col {
            let has_collision = self.collide(None, Some(next_active_piece_col), None);

            if !has_collision {
                self.active_piece_col = next_active_piece_col;
                self.piece_dirty = true;
                self.try_reset_lock_delay_for_move();
            }
        }
    }

    fn set_active_piece_row_and_reset_ticks(&mut self, new_active_piece_row: isize) {
        self.active_piece_row = new_active_piece_row;
        self.ticks_to_next_row_inc = self.get_new_ticks_to_next_row_inc();
        self.lock_reset_moves_remaining = RESET_MOVES;
        self.ticks_to_lock = LOCK_DELAY_TICKS;
        self.piece_dirty = true;
    }

    fn try_gravity_drop(&mut self, is_soft_drop: bool) {
        let next_active_piece_row = self.get_next_active_piece_row();

        // Vertical collision check
        if next_active_piece_row != self.active_piece_row {
            let has_collision = self.collide(Some(next_active_piece_row), None, None);

            if has_collision {
                self.ticks_to_lock -= self.get_tick_delta() as isize;

                // TODO: Need to mitigate "stalling" with certain pieces that can floor kick, as the gravity drop
                // will reset the move counter and allow for infinite stalling.
                // Tetra Legends seems to store the row at which contact was last made, and only reset the move
                // counter when the piece descends below that row. This allows for spinning out of cliffs, but
                // prevents stalling at a given row.
                if self.ticks_to_lock <= 0 {
                    self.lock_active_piece_and_get_next();
                }
            } else {
                if is_soft_drop {
                    self.score += 1;
                }

                self.set_active_piece_row_and_reset_ticks(next_active_piece_row);
            }
        }
    }

    pub fn update(&mut self, input: GameInput) {
        if input.toggle_pause {
            self.toggle_pause();
        }

        if self.is_game_over || self.is_paused {
            return;
        }

        self.tick = (self.start.elapsed().as_secs_f32() * TICKS_PER_SECOND).floor() as usize;

        let tick_delta = self.get_tick_delta();
        self.impact_ticks_remaining = self.impact_ticks_remaining.saturating_sub(tick_delta);
        self.clear_effect_ticks_remaining =
            self.clear_effect_ticks_remaining.saturating_sub(tick_delta);

        let speed_modifier = if input.soft_drop {
            (G_SOFT_DROP / self.get_gravity()).ceil().max(1.0) as usize
        } else {
            1
        };

        self.ticks_to_next_row_inc -= (self.get_tick_delta() * speed_modifier) as isize;

        if input.hold_piece {
            self.swap_active_piece();
        }

        if input.rotate_right {
            self.try_rotate_right();
        }

        if input.hard_drop {
            self.hard_drop();
        }

        // Try and move the piece horizontally,
        self.try_move_horizontal(input.shift_left, input.shift_right);

        // Drop the piece, or lock it if dropping would cause a collision.
        self.try_gravity_drop(input.soft_drop);

        // Only update grids if piece state changed
        if self.piece_dirty {
            self.grid_active.clear().set_cells(
                self.active_piece_row,
                self.active_piece_col,
                &self.cached_blocks,
                self.cached_bounds_height,
                self.cached_bounds_width,
            );

            self.cached_ghost_row = self.grid_locked.find_landing_row(
                self.active_piece_row,
                self.active_piece_col,
                &self.cached_blocks,
                self.cached_bounds_height,
                self.cached_bounds_width,
            );

            self.grid_ghost.clear().set_cells(
                self.cached_ghost_row,
                self.active_piece_col,
                &self.cached_blocks,
                self.cached_bounds_height,
                self.cached_bounds_width,
            );

            self.piece_dirty = false;
        }
    }

    fn increase_rows_cleared(&mut self, new_rows_cleared: usize) {
        self.rows_cleared += new_rows_cleared;
    }

    fn clear_filled_rows_and_update_score(&mut self) {
        let rows_cleared = self.grid_locked.clear_all_filled_rows();
        let level = self.get_level();

        if rows_cleared > 0 {
            self.last_clear_count = rows_cleared;
            self.clear_effect_ticks_remaining = LINE_CLEAR_EFFECT_TICKS;
        }

        match rows_cleared {
            1 => self.score += 100 * level,
            2 => self.score += 300 * level,
            3 => self.score += 500 * level,
            4 => self.score += 800 * level,
            _ => (),
        };

        self.increase_rows_cleared(rows_cleared);
    }

    pub fn get_grid_locked(&self) -> &Grid {
        &self.grid_locked
    }

    pub fn get_grid_active(&self) -> &Grid {
        &self.grid_active
    }

    pub fn get_grid_ghost(&self) -> &Grid {
        &self.grid_ghost
    }

    pub fn get_score(&self) -> usize {
        self.score
    }

    pub fn get_rows_cleared(&self) -> usize {
        self.rows_cleared
    }

    pub fn get_level(&self) -> usize {
        // Minimum level is 1. Maximum is 20.
        (self.rows_cleared as f32 / 10.0).ceil().max(1.0).min(20.0) as usize
    }

    pub fn get_gravity(&self) -> f32 {
        let level = self.get_level();
        let gravity_seconds = (0.8 - ((level - 1) as f32 * 0.007)).powi(level as i32 - 1);
        let gravity_ticks_per_row = 1.0 / gravity_seconds;
        let gravity = gravity_ticks_per_row / 60.0;

        // Cap at 1G, or 1 row per tick.
        gravity.min(1.0)
    }

    pub fn get_piece_previews(&self) -> [Piece; 3] {
        [
            self.bag_manager.peek(1),
            self.bag_manager.peek(2),
            self.bag_manager.peek(3),
        ]
    }

    pub fn get_held_piece(&self) -> Option<Piece> {
        self.held_piece
    }

    pub fn get_is_game_over(&self) -> bool {
        self.is_game_over
    }

    pub fn get_is_paused(&self) -> bool {
        self.is_paused
    }

    pub fn get_impact_effect(&self) -> f32 {
        self.impact_ticks_remaining as f32 / (LOCK_IMPACT_TICKS + 12) as f32
    }

    pub fn get_clear_effect(&self) -> (usize, f32) {
        (
            self.last_clear_count,
            self.clear_effect_ticks_remaining as f32 / LINE_CLEAR_EFFECT_TICKS as f32,
        )
    }

    pub fn get_impact_origins(&self) -> (&[(f32, f32)], Color) {
        (
            &self.impact_origins[..self.impact_origin_count],
            self.impact_color,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::impact_contact_origins;
    use crate::{block::Block, grid::Grid, piece::BlockCanvas};
    use macroquad::prelude::WHITE;

    #[test]
    fn rotated_l_reports_each_contact_on_a_stepped_stack() {
        let mut locked = Grid::new();
        let block = Some(Block::new(WHITE));
        // Supporting bed:
        //    [e][f]
        // [g][h][i]
        locked
            .set_cell(11, 4, block)
            .set_cell(11, 5, block)
            .set_cell(12, 3, block)
            .set_cell(12, 4, block)
            .set_cell(12, 5, block);

        // Clockwise-rotated L:
        // [a][b][c]
        // [d]
        let mut canvas: BlockCanvas = [[None; 5]; 5];
        canvas[0][0] = block;
        canvas[0][1] = block;
        canvas[0][2] = block;
        canvas[1][0] = block;

        let (contacts, contact_count) = impact_contact_origins(&locked, 10, 3, &canvas, 2, 3);

        assert_eq!(contact_count, 3);
        assert_eq!(
            &contacts[..contact_count],
            &[(4.5, 11.0), (5.5, 11.0), (3.5, 12.0)]
        );
    }
}
