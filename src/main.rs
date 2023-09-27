mod bag_manager;
mod block;
mod grid;
mod piece;

use bag_manager::BagManager;
use block::Block;
use grid::{Grid, GRID_COUNT_COLS, GRID_COUNT_ROWS};
use macroquad::prelude::*;
use piece::Piece;
use std::time::Instant;

const BLOCK_SIZE: f32 = 20.0;
const PLAYFIELD_OFFSET_X: f32 = 40.0;
const PLAYFIELD_OFFSET_Y: f32 = 40.0;
const PLAYFIELD_WIDTH: f32 = GRID_COUNT_COLS as f32 * BLOCK_SIZE + OUTLINE_WIDTH;
const PLAYFIELD_HEIGHT: f32 = GRID_COUNT_ROWS as f32 * BLOCK_SIZE + OUTLINE_WIDTH;
const OUTLINE_WIDTH: f32 = 2.0;
const OFFSET_INNER_X: f32 = PLAYFIELD_OFFSET_X + (OUTLINE_WIDTH / 2.0);
const OFFSET_INNER_Y: f32 = PLAYFIELD_OFFSET_Y + (OUTLINE_WIDTH / 2.0);

const PREVIEW_OFFSET_X: f32 = PLAYFIELD_OFFSET_X * 2.0 + PLAYFIELD_WIDTH;
const PREVIEW_OFFSET_Y: f32 = PLAYFIELD_OFFSET_Y;
const PREVIEW_WIDTH: f32 = 6.0 * BLOCK_SIZE + OUTLINE_WIDTH;
const PREVIEW_HEIGHT: f32 = 6.0 * 3.0 * BLOCK_SIZE + OUTLINE_WIDTH;
const PREVIEW_OFFSET_INNER_X: f32 = PREVIEW_OFFSET_X + (OUTLINE_WIDTH / 2.0);
const PREVIEW_OFFSET_INNER_Y: f32 = PREVIEW_OFFSET_Y + (OUTLINE_WIDTH / 2.0);

const TICKS_PER_SECOND: f32 = 60.0;

fn draw_playfield() {
    draw_rectangle_lines(
        PLAYFIELD_OFFSET_X,
        PLAYFIELD_OFFSET_Y,
        PLAYFIELD_WIDTH,
        PLAYFIELD_HEIGHT,
        OUTLINE_WIDTH,
        WHITE,
    );
}

fn draw_block(block: Block, row_id: usize, col_id: usize, offset_x: f32, offset_y: f32) {
    draw_rectangle(
        offset_x + (col_id as f32 * BLOCK_SIZE),
        offset_y + (row_id as f32 * BLOCK_SIZE),
        BLOCK_SIZE,
        BLOCK_SIZE,
        block.color,
    );

    draw_rectangle_lines(
        offset_x + (col_id as f32 * BLOCK_SIZE),
        offset_y + (row_id as f32 * BLOCK_SIZE),
        BLOCK_SIZE,
        BLOCK_SIZE,
        OUTLINE_WIDTH,
        WHITE,
    );
}

fn draw_grid(grid: &Grid) {
    for row_id in 0..GRID_COUNT_ROWS {
        for col_id in 0..GRID_COUNT_COLS {
            let cell = grid.get_cell(row_id, col_id);

            match cell {
                Some(block) => draw_block(block, row_id, col_id, OFFSET_INNER_X, OFFSET_INNER_Y),
                None => (),
            }
        }
    }
}

fn draw_debug_info(tick: u32) {
    draw_text(
        &format!("tick: {}", tick),
        PLAYFIELD_OFFSET_X + PLAYFIELD_WIDTH + 350.0,
        PLAYFIELD_OFFSET_Y - 10.0,
        32.0,
        WHITE,
    );
}

fn draw_score(score: u32) {
    draw_text(
        &format!("score: {}", score),
        PLAYFIELD_OFFSET_X,
        PLAYFIELD_OFFSET_Y - 10.0,
        32.0,
        WHITE,
    );
}

fn draw_piece(piece: Piece, orientation: usize, offset_x: f32, offset_y: f32) {
    let blocks = piece.get_blocks(orientation);

    for row_id in 0..piece.bounds_height {
        for col_id in 0..piece.bounds_width {
            let cell = blocks[row_id][col_id];

            match cell {
                Some(block) => draw_block(block, row_id, col_id, offset_x, offset_y),
                None => (),
            }
        }
    }
}

