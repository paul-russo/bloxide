use super::{GameInput, GameState, RotationDirection, LOCK_DELAY_TICKS, RESET_MOVES};
use crate::{
    block::Block,
    grid::{Grid, FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, GRID_COUNT_ROWS},
    high_score_manager::HighScoreManager,
    piece::{pieces, Piece},
};
use macroquad::prelude::WHITE;
use std::time::Duration;

const DIRECTIONS: [RotationDirection; 2] = [
    RotationDirection::Clockwise,
    RotationDirection::Counterclockwise,
];

fn rotation_input(direction: RotationDirection) -> GameInput {
    match direction {
        RotationDirection::Clockwise => GameInput {
            rotate_right: true,
            ..Default::default()
        },
        RotationDirection::Counterclockwise => GameInput {
            rotate_left: true,
            ..Default::default()
        },
    }
}

fn posed_state(
    high_scores: &HighScoreManager,
    piece: Piece,
    orientation: usize,
    row: isize,
    col: isize,
) -> GameState<'_> {
    let mut state = GameState::new(high_scores);
    state.set_active_piece_and_reset_state(piece);
    state.active_piece_orientation = orientation;
    state.active_piece_row = row;
    state.active_piece_col = col;
    state.refresh_cached_blocks();
    state.lowest_piece_row = state.lock_reference_row();
    state.update_with_elapsed(Duration::ZERO, GameInput::default());

    assert!(!state.is_game_over);
    assert!(!state.collide(None, None, None));

    state
}

fn floor_state(high_scores: &HighScoreManager, piece: Piece) -> GameState<'_> {
    let mut state = posed_state(
        high_scores,
        piece,
        0,
        piece.get_initial_row(),
        piece.get_initial_col(),
    );
    state.active_piece_row = state.grid_locked.find_landing_row(
        state.active_piece_row,
        state.active_piece_col,
        &state.cached_blocks,
        state.cached_bounds_height,
        state.cached_bounds_width,
    );
    state.piece_dirty = true;
    state.update_with_elapsed(Duration::ZERO, GameInput::default());

    assert!(state.has_touched_ground);

    state
}

fn cells(grid: &Grid) -> Vec<(isize, isize)> {
    (0..GRID_COUNT_ROWS)
        .flat_map(|row| {
            (0..GRID_COUNT_COLS)
                .filter(move |&col| grid.has_block_at_cell(row, col))
                .map(move |col| (row as isize, col as isize))
        })
        .collect()
}

fn rotated_cells(
    cells: &[(isize, isize)],
    doubled_pivot: (isize, isize),
    direction: RotationDirection,
) -> Vec<(isize, isize)> {
    let sign = match direction {
        RotationDirection::Clockwise => 1,
        RotationDirection::Counterclockwise => -1,
    };
    let mut rotated: Vec<_> = cells
        .iter()
        .map(|&(row, col)| {
            // Doubled coordinates keep the I/O half-cell pivots exact.
            let rotated_row = doubled_pivot.0 + sign * (2 * col - doubled_pivot.1);
            let rotated_col = doubled_pivot.1 - sign * (2 * row - doubled_pivot.0);
            assert_eq!(rotated_row.rem_euclid(2), 0);
            assert_eq!(rotated_col.rem_euclid(2), 0);

            (rotated_row / 2, rotated_col / 2)
        })
        .collect();
    rotated.sort_unstable();

    rotated
}

