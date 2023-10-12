use crate::block::Block;
use crate::game_state::GameState;
use crate::grid::{
    Grid, FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, GRID_COUNT_ROWS, VISIBLE_GRID_COUNT_ROWS,
};
use crate::piece::Piece;
use macroquad::prelude::*;
use num_format::{Locale, ToFormattedString};

const BLOCK_SIZE: f32 = 20.0;
const PLAYFIELD_OFFSET_Y: f32 = 40.0;
const OUTLINE_WIDTH: f32 = 2.0;
const PLAYFIELD_WIDTH: f32 = GRID_COUNT_COLS as f32 * BLOCK_SIZE + OUTLINE_WIDTH;
const PLAYFIELD_HEIGHT: f32 = VISIBLE_GRID_COUNT_ROWS as f32 * BLOCK_SIZE + OUTLINE_WIDTH;
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

// Text is ~14 pixels wide per character at 32 pixels tall. 14/32 = 0.4375
const TEXT_HEIGHT_WIDTH_RATIO: f32 = 0.4375;

fn draw_text_centered(
    container_width: f32,
    container_height: Option<f32>,
    text: &str,
    offset_x: f32,
    offset_y: f32,
    text_size: f32,
    color: Color,
) {
    draw_text(
        text,
        offset_x
            + ((container_width - (text.len() as f32 * text_size * TEXT_HEIGHT_WIDTH_RATIO)) / 2.0),
        offset_y + (container_height.unwrap_or(0.0) / 2.0),
        text_size,
        color,
    );
}

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

fn draw_score(score: usize) {
    let text = &(score * 100).to_formatted_string(&Locale::en);

    draw_text_centered(
        PLAYFIELD_WIDTH,
        None,
        text,
        PLAYFIELD_OFFSET_X + 1.0,
        PLAYFIELD_OFFSET_Y - 9.0,
        40.0,
        color_u8!(224, 127, 58, 255),
    );

    draw_text_centered(
        PLAYFIELD_WIDTH,
        None,
        text,
        PLAYFIELD_OFFSET_X,
        PLAYFIELD_OFFSET_Y - 10.0,
        40.0,
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

fn draw_banner(text: &str) {
    draw_rectangle(
        PLAYFIELD_OFFSET_X,
        PLAYFIELD_OFFSET_Y + (PLAYFIELD_HEIGHT / 2.0) - 32.0,
        PLAYFIELD_WIDTH,
        64.0,
        color_u8!(80, 80, 80, 255),
    );

    draw_text_centered(
        PLAYFIELD_WIDTH,
        Some(PLAYFIELD_HEIGHT),
        text,
        PLAYFIELD_OFFSET_X,
        PLAYFIELD_OFFSET_Y + 8.0,
        32.0,
        WHITE,
    );
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

        piece.draw(DrawPieceArgs {
            orientation: 0,
            offset_x: PREVIEW_OFFSET_INNER_X + piece_offset_x,
            offset_y: PREVIEW_OFFSET_INNER_Y
                + (2.0 * BLOCK_SIZE * (offset as f32))
                + (PREVIEW_PIECE_MARGIN * (offset as f32)),
        });
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

        piece.draw(DrawPieceArgs {
            orientation: 0,
            offset_x: HOLD_OFFSET_INNER_X + piece_offset_x,
            offset_y: HOLD_OFFSET_INNER_Y,
        });
    }
}

pub trait Drawable {
    type Args;

    fn draw(&self, args: Self::Args);
}

impl Drawable for GameState {
    type Args = ();

    fn draw(&self, _args: ()) {
        draw_playfield();
        draw_score(self.get_score());
        draw_level(self.get_level());
        self.get_grid_locked().draw(1.0);
        self.get_grid_active().draw(1.0);
        self.get_grid_ghost().draw(0.5);
        draw_piece_previews(self.get_piece_previews());
        draw_held_piece(self.get_held_piece());

        if self.get_is_game_over() {
            draw_banner("GAME OVER");
        }

        if self.get_is_paused() {
            draw_banner("PAUSED");
        }
    }
}

impl Drawable for Grid {
    type Args = f32;

    fn draw(&self, opacity: f32) {
        for row_id in FIRST_VISIBLE_ROW_ID..GRID_COUNT_ROWS {
            for col_id in 0..GRID_COUNT_COLS {
                let cell = self.get_cell(row_id, col_id);

                match cell {
                    Some(block) => block.draw(DrawBlockArgs {
                        row_id: row_id - FIRST_VISIBLE_ROW_ID,
                        col_id,
                        offset_x: OFFSET_INNER_X,
                        offset_y: OFFSET_INNER_Y,
                        opacity,
                    }),
                    None => (),
                }
            }
        }
    }
}

pub struct DrawPieceArgs {
    orientation: usize,
    offset_x: f32,
    offset_y: f32,
}

impl Drawable for Piece {
    type Args = DrawPieceArgs;

    fn draw(&self, args: DrawPieceArgs) {
        let DrawPieceArgs {
            offset_x,
            offset_y,
            orientation,
        } = args;

        let blocks = self.get_blocks(orientation, true);

        for row_id in 0..blocks.len() {
            for col_id in 0..blocks[row_id].len() {
                let cell = blocks[row_id][col_id];

                match cell {
                    Some(block) => block.draw(DrawBlockArgs {
                        row_id,
                        col_id,
                        offset_x,
                        offset_y,
                        opacity: 1.0,
                    }),
                    None => (),
                }
            }
        }
    }
}

pub struct DrawBlockArgs {
    row_id: usize,
    col_id: usize,
    offset_x: f32,
    offset_y: f32,
    opacity: f32,
}

impl Drawable for Block {
    type Args = DrawBlockArgs;

    fn draw(&self, args: DrawBlockArgs) {
        let DrawBlockArgs {
            offset_x,
            offset_y,
            row_id,
            col_id,
            opacity,
        } = args;

        draw_rectangle(
            offset_x + (col_id as f32 * BLOCK_SIZE),
            offset_y + (row_id as f32 * BLOCK_SIZE),
            BLOCK_SIZE,
            BLOCK_SIZE,
            Color {
                r: self.color.r,
                g: self.color.g,
                b: self.color.b,
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
}
