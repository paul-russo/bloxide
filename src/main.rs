mod block;
mod grid;
mod piece;

use grid::{Grid, GRID_COUNT_COLS, GRID_COUNT_ROWS};
use macroquad::prelude::*;
use piece::get_random_bag;
use std::{cmp, time::Instant};

const BLOCK_SIZE: f32 = 20.0;
const PLAYFIELD_OFFSET_X: f32 = 40.0;
const PLAYFIELD_OFFSET_Y: f32 = 40.0;
const PLAYFIELD_WIDTH: f32 = GRID_COUNT_COLS as f32 * BLOCK_SIZE + OUTLINE_WIDTH;
const PLAYFIELD_HEIGHT: f32 = GRID_COUNT_ROWS as f32 * BLOCK_SIZE + OUTLINE_WIDTH;
const OUTLINE_WIDTH: f32 = 2.0;
const OFFSET_INNER_X: f32 = PLAYFIELD_OFFSET_X + (OUTLINE_WIDTH / 2.0);
const OFFSET_INNER_Y: f32 = PLAYFIELD_OFFSET_Y + (OUTLINE_WIDTH / 2.0);

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

fn draw_grid(grid: &Grid) {
    for row in 0..GRID_COUNT_ROWS {
        for col in 0..GRID_COUNT_COLS {
            let cell = grid.get_cell(row, col);

            match cell {
                Some(block) => {
                    draw_rectangle(
                        OFFSET_INNER_X + (col as f32 * BLOCK_SIZE),
                        OFFSET_INNER_Y + (row as f32 * BLOCK_SIZE),
                        BLOCK_SIZE,
                        BLOCK_SIZE,
                        block.color,
                    );

                    draw_rectangle_lines(
                        OFFSET_INNER_X + (col as f32 * BLOCK_SIZE),
                        OFFSET_INNER_Y + (row as f32 * BLOCK_SIZE),
                        BLOCK_SIZE,
                        BLOCK_SIZE,
                        OUTLINE_WIDTH,
                        WHITE,
                    );
                }
                None => {}
            }
        }
    }
}

fn draw_debug_info(tick: u32) {
    draw_text(
        &format!("tick: {}", tick),
        PLAYFIELD_OFFSET_X,
        PLAYFIELD_OFFSET_Y - 10.0,
        32.0,
        WHITE,
    );
}

fn draw_score(score: u32) {
    draw_text(
        &format!("score: {}", score),
        PLAYFIELD_OFFSET_X * 2.0 + PLAYFIELD_WIDTH,
        PLAYFIELD_OFFSET_Y + 16.0,
        32.0,
        WHITE,
    );
}

#[macroquad::main("Retris")]
async fn main() {
    // Game state
    let bag = get_random_bag();
    let mut active_piece_index = 0;
    let mut active_piece = bag[active_piece_index];
    let gravity: f32 = 1.0 / 60.0; // 1 row per 60 ticks
    let mut grid_locked = Grid::new();
    let mut grid_active = Grid::new();
    let mut score: u32 = 0;
    let mut tick: u32;
    let mut active_piece_col = ((10 - bag[active_piece_index].bounds_width) / 2) as isize;
    let mut active_piece_row = 0;
    let mut orientation: usize = 0;
    let mut last_row_inc: u32 = 0;

    let start = Instant::now();

    loop {
        tick = (start.elapsed().as_secs_f32() * TICKS_PER_SECOND).floor() as u32;
        let ticks_since_inc = tick - last_row_inc;

        let gravity_modifier = if is_key_down(KeyCode::Down) { 5.0 } else { 1.0 };

        let next_active_piece_row = active_piece_row
            + (ticks_since_inc as f32 * gravity * gravity_modifier).floor() as isize;

        let mut col_offset = 0;
        match get_last_key_pressed() {
            Some(KeyCode::Left) => col_offset = -1,
            Some(KeyCode::Right) => col_offset = 1,
            Some(KeyCode::Up) => orientation = (orientation + 1) % 4,
            Some(KeyCode::C) => {
                active_piece_index = (active_piece_index + 1) % 7;
                active_piece = bag[active_piece_index];
                orientation = 0;
                active_piece_col = ((10 - bag[active_piece_index].bounds_width) / 2) as isize;
                last_row_inc = tick;
                active_piece_row = 0;
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

            println!("horizontal: {:?}", has_collision);

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

            println!("vertical: {:?}", has_collision);

            if has_collision {
                grid_locked.set_cells(
                    active_piece_row,
                    active_piece_col,
                    active_piece.get_blocks(orientation),
                );

                active_piece_index = (active_piece_index + 1) % 7;
                active_piece = bag[active_piece_index];
                orientation = 0;
                active_piece_col = ((10 - bag[active_piece_index].bounds_width) / 2) as isize;
                last_row_inc = tick;
                active_piece_row = 0;
            } else {
                active_piece_row = next_active_piece_row;
                last_row_inc = tick;
            }
        }

        let active_blocks = active_piece.get_blocks(orientation);
        grid_active.set_cells(active_piece_row, active_piece_col, active_blocks);

        clear_background(BLACK);
        draw_score(score);
        draw_playfield();
        draw_grid(&grid_locked);
        draw_grid(&grid_active);
        draw_debug_info(tick);

        match grid_locked.clear_all_filled_rows() {
            1 => score += 100,
            2 => score += 300,
            3 => score += 500,
            4 => score += 800,
            _ => (),
        }

        grid_active.clear();

        next_frame().await
    }
}