#[test]
fn every_piece_rotates_both_ways_about_its_srs_pivot_and_round_trips() {
    let high_scores = HighScoreManager::new();
    let base_row = FIRST_VISIBLE_ROW_ID as isize + 6;
    let mut transitions = 0;

    for piece in [
        pieces::I,
        pieces::J,
        pieces::L,
        pieces::O,
        pieces::S,
        pieces::T,
        pieces::Z,
    ] {
        let base_col = piece.get_initial_col();
        let doubled_pivot = match piece.name {
            "I" => (2 * base_row + 5, 2 * base_col + 5),
            "O" => (2 * base_row + 1, 2 * base_col + 3),
            _ => (2 * base_row + 2, 2 * base_col + 2),
        };

        for orientation in 0..4 {
            let offset = piece.orientations[orientation].offsets[0];

            for direction in DIRECTIONS {
                let mut state = posed_state(
                    &high_scores,
                    piece,
                    orientation,
                    base_row + offset.1,
                    base_col - offset.0,
                );
                let original = state;
                let before = cells(&state.grid_active);
                let expected = rotated_cells(&before, doubled_pivot, direction);
                let (next_orientation, inverse) = match direction {
                    RotationDirection::Clockwise => {
                        ((orientation + 1) % 4, RotationDirection::Counterclockwise)
                    }
                    RotationDirection::Counterclockwise => {
                        ((orientation + 3) % 4, RotationDirection::Clockwise)
                    }
                };

                state.update_with_elapsed(Duration::ZERO, rotation_input(direction));

                assert_eq!(
                    cells(&state.grid_active),
                    expected,
                    "{} {orientation} {direction:?}",
                    piece.name
                );
                assert_eq!(state.active_piece_orientation, next_orientation);
                assert_eq!(state.lock_reference_row(), base_row);
                assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
                assert!(!state.has_touched_ground);

                state.update_with_elapsed(Duration::ZERO, rotation_input(inverse));

                assert_eq!(cells(&state.grid_active), before);
                assert_eq!(state.active_piece_orientation, orientation);
                assert_eq!(state.active_piece_row, original.active_piece_row);
                assert_eq!(state.active_piece_col, original.active_piece_col);
                assert_eq!(state.tick, 0);
                transitions += 1;
            }
        }
    }

    assert_eq!(transitions, 56);
}

#[test]
fn both_directions_use_their_own_t_and_i_wall_kicks() {
    use RotationDirection::{Clockwise as Cw, Counterclockwise as Ccw};

    let high_scores = HighScoreManager::new();
    let row = FIRST_VISIBLE_ROW_ID as isize + 6;

    for (piece, from, col, direction, to, row_delta, expected_col) in [
        (pieces::T, 1, -1, Cw, 2, 0, 0),
        (pieces::T, 1, -1, Ccw, 0, 0, 0),
        (pieces::T, 3, 8, Cw, 0, 0, 7),
        (pieces::T, 3, 8, Ccw, 2, 0, 7),
        (pieces::I, 1, -2, Cw, 2, 1, 0),
        (pieces::I, 1, -2, Ccw, 0, 0, -1),
        (pieces::I, 3, 7, Cw, 0, -1, 5),
        (pieces::I, 3, 7, Ccw, 2, 0, 6),
    ] {
        let mut state = posed_state(&high_scores, piece, from, row, col);

        state.update_with_elapsed(Duration::ZERO, rotation_input(direction));

        assert_eq!(
            state.active_piece_orientation, to,
            "{} {direction:?}",
            piece.name
        );
        assert_eq!(state.active_piece_row, row + row_delta);
        assert_eq!(state.active_piece_col, expected_col);
        assert!(!state.collide(None, None, None));
        assert_eq!(cells(&state.grid_active).len(), 4);
        assert_eq!(state.score, 0);
    }
}

#[test]
fn both_directions_use_distinct_t_and_i_floor_kicks() {
    use RotationDirection::{Clockwise as Cw, Counterclockwise as Ccw};

    let high_scores = HighScoreManager::new();

    for (piece, direction, orientation, row_delta, col_delta) in [
        (pieces::T, Cw, 1, -1, -1),
        (pieces::T, Ccw, 3, -1, 1),
        (pieces::I, Cw, 1, -2, 2),
        (pieces::I, Ccw, 3, -1, -1),
    ] {
        let mut state = floor_state(&high_scores, piece);
        let row = state.active_piece_row;
        let col = state.active_piece_col;

        state.update_with_elapsed(Duration::ZERO, rotation_input(direction));

        assert_eq!(state.active_piece_orientation, orientation);
        assert_eq!(state.active_piece_row, row + row_delta);
        assert_eq!(state.active_piece_col, col + col_delta);
        assert!(state.collide(Some(state.active_piece_row + 1), None, None));
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - 1);
        assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
        assert!(cells(&state.grid_locked).is_empty());
    }
}

