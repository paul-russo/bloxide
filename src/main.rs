mod bag_manager;
mod block;
mod game_state;
mod grid;
mod piece;
mod utils;

use block::Block;
use game_state::{GameInput, GameState};
use grid::{Grid, FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, GRID_COUNT_ROWS, VISIBLE_GRID_COUNT_ROWS};
use macroquad::prelude::*;
use piece::Piece;

const BLOCK_SIZE: f32 = 20.0;
const PLAYFIELD_OFFSET_Y: f32 = 40.0;
const PLAYFIELD_WIDTH: f32 = GRID_COUNT_COLS as f32 * BLOCK_SIZE + OUTLINE_WIDTH;
const PLAYFIELD_HEIGHT: f32 = VISIBLE_GRID_COUNT_ROWS as f32 * BLOCK_SIZE + OUTLINE_WIDTH;
const OUTLINE_WIDTH: f32 = 2.0;
const PREVIEW_WIDTH: f32 =
    PREVIEW_PIECE_MAX_BLOCKS_W * BLOCK_SIZE + OUTLINE_WIDTH + (PREVIEW_PADDING_X * 2.0);

const HOLD_PADDING_X: f32 = 10.0;
const HOLD_PADDING_Y: f32 = 10.0;
const HOLD_WIDTH: f32 = PREVIEW_WIDTH;
const HOLD_OFFSET_X: f32 = PLAYFIELD_MARGIN;
const HOLD_OFFSET_Y: f32 = PLAYFIELD_OFFSET_Y;
const HOLD_OFFSET_INNER_X: f32 = HOLD_OFFSET_X + (OUTLINE_WIDTH / 2.0) + HOLD_PADDING_X;
const HOLD_OFFSET_INNER_Y: f32 = HOLD_OFFSET_Y + (OUTLINE_WIDTH / 2.0) + HOLD_PADDING_Y;

const PLAYFIELD_OFFSET_X: f32 = HOLD_WIDTH + (HOLD_OFFSET_X * 2.0);

const OFFSET_INNER_X: f32 = PLAYFIELD_OFFSET_X + (OUTLINE_WIDTH / 2.0);
const OFFSET_INNER_Y: f32 = PLAYFIELD_OFFSET_Y + (OUTLINE_WIDTH / 2.0);

const PLAYFIELD_MARGIN: f32 = 20.0;

const PREVIEW_OFFSET_X: f32 = PLAYFIELD_OFFSET_X + PLAYFIELD_WIDTH + PLAYFIELD_MARGIN;
const PREVIEW_OFFSET_Y: f32 = PLAYFIELD_OFFSET_Y;
const PREVIEW_PIECE_MAX_BLOCKS_H: f32 = 2.0;
const PREVIEW_PIECE_MAX_BLOCKS_W: f32 = 4.0;
const PREVIEW_PIECE_MARGIN: f32 = 20.0;
const PREVIEW_PADDING_X: f32 = 10.0;
const PREVIEW_PADDING_Y: f32 = 10.0;
const PREVIEW_HEIGHT: f32 = PREVIEW_PIECE_MAX_BLOCKS_H * 3.0 * BLOCK_SIZE
    + OUTLINE_WIDTH
    + (PREVIEW_PIECE_MARGIN * 2.0)
    + (PREVIEW_PADDING_Y * 2.0);
const PREVIEW_OFFSET_INNER_X: f32 = PREVIEW_OFFSET_X + (OUTLINE_WIDTH / 2.0) + PREVIEW_PADDING_X;
const PREVIEW_OFFSET_INNER_Y: f32 = PREVIEW_OFFSET_Y + (OUTLINE_WIDTH / 2.0) + PREVIEW_PADDING_Y;

const HOLD_HEIGHT: f32 =
    PREVIEW_PIECE_MAX_BLOCKS_H * BLOCK_SIZE + OUTLINE_WIDTH + (PREVIEW_PADDING_Y * 2.0);

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