fn draw_next_pieces(bag_manager: &BagManager) {
    draw_rectangle_lines(
        PREVIEW_OFFSET_X,
        PREVIEW_OFFSET_Y,
        PREVIEW_WIDTH,
        PREVIEW_HEIGHT,
        OUTLINE_WIDTH,
        WHITE,
    );

    let piece_1 = bag_manager.peek(1);
    let piece_2 = bag_manager.peek(2);
    let piece_3 = bag_manager.peek(3);

    draw_piece(piece_1, 0, PREVIEW_OFFSET_INNER_X, PREVIEW_OFFSET_INNER_Y);
    draw_piece(
        piece_2,
        0,
        PREVIEW_OFFSET_INNER_X,
        PREVIEW_OFFSET_INNER_Y + (6.0 * BLOCK_SIZE),
    );
    draw_piece(
        piece_3,
        0,
        PREVIEW_OFFSET_INNER_X,
        PREVIEW_OFFSET_INNER_Y + (6.0 * BLOCK_SIZE * 2.0),
    );
}

#[macroquad::main("Retris")]
async fn main() {
    // Game state
    let mut bag_manager = BagManager::new();
    let mut active_piece = bag_manager.next();
    let gravity: f32 = 1.0 / 60.0; // 1 row per 60 ticks (1 second)
    let mut grid_locked = Grid::new();
    let mut grid_active = Grid::new();
    let mut score: u32 = 0;
    let mut tick: u32;
    let mut last_tick: u32 = 0;
    let mut active_piece_col = active_piece.get_initial_col();
    let mut active_piece_row = 0;
    let mut orientation: usize = 0;
    let mut ticks_to_next_row_inc: i32 = (1.0 / gravity).ceil() as i32;

    let start = Instant::now();

    loop {
        tick = (start.elapsed().as_secs_f32() * TICKS_PER_SECOND).floor() as u32;

        // At 60fps or higher, this should be either 1 or 0. It may be more than 1 at lower frame rates.
        let delta_ticks = tick - last_tick;

        let speed_modifier = if is_key_down(KeyCode::Down) { 5 } else { 1 };

        ticks_to_next_row_inc -= (delta_ticks * speed_modifier) as i32;

        let mut next_active_piece_row = if ticks_to_next_row_inc <= 0 {
            active_piece_row + 1
        } else {
            active_piece_row
        };

        let mut col_offset = 0;
        match get_last_key_pressed() {
            Some(KeyCode::Left) => col_offset = -1,
            Some(KeyCode::Right) => col_offset = 1,
            Some(KeyCode::Up) => {
                let next_orientation = (orientation + 1) % 4;

                let has_collision = grid_locked.collision_check(
                    active_piece_row,
                    active_piece_col,
                    active_piece.get_blocks(next_orientation),
                );

                if !has_collision {
                    orientation = next_orientation;
                }
            }
            Some(KeyCode::C) => {
                active_piece = bag_manager.next();
                orientation = 0;
                active_piece_col = active_piece.get_initial_col();
                active_piece_row = 0;
                next_active_piece_row = 0;
                ticks_to_next_row_inc = (1.0 / gravity).ceil() as i32;
            }
            _ => (),
        };

        let next_active_piece_col = active_piece_col + col_offset;

        // Horizontal collision check
        if next_active_piece_col != active_piece_col {
            let has_collision = grid_locked.collision_check(
                active_piece_row,
                next_active_piece_col,
                active_piece.get_blocks(orientation),
            );

            if !has_collision {
                active_piece_col = next_active_piece_col;
            }
        }

        // Vertical collision check
        if next_active_piece_row != active_piece_row {
            let has_collision = grid_locked.collision_check(
                next_active_piece_row,
                active_piece_col,
                active_piece.get_blocks(orientation),
            );

            if has_collision {
                grid_locked.set_cells(
                    active_piece_row,
                    active_piece_col,
                    active_piece.get_blocks(orientation),
                );

                active_piece = bag_manager.next();
                orientation = 0;
                active_piece_col = active_piece.get_initial_col();
                active_piece_row = 0;
                ticks_to_next_row_inc = (1.0 / gravity).ceil() as i32;
            } else {
                active_piece_row = next_active_piece_row;
                ticks_to_next_row_inc = (1.0 / gravity).ceil() as i32;
            }
        }

        let active_blocks = active_piece.get_blocks(orientation);
        grid_active.set_cells(active_piece_row, active_piece_col, active_blocks);

        clear_background(BLACK);
        draw_score(score);
        draw_playfield();
        draw_grid(&grid_locked);
        draw_grid(&grid_active);
        draw_next_pieces(&bag_manager);

        draw_debug_info(tick);

        match grid_locked.clear_all_filled_rows() {
            1 => score += 100,
            2 => score += 300,
            3 => score += 500,
            4 => score += 800,
            _ => (),
        }

        grid_active.clear();

        last_tick = tick;

        next_frame().await
    }
}