fn ceiling_kick_state(
    high_scores: &HighScoreManager,
    row: isize,
    direction: RotationDirection,
) -> GameState<'_> {
    let (orientation, columns) = match direction {
        RotationDirection::Clockwise => (1, [3, 6, 6]),
        RotationDirection::Counterclockwise => (3, [5, 2, 2]),
    };
    let mut state = posed_state(high_scores, pieces::T, orientation, row, 3);

    for (block_row, col) in [
        (row + 1, columns[0]),
        (row + 1, columns[1]),
        (row + 2, columns[2]),
    ] {
        state
            .grid_locked
            .set_cell(block_row as usize, col, Some(Block::new(WHITE)));
    }

    state.update_with_elapsed(Duration::ZERO, GameInput::default());

    assert!(!state.collide(None, None, None));

    state
}

#[test]
fn both_directions_can_kick_into_the_hidden_buffer() {
    let high_scores = HighScoreManager::new();
    let row = FIRST_VISIBLE_ROW_ID as isize - 2;

    for direction in DIRECTIONS {
        let mut state = ceiling_kick_state(&high_scores, row, direction);

        state.update_with_elapsed(Duration::ZERO, rotation_input(direction));

        assert_eq!(state.active_piece_orientation, 2);
        assert_eq!(state.active_piece_row, row - 2);
        assert_eq!(state.active_piece_col, 3);
        assert!(state.check_for_lock_out());
        assert!(!state.is_game_over);
        assert!(!state.collide(None, None, None));
        assert_eq!(cells(&state.grid_active).len(), 4);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
    }
}

#[test]
fn neither_direction_can_kick_through_the_actual_buffer_ceiling() {
    let high_scores = HighScoreManager::new();

    for direction in DIRECTIONS {
        let mut state = ceiling_kick_state(&high_scores, 0, direction);
        let orientation = state.active_piece_orientation;
        let before = cells(&state.grid_active);

        state.update_with_elapsed(Duration::ZERO, rotation_input(direction));

        assert_eq!(state.active_piece_orientation, orientation);
        assert_eq!(state.active_piece_row, 0);
        assert_eq!(state.active_piece_col, 3);
        assert_eq!(cells(&state.grid_active), before);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
        assert!(!state.is_game_over);
    }
}

#[test]
fn counterclockwise_is_not_emulated_by_three_clockwise_turns() {
    let high_scores = HighScoreManager::new();
    let row = FIRST_VISIBLE_ROW_ID as isize + 6;
    let mut initial = posed_state(&high_scores, pieces::J, 0, row, 3);
    initial
        .grid_locked
        .set_cell(row as usize, 5, Some(Block::new(WHITE)));
    let mut counterclockwise = initial;
    let mut clockwise = initial;

    counterclockwise.update_with_elapsed(
        Duration::ZERO,
        rotation_input(RotationDirection::Counterclockwise),
    );

    for _ in 0..3 {
        clockwise.update_with_elapsed(Duration::ZERO, rotation_input(RotationDirection::Clockwise));
    }

    assert_eq!(counterclockwise.active_piece_orientation, 3);
    assert_eq!(clockwise.active_piece_orientation, 3);
    assert_eq!(counterclockwise.active_piece_col, 3);
    assert_eq!(clockwise.active_piece_col, 2);
    assert_ne!(
        cells(&counterclockwise.grid_active),
        cells(&clockwise.grid_active)
    );
}

#[test]
fn a_blocked_counterclockwise_turn_does_not_reset_lock_delay() {
    let high_scores = HighScoreManager::new();
    let mut state = floor_state(&high_scores, pieces::T);
    state.active_piece_col = GRID_COUNT_COLS as isize - 3;
    state.piece_dirty = true;
    state.grid_locked.set_cell(
        GRID_COUNT_ROWS - 3,
        GRID_COUNT_COLS - 1,
        Some(Block::new(WHITE)),
    );
    state.update_with_elapsed(Duration::from_millis(250), GameInput::default());
    let before = cells(&state.grid_active);

    state.update_with_elapsed(
        Duration::ZERO,
        rotation_input(RotationDirection::Counterclockwise),
    );

    assert_eq!(cells(&state.grid_active), before);
    assert_eq!(state.active_piece_orientation, 0);
    assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS / 2);
    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
}

