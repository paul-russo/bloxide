use std::time::Instant;

use macroquad::prelude::KeyCode;

use crate::{bag_manager::BagManager, grid::Grid, piece::Piece};

const TICKS_PER_SECOND: f32 = 60.0;

pub struct GameState {
    grid_locked: Grid,
    grid_active: Grid,
    grid_ghost: Grid,
    bag_manager: BagManager,
    active_piece: Piece,
    gravity: f32,
    score: u32,
    tick: u32,
    last_tick: u32,
    start: Instant,
    active_piece_col: isize,
    next_active_piece_col: isize,
    active_piece_row: isize,
    next_active_piece_row: isize,
    active_piece_orientation: usize,
    ticks_to_next_row_inc: i32,
    held_piece: Option<Piece>,
    last_piece_swapped: bool,
}

impl GameState {
    pub fn new() -> Self {
        let grid_locked = Grid::new();
        let grid_active = Grid::new();
        let grid_ghost = Grid::new();
        let mut bag_manager = BagManager::new();
        let active_piece = bag_manager.next();
        let gravity: f32 = 1.0 / 60.0; // 1 row per 60 ticks (1 second)
        let score: u32 = 0;
        let tick: u32 = 0;
        let last_tick: u32 = 0;
        let active_piece_col = active_piece.get_initial_col();
        let active_piece_row = 0;
        let active_piece_orientation: usize = 0;
        let ticks_to_next_row_inc: i32 = (1.0 / gravity).ceil() as i32;
        let start = Instant::now();

        Self {
            grid_locked,
            grid_active,
            grid_ghost,
            bag_manager,
            active_piece,
            gravity,
            score,
            tick,
            last_tick,
            start,
            active_piece_col,
            next_active_piece_col: active_piece_col,
            active_piece_row,
            next_active_piece_row: active_piece_row,
            active_piece_orientation,
            ticks_to_next_row_inc,
            held_piece: None,
            last_piece_swapped: false,
        }
    }

    pub fn start_tick(&mut self) -> u32 {
        self.tick = (self.start.elapsed().as_secs_f32() * TICKS_PER_SECOND).floor() as u32;
        self.tick
    }

    pub fn end_tick(&mut self) {
        self.last_tick = self.tick;
        self.update_score();
        self.grid_active.clear();
        self.grid_ghost.clear();
    }

    /// Returns the number of ticks elapsed between the last rendered tick and the current one.
    /// At 60fps or higher, this should be either 1 or 0. It may be more than 1 at lower frame rates.
    pub fn get_tick_delta(&self) -> u32 {
        self.tick - self.last_tick
    }

    fn reset_piece_state(&mut self) {
        self.active_piece_orientation = 0;
        self.active_piece_col = self.active_piece.get_initial_col();
        self.active_piece_row = 0;
        self.next_active_piece_row = 0;
        self.ticks_to_next_row_inc = (1.0 / self.gravity).ceil() as i32;
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

    pub fn apply_input(&mut self, is_down_key_down: bool, last_key_pressed: Option<KeyCode>) {
        let speed_modifier = if is_down_key_down { 5 } else { 1 };

        self.ticks_to_next_row_inc -= (self.get_tick_delta() * speed_modifier) as i32;

        self.next_active_piece_row = if self.ticks_to_next_row_inc <= 0 {
            self.active_piece_row + 1
        } else {
            self.active_piece_row
        };

        let mut col_offset = 0;

        match last_key_pressed {
            Some(KeyCode::Left) => col_offset = -1,
            Some(KeyCode::Right) => col_offset = 1,
            Some(KeyCode::Up) => {
                let next_orientation = (self.active_piece_orientation + 1) % 4;

                let has_collision = self.grid_locked.collision_check(
                    self.active_piece_row,
                    self.active_piece_col,
                    &self.active_piece.get_blocks(next_orientation, false),
                );

                if !has_collision {
                    self.active_piece_orientation = next_orientation;
                }
            }
            Some(KeyCode::C) => {
                self.swap_active_piece();
            }
            Some(KeyCode::Space) => {
                self.hard_drop();
            }
            _ => (),
        };

        self.next_active_piece_col = self.active_piece_col + col_offset;

        // Horizontal collision check
        if self.next_active_piece_col != self.active_piece_col {
            let has_collision = self.grid_locked.collision_check(
                self.active_piece_row,
                self.next_active_piece_col,
                &self
                    .active_piece
                    .get_blocks(self.active_piece_orientation, false),
            );

            if !has_collision {
                self.active_piece_col = self.next_active_piece_col;
            }
        }

        // Vertical collision check
        if self.next_active_piece_row != self.active_piece_row {
            let has_collision = self.grid_locked.collision_check(
                self.next_active_piece_row,
                self.active_piece_col,
                &self
                    .active_piece
                    .get_blocks(self.active_piece_orientation, false),
            );

            if has_collision {
                self.grid_locked.set_cells(
                    self.active_piece_row,
                    self.active_piece_col,
                    &self
                        .active_piece
                        .get_blocks(self.active_piece_orientation, false),
                );

                self.active_piece = self.bag_manager.next();
                self.active_piece_orientation = 0;
                self.active_piece_col = self.active_piece.get_initial_col();
                self.active_piece_row = 0;
                self.ticks_to_next_row_inc = (1.0 / self.gravity).ceil() as i32;
            } else {
                self.active_piece_row = self.next_active_piece_row;
                self.ticks_to_next_row_inc = (1.0 / self.gravity).ceil() as i32;
            }
        }

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

    fn update_score(&mut self) {
        match self.grid_locked.clear_all_filled_rows() {
            1 => self.score += 100,
            2 => self.score += 300,
            3 => self.score += 500,
            4 => self.score += 800,
            _ => (),
        }
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

    pub fn get_score(&self) -> u32 {
        self.score
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