fn draw_block(
    block: Block,
    row_id: usize,
    col_id: usize,
    offset_x: f32,
    offset_y: f32,
    opacity: f32,
) {
    draw_rectangle(
        offset_x + (col_id as f32 * BLOCK_SIZE),
        offset_y + (row_id as f32 * BLOCK_SIZE),
        BLOCK_SIZE,
        BLOCK_SIZE,
        Color {
            r: block.color.r,
            g: block.color.g,
            b: block.color.b,
            a: opacity,
        },
    );

    draw_rectangle_lines(
        offset_x + (col_id as f32 * BLOCK_SIZE),
        offset_y + (row_id as f32 * BLOCK_SIZE),
        BLOCK_SIZE,
        BLOCK_SIZE,
        OUTLINE_WIDTH,
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: opacity,
        },
    );
}

fn draw_grid(grid: &Grid, opacity: f32) {
    for row_id in FIRST_VISIBLE_ROW_ID..GRID_COUNT_ROWS {
        for col_id in 0..GRID_COUNT_COLS {
            let cell = grid.get_cell(row_id, col_id);

            match cell {
                Some(block) => draw_block(
                    block,
                    row_id - FIRST_VISIBLE_ROW_ID,
                    col_id,
                    OFFSET_INNER_X,
                    OFFSET_INNER_Y,
                    opacity,
                ),
                None => (),
            }
        }
    }
}

fn draw_debug_info(
    tick: usize,
    gravity: f32,
    last_key_pressed: Option<KeyCode>,
    speed_modifier: usize,
) {
    draw_text(
        &format!("tick: {}", tick),
        PLAYFIELD_OFFSET_X + PLAYFIELD_WIDTH + 350.0,
        PLAYFIELD_OFFSET_Y - 10.0,
        16.0,
        WHITE,
    );

    draw_text(
        &format!("gravity: {}", gravity),
        PLAYFIELD_OFFSET_X + PLAYFIELD_WIDTH + 350.0,
        PLAYFIELD_OFFSET_Y + 6.0,
        16.0,
        WHITE,
    );

    draw_text(
        &format!("key: {:?}", last_key_pressed),
        PLAYFIELD_OFFSET_X + PLAYFIELD_WIDTH + 350.0,
        PLAYFIELD_OFFSET_Y + 22.0,
        16.0,
        WHITE,
    );

    draw_text(
        &format!("modifier: {}", speed_modifier),
        PLAYFIELD_OFFSET_X + PLAYFIELD_WIDTH + 350.0,
        PLAYFIELD_OFFSET_Y + 38.0,
        16.0,
        WHITE,
    );

    draw_text(
        &format!("FPS: {} ({}ms)", get_fps(), get_frame_time() * 1000.0),
        PLAYFIELD_OFFSET_X + PLAYFIELD_WIDTH + 350.0,
        PLAYFIELD_OFFSET_Y + 54.0,
        16.0,
        WHITE,
    );
}

fn draw_score(score: usize) {
    draw_text(
        &format!("score: {}", score),
        PLAYFIELD_OFFSET_X,
        PLAYFIELD_OFFSET_Y - 10.0,
        32.0,
        WHITE,
    );
}

fn draw_level(level: usize) {
    draw_text(
        &format!("Level {}", level),
        PREVIEW_OFFSET_X,
        PREVIEW_OFFSET_Y + PREVIEW_HEIGHT + PLAYFIELD_MARGIN + 10.0,
        32.0,
        WHITE,
    );
}

fn draw_game_over_screen() {
    draw_rectangle(
        PLAYFIELD_OFFSET_X,
        PLAYFIELD_OFFSET_Y + (PLAYFIELD_HEIGHT / 2.0) - 32.0,
        PLAYFIELD_WIDTH,
        64.0,
        color_u8!(80, 80, 80, 255),
    );

    draw_text(
        "GAME OVER",
        PLAYFIELD_OFFSET_X + 38.0,
        PLAYFIELD_OFFSET_Y + (PLAYFIELD_HEIGHT / 2.0) + 8.0,
        32.0,
        WHITE,
    );
}