#[test]
fn either_rotation_press_runs_once_with_zero_or_many_elapsed_ticks() {
    let high_scores = HighScoreManager::new();

    for direction in DIRECTIONS {
        for (elapsed, fallen_rows) in [
            (Duration::ZERO, 0),
            (Duration::from_millis(1), 0),
            (Duration::from_millis(200), 12),
        ] {
            let mut state = posed_state(
                &high_scores,
                pieces::T,
                0,
                pieces::T.get_initial_row(),
                pieces::T.get_initial_col(),
            );
            state.rows_cleared = 190;
            let orientation = match direction {
                RotationDirection::Clockwise => 1,
                RotationDirection::Counterclockwise => 3,
            };

            state.update_with_elapsed(elapsed, rotation_input(direction));

            assert_eq!(state.active_piece_orientation, orientation);
            assert_eq!(
                state.active_piece_row,
                pieces::T.get_initial_row() + fallen_rows
            );
            assert!(!state.held_input.rotate_left);
            assert!(!state.held_input.rotate_right);

            state.update_with_elapsed(Duration::from_millis(50), GameInput::default());

            assert_eq!(state.active_piece_orientation, orientation);
            assert_eq!(
                state.active_piece_row,
                pieces::T.get_initial_row() + fallen_rows + 3
            );
            assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
        }
    }
}

#[test]
fn opposite_rotation_presses_cancel_without_spending_a_lock_reset() {
    let high_scores = HighScoreManager::new();
    let mut state = floor_state(&high_scores, pieces::O);
    state.update_with_elapsed(Duration::from_millis(250), GameInput::default());
    let before = cells(&state.grid_active);

    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            rotate_left: true,
            rotate_right: true,
            ..Default::default()
        },
    );

    assert_eq!(state.active_piece_orientation, 0);
    assert_eq!(cells(&state.grid_active), before);
    assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS / 2);
    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
}

#[test]
fn paused_counterclockwise_presses_are_not_replayed_on_resume() {
    let high_scores = HighScoreManager::new();
    let mut state = posed_state(
        &high_scores,
        pieces::T,
        0,
        pieces::T.get_initial_row(),
        pieces::T.get_initial_col(),
    );
    state.toggle_pause();

    state.update_with_elapsed(
        Duration::from_secs(10),
        rotation_input(RotationDirection::Counterclockwise),
    );

    assert!(state.is_paused);
    assert_eq!(state.active_piece_orientation, 0);
    assert!(!state.held_input.rotate_left);

    state.update_with_elapsed(
        Duration::from_secs(10),
        GameInput {
            toggle_pause: true,
            ..Default::default()
        },
    );
    state.update_with_elapsed(Duration::from_millis(200), GameInput::default());

    assert!(!state.is_paused);
    assert_eq!(state.active_piece_orientation, 0);
    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
}

#[test]
fn counterclockwise_o_spins_preserve_position_and_exhaust_the_lock_budget() {
    let high_scores = HighScoreManager::new();
    let mut state = floor_state(&high_scores, pieces::O);
    let before = cells(&state.grid_active);
    let next_piece = state.bag_manager.peek(1);

    for rotations in 1..RESET_MOVES {
        state.update_with_elapsed(
            Duration::ZERO,
            rotation_input(RotationDirection::Counterclockwise),
        );

        assert_eq!(cells(&state.grid_active), before);
        assert_eq!(
            state.active_piece_orientation,
            (4 - rotations as usize % 4) % 4
        );
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - rotations);
        assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
        assert!(cells(&state.grid_locked).is_empty());
    }

    state.update_with_elapsed(
        Duration::ZERO,
        rotation_input(RotationDirection::Counterclockwise),
    );

    assert_eq!(cells(&state.grid_locked), before);
    assert_eq!(state.active_piece.name, next_piece.name);
    assert_eq!(state.active_piece_row, next_piece.get_initial_row());
    assert_eq!(state.active_piece_orientation, 0);
    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
    assert_eq!(state.score, 0);
}
