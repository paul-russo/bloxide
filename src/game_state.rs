use std::time::Instant;

use macroquad::prelude::KeyCode;

use crate::{bag_manager::BagManager, grid::Grid, piece::Piece};

const TICKS_PER_SECOND: f32 = 60.0;
const INITIAL_GRAVITY: f32 = 1.0 / 60.0; // 1/60G. 1 row per 60 ticks (1 second)
const G_SOFT_DROP: f32 = 30.0 / 60.0; // 1/2G. 30 rows per 60 ticks (1 second)

enum CollisionVar {
    Row(isize),
    Col(isize),
    Orientation(usize),
}

impl CollisionVar {
    fn row_or(&self, fallback: isize) -> isize {
        match &self {
            CollisionVar::Col(_) => fallback,
            CollisionVar::Row(value) => *value,
            CollisionVar::Orientation(_) => fallback,
        }
    }

    fn col_or(&self, fallback: isize) -> isize {
        match &self {
            CollisionVar::Col(value) => *value,
            CollisionVar::Row(_) => fallback,
            CollisionVar::Orientation(_) => fallback,
        }
    }

    fn orientation_or(&self, fallback: usize) -> usize {
        match &self {
            CollisionVar::Col(_) => fallback,
            CollisionVar::Row(_) => fallback,
            CollisionVar::Orientation(value) => *value,
        }
    }
}

pub struct GameState {
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
    held_piece: Option<Piece>,
    last_piece_swapped: bool,
    rows_cleared: usize,
    level: usize,
}

impl GameState {
    pub fn new() -> Self {
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
            held_piece: None,
            last_piece_swapped: false,
            rows_cleared: 0,
            level: 1,
        }
    }

    pub fn start_tick(&mut self) -> usize {
        self.tick = (self.start.elapsed().as_secs_f32() * TICKS_PER_SECOND).floor() as usize;
        self.tick
    }

    pub fn end_tick(&mut self) {
        self.last_tick = self.tick;
        self.clear_filled_rows_and_update_score();
        self.grid_active.clear();
        self.grid_ghost.clear();
    }

    /// Returns the number of ticks elapsed between the last rendered tick and the current one.
    /// At 60fps or higher, this should be either 1 or 0. It may be more than 1 at lower frame rates.
    pub fn get_tick_delta(&self) -> usize {
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
    }

    fn set_active_piece_and_reset_state(&mut self, active_piece: Piece) {
        self.active_piece = active_piece;
        self.reset_piece_state();
    }

    fn next_piece(&mut self) {
        let active_piece = self.bag_manager.next();
        self.set_active_piece_and_reset_state(active_piece);
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

    fn lock_active_piece(&mut self) {
        self.grid_locked.set_cells(
            self.active_piece_row,
            self.active_piece_col,
            &self
                .active_piece
                .get_blocks(self.active_piece_orientation, false),
        );
    }

    fn hard_drop(&mut self) {
        self.active_piece_row = self.grid_locked.find_landing_row(
            self.active_piece_row,
            self.active_piece_col,
            &self
                .active_piece
                .get_blocks(self.active_piece_orientation, false),
        );

        self.lock_active_piece();
        self.next_piece();
    }

    fn get_next_active_piece_row(&self) -> isize {
        if self.ticks_to_next_row_inc <= 0 {
            self.active_piece_row + 1
        } else {
            self.active_piece_row
        }
    }

    fn collision_check(&self, collision_var: CollisionVar) -> bool {
        self.grid_locked.collision_check(
            collision_var.row_or(self.active_piece_row),
            collision_var.col_or(self.active_piece_col),
            &self.active_piece.get_blocks(
                collision_var.orientation_or(self.active_piece_orientation),
                false,
            ),
        )
    }

    fn try_rotate_right(&mut self) {
        let next_orientation = (self.active_piece_orientation + 1) % 4;

        let has_collision = self.collision_check(CollisionVar::Orientation(next_orientation));

        if !has_collision {
            self.active_piece_orientation = next_orientation;
        }
    }

    fn try_move_horizontal(&mut self, col_offset: isize) {
        let next_active_piece_col = self.active_piece_col + col_offset;

        // Horizontal collision check
        if next_active_piece_col != self.active_piece_col {
            let has_collision = self.collision_check(CollisionVar::Col(next_active_piece_col));

            if !has_collision {
                self.active_piece_col = next_active_piece_col;
            }
        }
    }

    fn set_active_piece_row(&mut self, new_active_piece_row: isize) {
        self.active_piece_row = new_active_piece_row;
        self.ticks_to_next_row_inc = self.get_new_ticks_to_next_row_inc();
    }

    fn try_gravity_drop(&mut self) {
        let next_active_piece_row = self.get_next_active_piece_row();

        // Vertical collision check
        if next_active_piece_row != self.active_piece_row {
            let has_collision = self.collision_check(CollisionVar::Row(next_active_piece_row));

            if has_collision {
                self.lock_active_piece();
                self.next_piece();
            } else {
                self.set_active_piece_row(next_active_piece_row);
            }
        }
    }

    pub fn apply_input(&mut self, is_soft_drop: bool, last_key_pressed: Option<KeyCode>) {
        let speed_modifier = if is_soft_drop {
            (G_SOFT_DROP / self.get_gravity()).ceil().max(1.0) as usize
        } else {
            1
        };

        self.ticks_to_next_row_inc -= (self.get_tick_delta() * speed_modifier) as isize;

        let mut col_offset = 0;

        match last_key_pressed {
            Some(KeyCode::Left) => col_offset = -1,
            Some(KeyCode::Right) => col_offset = 1,
            Some(KeyCode::Up) => self.try_rotate_right(),
            Some(KeyCode::C) => self.swap_active_piece(),
            Some(KeyCode::Space) => self.hard_drop(),
            _ => (),
        };

        // Try and move the piece horizontally,
        self.try_move_horizontal(col_offset);

        // Drop the piece, or lock it if dropping would cause a collision.
        self.try_gravity_drop();

        let active_blocks = self
            .active_piece
            .get_blocks(self.active_piece_orientation, false);

        self.grid_active
            .set_cells(self.active_piece_row, self.active_piece_col, &active_blocks);

        let ghost_row = self.grid_locked.find_landing_row(
            self.active_piece_row,
            self.active_piece_col,
            &active_blocks,
        );

        self.grid_ghost
            .set_cells(ghost_row, self.active_piece_col, &active_blocks);
    }

    fn increase_rows_cleared(&mut self, new_rows_cleared: usize) {
        self.rows_cleared += new_rows_cleared;
        self.level = (self.rows_cleared as f32 / 10.0).ceil().max(1.0) as usize;
    }

    fn clear_filled_rows_and_update_score(&mut self) {
        let rows_cleared = self.grid_locked.clear_all_filled_rows();

        match rows_cleared {
            1 => self.score += 100,
            2 => self.score += 300,
            3 => self.score += 500,
            4 => self.score += 800,
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

    pub fn get_level(&self) -> usize {
        self.level
    }

    pub fn get_gravity(&self) -> f32 {
        self.level as f32 / 60.0
    }

    pub fn get_piece_previews(&self) -> Vec<Piece> {
        vec![
            self.bag_manager.peek(1),
            self.bag_manager.peek(2),
            self.bag_manager.peek(3),
        ]
    }

    pub fn get_held_piece(&self) -> Option<Piece> {
        self.held_piece
    }
}