fn draw_piece(piece: Piece, orientation: usize, offset_x: f32, offset_y: f32) {
    let blocks = piece.get_blocks(orientation, true);

    for row_id in 0..blocks.len() {
        for col_id in 0..blocks[row_id].len() {
            let cell = blocks[row_id][col_id];

            match cell {
                Some(block) => draw_block(block, row_id, col_id, offset_x, offset_y, 1.0),
                None => (),
            }
        }
    }
}

fn draw_piece_previews(piece_previews: Vec<Piece>) {
    draw_text(
        &format!("Next"),
        PREVIEW_OFFSET_X,
        PREVIEW_OFFSET_Y - 10.0,
        32.0,
        WHITE,
    );

    draw_rectangle_lines(
        PREVIEW_OFFSET_X,
        PREVIEW_OFFSET_Y,
        PREVIEW_WIDTH,
        PREVIEW_HEIGHT,
        OUTLINE_WIDTH,
        WHITE,
    );

    for offset in 0..3 {
        let piece = piece_previews[offset];
        let piece_w = piece.orientations[0].bounds_x.1 - piece.orientations[0].bounds_x.0;
        let piece_offset_x = ((PREVIEW_PIECE_MAX_BLOCKS_W - piece_w as f32) / 2.0) * BLOCK_SIZE;

        draw_piece(
            piece,
            0,
            PREVIEW_OFFSET_INNER_X + piece_offset_x,
            PREVIEW_OFFSET_INNER_Y
                + (2.0 * BLOCK_SIZE * (offset as f32))
                + (PREVIEW_PIECE_MARGIN * (offset as f32)),
        );
    }
}

fn draw_held_piece(held_piece: Option<Piece>) {
    draw_text(
        &format!("Hold"),
        HOLD_OFFSET_X,
        HOLD_OFFSET_Y - 10.0,
        32.0,
        WHITE,
    );

    draw_rectangle_lines(
        HOLD_OFFSET_X,
        HOLD_OFFSET_Y,
        HOLD_WIDTH,
        HOLD_HEIGHT,
        OUTLINE_WIDTH,
        WHITE,
    );

    if let Some(piece) = held_piece {
        let piece_w = piece.orientations[0].bounds_x.1 - piece.orientations[0].bounds_x.0;
        let piece_offset_x = ((PREVIEW_PIECE_MAX_BLOCKS_W - piece_w as f32) / 2.0) * BLOCK_SIZE;

        draw_piece(
            piece,
            0,
            HOLD_OFFSET_INNER_X + piece_offset_x,
            HOLD_OFFSET_INNER_Y,
        );
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: String::from("Retris"),
        high_dpi: true,
        window_resizable: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Game state
    let mut game_state = GameState::new();

    loop {
        game_state.update(GameInput {
            soft_drop: is_key_down(KeyCode::Down),
            shift_left: is_key_down(KeyCode::Left),
            shift_right: is_key_down(KeyCode::Right),
            rotate_right: is_key_pressed(KeyCode::Up),
            hard_drop: is_key_pressed(KeyCode::Space),
            hold_piece: is_key_pressed(KeyCode::C),
        });

        clear_background(BLACK);
        draw_score(game_state.get_score());
        draw_level(game_state.get_level());
        draw_playfield();
        draw_grid(game_state.get_grid_locked(), 1.0);
        draw_grid(game_state.get_grid_active(), 1.0);
        draw_grid(game_state.get_grid_ghost(), 0.5);
        draw_piece_previews(game_state.get_piece_previews());
        draw_held_piece(game_state.get_held_piece());

        // draw_debug_info(tick, gravity, last_key_pressed, speed_modifier);

        if game_state.get_is_game_over() {
            draw_game_over_screen();
        }

        game_state.clean_up();
        next_frame().await
    }
}
