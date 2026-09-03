use super::{GameInput, GameState};
use crate::{
    block::Block,
    grid::{Grid, GRID_COUNT_COLS, GRID_COUNT_ROWS},
    high_score_manager::HighScoreManager,
    piece::{pieces, Piece},
};
use macroquad::prelude::WHITE;

fn state_with_piece(high_scores: &HighScoreManager, piece: Piece) -> GameState<'_> {
    let mut state = GameState::new(high_scores);
    state.set_active_piece_and_reset_state(piece);
    state
}

fn occupied_cells(grid: &Grid) -> usize {
    (0..GRID_COUNT_ROWS)
        .map(|row| {
            (0..GRID_COUNT_COLS)
                .filter(|&col| grid.has_block_at_cell(row, col))
                .count()
        })
        .sum()
}

#[test]
fn blocked_hold_stops_before_same_update_hard_drop_and_ghost_projection() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::I);
    state.held_piece = Some(pieces::T);
    state.grid_locked.set_cell(1, 4, Some(Block::new(WHITE)));

    state.update(GameInput {
        hold_piece: true,
        hard_drop: true,
        ..Default::default()
    });

    assert!(state.is_game_over);
    assert_eq!(state.active_piece.name, "T");
    assert_eq!(state.active_piece_row, 1);
    assert_eq!(state.score, 0);
    assert_eq!(occupied_cells(&state.grid_locked), 1);
    assert_eq!(occupied_cells(&state.grid_active), 0);
    assert_eq!(occupied_cells(&state.grid_ghost), 0);
}

#[test]
fn hard_drop_block_out_stops_before_moving_the_blocked_successor() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    state.active_piece_row = 20;
    state.active_piece_col = 0;
    let next_piece = state.bag_manager.peek(1);
    let (canvas, height, width) = next_piece.get_blocks(0);
    let (row, col) = (0..height)
        .find_map(|row| {
            (0..width)
                .find(|&col| canvas[row][col].is_some())
                .map(|col| (row + 1, col + next_piece.get_initial_col() as usize))
        })
        .expect("every piece has an occupied spawn cell");
    state.grid_locked.set_cell(row, col, Some(Block::new(WHITE)));

    state.update(GameInput {
        hard_drop: true,
        shift_left: true,
        ..Default::default()
    });

    assert!(state.is_game_over);
    assert_eq!(state.active_piece.name, next_piece.name);
    assert_eq!(state.active_piece_col, next_piece.get_initial_col());
    assert_eq!(state.score, 0);
    assert_eq!(occupied_cells(&state.grid_locked), 5);
    assert_eq!(occupied_cells(&state.grid_active), 0);
    assert_eq!(occupied_cells(&state.grid_ghost), 0);
}

#[test]
fn game_over_ignores_all_further_input_including_pause() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::I);
    state.end_game();
    let previews = state.get_piece_previews().map(|piece| piece.name);

    state.update(GameInput {
        soft_drop: true,
        shift_left: true,
        shift_right: true,
        rotate_right: true,
        hard_drop: true,
        hold_piece: true,
        toggle_pause: true,
    });

    assert!(state.is_game_over);
    assert!(!state.is_paused);
    assert_eq!(state.active_piece.name, "I");
    assert_eq!(state.active_piece_row, 1);
    assert_eq!(state.score, 0);
    assert!(state.held_piece.is_none());
    assert_eq!(state.get_piece_previews().map(|piece| piece.name), previews);
}
