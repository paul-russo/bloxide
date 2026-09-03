use super::{
    GameInput, GameState, HARD_DROP_TRAIL_TICKS, LEVEL_FLARE_TICKS, LINE_CLEAR_EFFECT_TICKS,
    LOCK_DELAY_TICKS, LOCK_IMPACT_TICKS, REPEAT_DELAY_TICKS, REPEAT_INTERVAL_TICKS, RESET_MOVES,
    TICKS_PER_SECOND,
};
use crate::{
    block::Block,
    grid::{Grid, GRID_COUNT_COLS, GRID_COUNT_ROWS},
    high_score_manager::HighScoreManager,
    piece::{pieces, Piece},
    render3d::{LavaSplash, ShrapnelVoxel, LAVA_Y},
};
use macroquad::prelude::{Vec3, WHITE};
use std::time::Duration;

const FRAME_RATES: [u32; 5] = [240, 120, 60, 30, 15];

fn state_with_piece(high_scores: &HighScoreManager, piece: Piece) -> GameState<'_> {
    let mut state = GameState::new(high_scores);
    state.set_active_piece_and_reset_state(piece);

    state
}

fn state_on_floor(
    high_scores: &HighScoreManager,
    piece: Piece,
    orientation: usize,
) -> GameState<'_> {
    let mut state = state_with_piece(high_scores, piece);
    state.active_piece_orientation = orientation;
    state.refresh_cached_blocks();
    state.active_piece_row = state.grid_locked.find_landing_row(
        state.active_piece_row,
        state.active_piece_col,
        &state.cached_blocks,
        state.cached_bounds_height,
        state.cached_bounds_width,
    );
    state.update_with_elapsed(Duration::ZERO, GameInput::default());

    assert!(state.has_touched_ground);

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

fn occupied_positions(grid: &Grid) -> Vec<(usize, usize)> {
    (0..GRID_COUNT_ROWS)
        .flat_map(|row| {
            (0..GRID_COUNT_COLS)
                .filter(move |&col| grid.has_block_at_cell(row, col))
                .map(move |col| (row, col))
        })
        .collect()
}

fn block_next_spawn(state: &mut GameState<'_>) -> Piece {
    let next_piece = state.bag_manager.peek(1);
    let (canvas, height, width) = next_piece.get_blocks(0);
    let (row, col) = (0..height)
        .find_map(|row| {
            (0..width)
                .find(|&col| canvas[row][col].is_some())
                .map(|col| (row + 1, col + next_piece.get_initial_col() as usize))
        })
        .expect("every piece has an occupied spawn cell");
    state
        .grid_locked
        .set_cell(row, col, Some(Block::new(WHITE)));

    next_piece
}

fn duration_for_ticks(ticks: u64) -> Duration {
    Duration::from_nanos((ticks * 1_000_000_000).div_ceil(u64::from(TICKS_PER_SECOND)))
}

fn advance_at_fps(state: &mut GameState<'_>, elapsed: Duration, fps: u32, held_input: GameInput) {
    state.update_with_elapsed(Duration::ZERO, held_input);

    // Derive each frame boundary from absolute integer time, rather than
    // repeatedly rounding a fractional frame duration.
    let frame_count = (elapsed.as_nanos() * u128::from(fps) / 1_000_000_000) as u64;
    let mut previous = Duration::ZERO;

    for frame in 1..=frame_count {
        let time = Duration::from_nanos(frame * 1_000_000_000 / u64::from(fps));
        state.update_with_elapsed(time - previous, held_input);
        previous = time;
    }

    state.update_with_elapsed(elapsed - previous, held_input);
}

fn assert_matching_motion(actual: &GameState<'_>, expected: &GameState<'_>) {
    assert_eq!(
        (actual.tick, actual.tick_accumulator),
        (expected.tick, expected.tick_accumulator)
    );
    assert_eq!(
        (
            actual.active_piece.name,
            actual.active_piece_row,
            actual.active_piece_col,
            actual.active_piece_orientation,
        ),
        (
            expected.active_piece.name,
            expected.active_piece_row,
            expected.active_piece_col,
            expected.active_piece_orientation,
        )
    );
    assert_eq!(actual.fall_progress, expected.fall_progress);
    assert_eq!(actual.ticks_to_repeat, expected.ticks_to_repeat);
    assert_eq!(actual.ticks_to_lock, expected.ticks_to_lock);
    assert_eq!(
        actual.lock_reset_moves_remaining,
        expected.lock_reset_moves_remaining
    );
    assert_eq!(actual.has_touched_ground, expected.has_touched_ground);
    assert_eq!(actual.lowest_piece_row, expected.lowest_piece_row);
    assert_eq!(actual.score, expected.score);
    assert_eq!(actual.cached_ghost_row, expected.cached_ghost_row);
}

#[test]
fn blocked_hold_stops_before_same_update_hard_drop_and_ghost_projection() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::I);
    state.held_piece = Some(pieces::T);
    state.grid_locked.set_cell(1, 4, Some(Block::new(WHITE)));

    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            hold_piece: true,
            hard_drop: true,
            ..Default::default()
        },
    );

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

    let next_piece = block_next_spawn(&mut state);

    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            hard_drop: true,
            shift_left: true,
            ..Default::default()
        },
    );

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

    state.update_with_elapsed(
        Duration::from_secs(10),
        GameInput {
            soft_drop: true,
            shift_left: true,
            shift_right: true,
            rotate_right: true,
            hard_drop: true,
            hold_piece: true,
            toggle_pause: true,
        },
    );

    assert!(state.is_game_over);
    assert!(!state.is_paused);
    assert_eq!(state.active_piece.name, "I");
    assert_eq!(state.active_piece_row, 1);
    assert_eq!(state.score, 0);
    assert!(state.held_piece.is_none());
    assert_eq!(state.get_piece_previews().map(|piece| piece.name), previews);
}

#[test]
fn levels_advance_at_each_ten_line_boundary_and_stop_at_twenty() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);

    for (lines, level) in [
        (0, 1),
        (1, 1),
        (9, 1),
        (10, 2),
        (11, 2),
        (19, 2),
        (20, 3),
        (21, 3),
        (189, 19),
        (190, 20),
        (191, 20),
        (200, 20),
        (usize::MAX, 20),
    ] {
        state.rows_cleared = lines;
        assert_eq!(state.get_level(), level, "after {lines} cleared lines");
    }
}

#[test]
fn tenth_line_levels_up_and_preserves_pre_clear_scoring() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    state.rows_cleared = 9;

    for col in 0..GRID_COUNT_COLS {
        state.grid_locked.set_cell(21, col, Some(Block::new(WHITE)));
    }

    state.clear_filled_rows_and_update_score();

    assert_eq!(state.get_rows_cleared(), 10);
    assert_eq!(state.get_level(), 2);
    assert_eq!(state.get_score(), 100);
    assert_eq!(state.get_level_flare(), 1.0);

    for col in 0..GRID_COUNT_COLS {
        state.grid_locked.set_cell(21, col, Some(Block::new(WHITE)));
    }

    state.clear_filled_rows_and_update_score();

    assert_eq!(state.get_rows_cleared(), 11);
    assert_eq!(state.get_level(), 2);
    assert_eq!(state.get_score(), 300);
}

#[test]
fn gravity_and_held_input_match_across_render_cadences_and_irregular_frames() {
    let high_scores = HighScoreManager::new();
    let elapsed = Duration::from_millis(200);

    for (lines, soft_drop, rows) in [
        (0, false, 0),
        (90, false, 3),
        (120, false, 11),
        (190, false, 12),
        (0, true, 6),
        (90, true, 6),
        (120, true, 11),
        (190, true, 12),
    ] {
        let mut initial = state_with_piece(&high_scores, pieces::O);
        initial.rows_cleared = lines;
        let held_input = GameInput {
            soft_drop,
            shift_right: true,
            ..Default::default()
        };
        let mut reference = initial;
        reference.update_with_elapsed(Duration::ZERO, held_input);
        reference.update_with_elapsed(elapsed, held_input);

        assert_eq!(reference.tick, 48);
        assert_eq!(reference.tick_accumulator, 0);
        assert_eq!(reference.active_piece_row, 1 + rows);
        assert_eq!(reference.active_piece_col, initial.active_piece_col + 2);
        assert_eq!(reference.score, if soft_drop { rows as usize } else { 0 });

        for fps in FRAME_RATES {
            let mut state = initial;
            advance_at_fps(&mut state, elapsed, fps, held_input);

            assert_matching_motion(&state, &reference);
            assert_eq!(occupied_cells(&state.grid_active), 4);
            assert_eq!(occupied_cells(&state.grid_ghost), 4);
        }

        let mut irregular = initial;
        irregular.update_with_elapsed(Duration::ZERO, held_input);
        let mut remaining = elapsed;

        for part in [
            Duration::from_nanos(1),
            Duration::from_millis(67),
            Duration::from_millis(3),
            Duration::from_nanos(11),
            Duration::from_millis(29),
        ] {
            irregular.update_with_elapsed(part, held_input);
            remaining -= part;
        }

        irregular.update_with_elapsed(remaining, held_input);
        assert_matching_motion(&irregular, &reference);
    }
}

#[test]
fn level_one_falls_at_exact_one_second_boundaries() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);

    state.update_with_elapsed(Duration::from_nanos(999_999_999), GameInput::default());

    assert_eq!(state.tick, 239);
    assert_eq!(state.active_piece_row, 1);

    state.update_with_elapsed(Duration::from_nanos(1), GameInput::default());

    assert_eq!(state.tick, 240);
    assert_eq!(state.tick_accumulator, 0);
    assert_eq!(state.active_piece_row, 2);
    assert_eq!(state.fall_progress, 0.0);

    state.update_with_elapsed(Duration::from_secs(1), GameInput::default());

    assert_eq!(state.tick, 480);
    assert_eq!(state.active_piece_row, 3);
    assert_eq!(state.fall_progress, 0.0);
    assert_eq!(state.score, 0);
}

#[test]
fn fractional_gravity_preserves_the_seconds_per_row_formula_and_one_g_cap() {
    let high_scores = HighScoreManager::new();

    for level in [2, 7, 10, 13, 14, 20] {
        let mut state = state_with_piece(&high_scores, pieces::O);
        state.rows_cleared = (level - 1) * 10;
        let seconds_per_row = (0.8_f64 - (level - 1) as f64 * 0.007).powi(level as i32 - 1);
        let rows_per_second = seconds_per_row.recip().min(60.0);
        let distance = rows_per_second * 0.2;

        state.update_with_elapsed(Duration::from_millis(200), GameInput::default());

        assert!((state.get_gravity() * 60.0 - rows_per_second).abs() < 1e-12);
        assert_eq!(state.active_piece_row, 1 + distance.floor() as isize);
        assert!(
            (state.fall_progress / f64::from(TICKS_PER_SECOND) - distance.fract()).abs() < 1e-10
        );

        if level >= 14 {
            assert_eq!(state.get_gravity(), 1.0);
            assert_eq!(state.active_piece_row, 13);
        }
    }
}

#[test]
fn soft_drop_is_thirty_rows_per_second_unless_natural_gravity_is_faster() {
    let high_scores = HighScoreManager::new();
    let held_input = GameInput {
        soft_drop: true,
        ..Default::default()
    };

    for (lines, ticks_per_row) in [(0, 8), (90, 8), (190, 4)] {
        let mut state = state_with_piece(&high_scores, pieces::O);
        state.rows_cleared = lines;
        state.update_with_elapsed(Duration::ZERO, held_input);
        state.update_with_elapsed(duration_for_ticks(ticks_per_row - 1), held_input);

        assert_eq!(state.active_piece_row, 1);
        assert_eq!(state.score, 0);

        state.update_with_elapsed(duration_for_ticks(1), held_input);

        assert_eq!(state.active_piece_row, 2);
        assert_eq!(state.score, 1);

        state.update_with_elapsed(duration_for_ticks(ticks_per_row * 3), held_input);

        assert_eq!(state.active_piece_row, 5);
        assert_eq!(state.score, 4);
    }
}

#[test]
fn contact_locks_at_exactly_five_hundred_ms_for_natural_soft_and_released_soft_drop() {
    let high_scores = HighScoreManager::new();
    let just_before_lock = Duration::from_millis(500) - Duration::from_nanos(1);

    for (mode, lines, soft_before, soft_after, landing_ticks) in [
        ("natural", 0, false, false, 240),
        ("soft", 0, true, true, 8),
        ("released soft", 0, true, false, 8),
        ("natural 1G", 190, false, false, 4),
        ("soft 1G", 190, true, true, 4),
        ("released soft 1G", 190, true, false, 4),
    ] {
        for fps in FRAME_RATES.into_iter().chain([1]) {
            let mut state = state_with_piece(&high_scores, pieces::O);
            state.active_piece_row = 19;
            state.rows_cleared = lines;
            let landing_input = GameInput {
                soft_drop: soft_before,
                ..Default::default()
            };
            advance_at_fps(
                &mut state,
                duration_for_ticks(landing_ticks),
                fps,
                landing_input,
            );

            assert_eq!(state.tick, landing_ticks as usize, "{mode}, {fps} FPS");
            assert_eq!(state.active_piece_row, 20);
            assert!(state.has_touched_ground);
            assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS, "{mode}, {fps} FPS");
            assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
            assert_eq!(occupied_cells(&state.grid_locked), 0);

            let contact_input = GameInput {
                soft_drop: soft_after,
                ..Default::default()
            };
            advance_at_fps(&mut state, just_before_lock, fps, contact_input);

            assert_eq!(state.tick, landing_ticks as usize + 119);
            assert_eq!(state.ticks_to_lock, 1, "{mode}, {fps} FPS");
            assert_eq!(occupied_cells(&state.grid_locked), 0);

            state.update_with_elapsed(Duration::from_nanos(1), contact_input);

            assert_eq!(state.tick, landing_ticks as usize + 120);
            assert_eq!(occupied_cells(&state.grid_locked), 4, "{mode}, {fps} FPS");
            assert_eq!(state.score, if soft_before { 1 } else { 0 });
            assert_eq!(state.active_piece_row, 1);
            assert_eq!(state.fall_progress, 0.0);
            assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
            assert!(!state.has_touched_ground);
        }
    }
}

#[test]
fn contact_deadlines_match_when_single_or_irregular_frames_span_the_landing() {
    let high_scores = HighScoreManager::new();

    for (soft_drop, landing_ticks) in [(false, 240), (true, 8)] {
        let mut initial = state_with_piece(&high_scores, pieces::O);
        initial.active_piece_row = 19;
        let input = GameInput {
            soft_drop,
            ..Default::default()
        };
        initial.update_with_elapsed(Duration::ZERO, input);
        let deadline = duration_for_ticks(landing_ticks) + Duration::from_millis(500);
        let just_before_lock = deadline - Duration::from_nanos(1);
        let mut single_frame = initial;
        single_frame.update_with_elapsed(just_before_lock, input);

        assert!(single_frame.has_touched_ground);
        assert_eq!(single_frame.ticks_to_lock, 1);
        assert_eq!(occupied_cells(&single_frame.grid_locked), 0);

        let mut irregular = initial;
        let mut previous = Duration::ZERO;

        for boundary in [
            Duration::from_millis(3),
            Duration::from_millis(53),
            Duration::from_millis(211),
            just_before_lock,
        ] {
            irregular.update_with_elapsed(boundary - previous, input);
            previous = boundary;
        }

        assert_matching_motion(&irregular, &single_frame);

        for mut state in [single_frame, irregular] {
            state.update_with_elapsed(Duration::from_nanos(1), input);

            assert_eq!(state.tick, landing_ticks as usize + 120);
            assert_eq!(occupied_cells(&state.grid_locked), 4);
            assert_eq!(state.active_piece_row, 1);
        }
    }
}

#[test]
fn first_contact_from_a_shift_or_rotation_keeps_the_full_delay_and_reset_allowance() {
    let high_scores = HighScoreManager::new();

    for rotate in [false, true] {
        let piece = if rotate { pieces::T } else { pieces::O };
        let mut state = state_with_piece(&high_scores, piece);
        state.active_piece_row = if rotate { 19 } else { 18 };
        state.active_piece_col = if rotate { 3 } else { 0 };

        if !rotate {
            state.grid_locked.set_cell(20, 3, Some(Block::new(WHITE)));
        }

        state.update_with_elapsed(Duration::ZERO, GameInput::default());
        assert!(!state.has_touched_ground);

        state.update_with_elapsed(
            Duration::ZERO,
            GameInput {
                rotate_right: rotate,
                shift_right: !rotate,
                ..Default::default()
            },
        );

        assert!(state.has_touched_ground);
        assert!(state.collide(Some(state.active_piece_row + 1), None, None));
        assert_eq!(state.tick, 0);
        assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);

        state.update_with_elapsed(duration_for_ticks(1), GameInput::default());

        assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS - 1);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
    }
}

#[test]
fn grounded_shifts_and_rotations_restart_the_full_contact_delay() {
    let high_scores = HighScoreManager::new();

    for rotate in [false, true] {
        let mut state = state_on_floor(&high_scores, pieces::O, 0);
        state.update_with_elapsed(Duration::from_millis(250), GameInput::default());

        assert_eq!(state.ticks_to_lock, 60);

        state.update_with_elapsed(
            Duration::ZERO,
            GameInput {
                rotate_right: rotate,
                shift_left: !rotate,
                ..Default::default()
            },
        );

        assert_eq!(state.tick, 60);
        assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - 1);
        assert_eq!(state.lowest_piece_row, 20);
        assert!(state.collide(Some(state.active_piece_row + 1), None, None));

        state.update_with_elapsed(Duration::ZERO, GameInput::default());
        state.update_with_elapsed(
            Duration::from_millis(500) - Duration::from_nanos(1),
            GameInput::default(),
        );

        assert_eq!(state.ticks_to_lock, 1);
        assert_eq!(occupied_cells(&state.grid_locked), 0);

        state.update_with_elapsed(Duration::from_nanos(1), GameInput::default());

        assert_eq!(occupied_cells(&state.grid_locked), 4);
        assert_eq!(state.tick, 180);
    }
}

#[test]
fn grounded_das_resets_after_debiting_the_elapsed_interval() {
    let high_scores = HighScoreManager::new();
    let mut state = state_on_floor(&high_scores, pieces::O, 0);
    let held_input = GameInput {
        shift_right: true,
        ..Default::default()
    };
    state.update_with_elapsed(Duration::ZERO, held_input);
    state.ticks_to_lock = 1;
    state.ticks_to_repeat = 1;
    let before_col = state.active_piece_col;

    state.update_with_elapsed(duration_for_ticks(1), held_input);

    assert_eq!(state.active_piece_col, before_col + 1);
    assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - 2);
    assert_eq!(occupied_cells(&state.grid_locked), 0);

    state.update_with_elapsed(duration_for_ticks(1), GameInput::default());

    assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS - 1);
}

#[test]
fn failed_shifts_and_rotations_do_not_reset_contact_delay_or_budget() {
    let high_scores = HighScoreManager::new();

    for rotate in [false, true] {
        let piece = if rotate { pieces::T } else { pieces::O };
        let mut state = state_on_floor(&high_scores, piece, 0);
        state.active_piece_col = if rotate { 0 } else { 7 };
        state.piece_dirty = true;

        if rotate {
            // This blocks the T's only floor-kick candidate that fits the well.
            state.grid_locked.set_cell(19, 0, Some(Block::new(WHITE)));
        }

        state.update_with_elapsed(Duration::from_millis(250), GameInput::default());
        let before_cells = occupied_positions(&state.grid_active);
        let locked_before = occupied_cells(&state.grid_locked);
        let input = GameInput {
            rotate_right: rotate,
            shift_right: !rotate,
            ..Default::default()
        };
        state.update_with_elapsed(Duration::ZERO, input);

        assert_eq!(occupied_positions(&state.grid_active), before_cells);
        assert_eq!(state.ticks_to_lock, 60);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);

        // Includes a blocked horizontal repeat, or another failed rotation.
        state.update_with_elapsed(duration_for_ticks(44), input);

        assert_eq!(occupied_positions(&state.grid_active), before_cells);
        assert_eq!(state.ticks_to_lock, 16);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
        assert_eq!(occupied_cells(&state.grid_locked), locked_before);

        state.update_with_elapsed(duration_for_ticks(16), GameInput::default());

        assert_eq!(occupied_cells(&state.grid_locked), locked_before + 4);
    }
}

#[test]
fn zero_tick_floor_shifts_and_o_spins_lock_on_the_fifteenth_reset() {
    let high_scores = HighScoreManager::new();

    for orientation in 0..4 {
        for rotate in [false, true] {
            let mut state = state_on_floor(&high_scores, pieces::O, orientation);
            let initial_cells = occupied_positions(&state.grid_active);
            let next_piece = state.bag_manager.peek(1);

            for moves in 1..=RESET_MOVES {
                state.update_with_elapsed(
                    Duration::ZERO,
                    GameInput {
                        rotate_right: rotate,
                        shift_left: !rotate && moves % 2 == 1,
                        shift_right: !rotate && moves % 2 == 0,
                        ..Default::default()
                    },
                );

                assert_eq!(state.tick, 0);

                if moves < RESET_MOVES {
                    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - moves);
                    assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
                    assert_eq!(state.lowest_piece_row, 20);
                    assert_eq!(occupied_cells(&state.grid_locked), 0);

                    if rotate {
                        assert_eq!(
                            state.active_piece_orientation,
                            (orientation + moves as usize) % 4
                        );
                        assert_eq!(occupied_positions(&state.grid_active), initial_cells);
                    }
                }
            }

            assert_eq!(occupied_cells(&state.grid_locked), 4);
            assert_eq!(state.active_piece.name, next_piece.name);
            assert_eq!(state.active_piece_row, 1);
            assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
            assert!(!state.has_touched_ground);

            if rotate {
                assert_eq!(occupied_positions(&state.grid_locked), initial_cells);
            }
        }
    }
}

#[test]
fn airborne_shifts_after_contact_still_spend_the_reset_allowance() {
    let high_scores = HighScoreManager::new();
    let mut state = state_on_floor(&high_scores, pieces::I, 1);
    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            rotate_right: true,
            ..Default::default()
        },
    );

    assert!(!state.collide(Some(state.active_piece_row + 1), None, None));
    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - 1);

    for (moves, shift_left) in [(2, true), (3, false)] {
        state.update_with_elapsed(
            Duration::ZERO,
            GameInput {
                shift_left,
                shift_right: !shift_left,
                ..Default::default()
            },
        );

        assert!(!state.collide(Some(state.active_piece_row + 1), None, None));
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - moves);
        assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
        assert_eq!(state.lowest_piece_row, 17);
        assert_eq!(occupied_cells(&state.grid_locked), 0);
    }
}

#[test]
fn i_rotation_offsets_do_not_refill_or_bypass_an_exhausted_budget() {
    let high_scores = HighScoreManager::new();
    let mut state = state_on_floor(&high_scores, pieces::I, 1);
    let rotate = GameInput {
        rotate_right: true,
        ..Default::default()
    };

    for moves in 1..RESET_MOVES {
        state.update_with_elapsed(Duration::ZERO, rotate);

        assert_eq!(state.active_piece_orientation, (1 + moves as usize) % 4);
        assert_eq!(state.lowest_piece_row, 17);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - moves);
        assert_eq!(occupied_cells(&state.grid_locked), 0);

        if moves == 1 {
            assert_eq!(
                state.active_piece_row, 18,
                "the canvas moves, not the SRS reference"
            );
        }
    }

    assert!(state.collide(Some(state.active_piece_row + 1), None, None));
    state.update_with_elapsed(Duration::from_millis(250), GameInput::default());
    assert_eq!(state.ticks_to_lock, 60);

    // The fifteenth rotation lifts the blocks off the floor without translating
    // the rotation reference. No further move may reset the airborne piece.
    state.update_with_elapsed(Duration::ZERO, rotate);

    assert_eq!(state.active_piece_orientation, 0);
    assert!(!state.collide(Some(state.active_piece_row + 1), None, None));
    assert_eq!(state.lock_reset_moves_remaining, 0);
    assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);

    state.update_with_elapsed(Duration::from_millis(500), GameInput::default());
    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            shift_left: true,
            ..Default::default()
        },
    );

    assert_eq!(state.active_piece_row, 17);
    assert_eq!(state.lowest_piece_row, 17);
    assert_eq!(state.lock_reset_moves_remaining, 0);
    assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
    assert_eq!(occupied_cells(&state.grid_locked), 0);

    let before_tick = state.tick;
    state.update_with_elapsed(Duration::ZERO, rotate);

    assert_eq!(state.tick, before_tick);
    assert_eq!(occupied_cells(&state.grid_locked), 4);
    assert_eq!(state.active_piece_row, 1);
    assert!(!state.has_touched_ground);
}

#[test]
fn t_floor_kick_cycles_exhaust_the_budget_without_refilling_at_the_old_height() {
    let high_scores = HighScoreManager::new();
    let mut state = state_on_floor(&high_scores, pieces::T, 0);
    state.active_piece_col = 0;
    state.piece_dirty = true;
    state.rows_cleared = 190;
    let rotate = GameInput {
        rotate_right: true,
        ..Default::default()
    };

    for moves in 1..RESET_MOVES {
        state.update_with_elapsed(duration_for_ticks(40), rotate);

        assert_eq!(state.active_piece_orientation, moves as usize % 4);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - moves);
        assert_eq!(state.lowest_piece_row, 20);
        assert_eq!(occupied_cells(&state.grid_locked), 0);

        if moves % 4 == 0 {
            assert_eq!(state.active_piece_row, 19);

            state.update_with_elapsed(duration_for_ticks(4), GameInput::default());

            assert_eq!(state.active_piece_row, 20);
            assert_eq!(state.active_piece_col, 0);
            assert_eq!(state.lowest_piece_row, 20);
            assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - moves);
            assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS - 3);
        }
    }

    state.update_with_elapsed(duration_for_ticks(40), rotate);

    assert_eq!(occupied_cells(&state.grid_locked), 4);
    assert_eq!(state.active_piece_row, 1);
    assert_eq!(state.score, 0);
    assert!(!state.is_game_over);
}

#[test]
fn leaving_a_ledge_on_the_fifteenth_move_refills_only_below_the_previous_lowest_row() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    state.active_piece_row = 8;
    state.rows_cleared = 190;
    state.grid_locked.set_cell(10, 4, Some(Block::new(WHITE)));
    state.update_with_elapsed(Duration::ZERO, GameInput::default());

    assert!(state.has_touched_ground);
    assert_eq!(state.lowest_piece_row, 8);

    for _ in 1..RESET_MOVES {
        state.update_with_elapsed(
            Duration::ZERO,
            GameInput {
                rotate_right: true,
                ..Default::default()
            },
        );
    }

    assert_eq!(state.lock_reset_moves_remaining, 1);

    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            shift_right: true,
            ..Default::default()
        },
    );

    assert!(!state.collide(Some(state.active_piece_row + 1), None, None));
    assert_eq!(state.lowest_piece_row, 8);
    assert_eq!(state.lock_reset_moves_remaining, 0);
    assert_eq!(occupied_cells(&state.grid_locked), 1);

    let first_row_time = duration_for_ticks(4);
    state.update_with_elapsed(first_row_time, GameInput::default());

    assert_eq!(state.lowest_piece_row, 9);
    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
    assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
    assert!(state.has_touched_ground);
    assert!(!state.collide(Some(state.active_piece_row + 1), None, None));

    state.update_with_elapsed(
        Duration::from_millis(200) - first_row_time,
        GameInput::default(),
    );

    assert_eq!(state.lowest_piece_row, 20);
    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
    assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
    assert!(state.collide(Some(state.active_piece_row + 1), None, None));
    assert_eq!(occupied_cells(&state.grid_locked), 1);

    state.update_with_elapsed(
        Duration::from_millis(500) - Duration::from_nanos(1),
        GameInput::default(),
    );

    assert_eq!(state.ticks_to_lock, 1);
    assert_eq!(occupied_cells(&state.grid_locked), 1);

    state.update_with_elapsed(Duration::from_nanos(1), GameInput::default());

    assert_eq!(occupied_cells(&state.grid_locked), 5);
    assert_eq!(state.score, 0);
}

#[test]
fn pause_freezes_contact_delay_and_reset_budget() {
    let high_scores = HighScoreManager::new();
    let mut state = state_on_floor(&high_scores, pieces::O, 0);
    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            shift_left: true,
            ..Default::default()
        },
    );
    state.update_with_elapsed(Duration::ZERO, GameInput::default());
    state.update_with_elapsed(
        Duration::from_millis(250),
        GameInput {
            toggle_pause: true,
            ..Default::default()
        },
    );

    assert_eq!(state.ticks_to_lock, 60);
    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES - 1);

    let frozen = state;
    state.update_with_elapsed(
        Duration::from_secs(60),
        GameInput {
            soft_drop: true,
            shift_left: true,
            rotate_right: true,
            hard_drop: true,
            hold_piece: true,
            ..Default::default()
        },
    );

    assert!(state.is_paused);
    assert!(state.held_piece.is_none());
    assert_matching_motion(&state, &frozen);

    state.update_with_elapsed(
        Duration::from_secs(3600),
        GameInput {
            toggle_pause: true,
            ..Default::default()
        },
    );

    assert!(!state.is_paused);
    assert_matching_motion(&state, &frozen);

    state.update_with_elapsed(
        Duration::from_millis(250) - Duration::from_nanos(1),
        GameInput::default(),
    );

    assert_eq!(state.ticks_to_lock, 1);
    assert_eq!(occupied_cells(&state.grid_locked), 0);

    state.update_with_elapsed(Duration::from_nanos(1), GameInput::default());

    assert_eq!(state.tick, 120);
    assert_eq!(occupied_cells(&state.grid_locked), 4);
}

#[test]
fn hard_drop_locks_immediately_during_a_fresh_or_partially_spent_contact_delay() {
    let high_scores = HighScoreManager::new();

    for elapsed in [Duration::ZERO, Duration::from_millis(250)] {
        let mut state = state_on_floor(&high_scores, pieces::O, 0);
        state.update_with_elapsed(elapsed, GameInput::default());
        let before_tick = state.tick;
        let next_piece = state.bag_manager.peek(1);

        assert!(state.ticks_to_lock > 0);

        state.update_with_elapsed(
            Duration::ZERO,
            GameInput {
                hard_drop: true,
                ..Default::default()
            },
        );

        assert_eq!(state.tick, before_tick);
        assert_eq!(occupied_cells(&state.grid_locked), 4);
        assert_eq!(state.active_piece.name, next_piece.name);
        assert_eq!(state.active_piece_row, 1);
        assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
        assert!(!state.has_touched_ground);
        assert_eq!(state.score, 0);
    }
}

#[test]
fn sampled_rotation_after_the_contact_deadline_applies_only_to_the_successor() {
    let high_scores = HighScoreManager::new();
    let mut state = state_on_floor(&high_scores, pieces::O, 0);
    let next_piece = state.bag_manager.peek(1);
    state.update_with_elapsed(
        Duration::from_millis(500),
        GameInput {
            rotate_right: true,
            ..Default::default()
        },
    );

    assert_eq!(state.tick, 120);
    assert_eq!(occupied_cells(&state.grid_locked), 4);
    assert_eq!(state.active_piece.name, next_piece.name);
    assert_eq!(state.active_piece_orientation, 1);
    assert_eq!(state.fall_progress, 0.0);
    assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
    assert!(!state.has_touched_ground);
    assert_eq!(state.score, 0);
}

#[test]
fn exhausted_instant_actions_stop_at_top_out_before_following_input() {
    let high_scores = HighScoreManager::new();

    for rotate in [false, true] {
        let mut state = state_on_floor(&high_scores, pieces::O, 0);

        for _ in 1..RESET_MOVES {
            state.update_with_elapsed(
                Duration::ZERO,
                GameInput {
                    rotate_right: true,
                    ..Default::default()
                },
            );
        }

        let next_piece = block_next_spawn(&mut state);
        state.update_with_elapsed(
            Duration::ZERO,
            GameInput {
                rotate_right: rotate,
                hard_drop: rotate,
                shift_left: true,
                ..Default::default()
            },
        );

        assert!(state.is_game_over);
        assert_eq!(state.tick, 0);
        assert_eq!(state.active_piece.name, next_piece.name);
        assert_eq!(state.active_piece_row, 1);
        assert_eq!(state.active_piece_col, next_piece.get_initial_col());
        assert_eq!(state.active_piece_orientation, 0);
        assert_eq!(state.score, 0);
        assert_eq!(occupied_cells(&state.grid_locked), 5);
        assert_eq!(occupied_cells(&state.grid_active), 0);
        assert_eq!(occupied_cells(&state.grid_ghost), 0);
    }
}

#[test]
fn das_and_arr_fire_at_the_same_ticks_at_every_frame_rate() {
    let high_scores = HighScoreManager::new();
    let held_input = GameInput {
        shift_right: true,
        ..Default::default()
    };

    for (ticks, moves, repeat_remaining) in [
        (0, 1, 44),
        (43, 1, 1),
        (44, 2, 16),
        (59, 2, 1),
        (60, 3, 16),
        (75, 3, 1),
        (76, 4, 16),
    ] {
        for fps in FRAME_RATES {
            let mut state = state_with_piece(&high_scores, pieces::O);
            let initial_col = state.active_piece_col;
            advance_at_fps(&mut state, duration_for_ticks(ticks), fps, held_input);

            assert_eq!(state.tick, ticks as usize);
            assert_eq!(state.active_piece_col, initial_col + moves);
            assert_eq!(state.ticks_to_repeat, repeat_remaining);
            assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
            assert!(!state.has_touched_ground);
        }
    }
}

#[test]
fn new_held_keys_and_releases_are_not_applied_to_time_before_the_sample() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    let initial_col = state.active_piece_col;

    state.update_with_elapsed(
        Duration::from_millis(200),
        GameInput {
            soft_drop: true,
            shift_left: true,
            ..Default::default()
        },
    );

    assert_eq!(state.active_piece_row, 1);
    assert_eq!(state.active_piece_col, initial_col - 1);
    assert_eq!(state.ticks_to_repeat, REPEAT_DELAY_TICKS);
    assert_eq!(state.score, 0);

    // The release is sampled after this interval, which still uses held keys.
    state.update_with_elapsed(Duration::from_millis(200), GameInput::default());

    assert_eq!(state.active_piece_row, 7);
    assert_eq!(state.active_piece_col, initial_col - 2);
    assert_eq!(state.score, 6);

    state.update_with_elapsed(Duration::from_millis(200), GameInput::default());

    assert_eq!(state.active_piece_row, 7);
    assert_eq!(state.active_piece_col, initial_col - 2);
    assert_eq!(state.score, 6);
}

#[test]
fn held_repeat_continues_across_a_hard_drop_without_restarting_das() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    state.active_piece_row = 20;
    let held_input = GameInput {
        shift_left: true,
        ..Default::default()
    };
    state.update_with_elapsed(Duration::ZERO, held_input);
    state.update_with_elapsed(
        duration_for_ticks(44),
        GameInput {
            hard_drop: true,
            ..held_input
        },
    );

    let spawn_col = state.active_piece.get_initial_col();
    assert_eq!(state.active_piece_col, spawn_col);
    assert_eq!(state.ticks_to_repeat, REPEAT_INTERVAL_TICKS);
    assert_eq!(occupied_cells(&state.grid_locked), 4);
    assert_eq!(state.score, 0);

    state.update_with_elapsed(duration_for_ticks(15), held_input);

    assert_eq!(state.active_piece_col, spawn_col);
    assert_eq!(state.ticks_to_repeat, 1);

    state.update_with_elapsed(duration_for_ticks(1), held_input);

    assert_eq!(state.active_piece_col, spawn_col - 1);
    assert_eq!(state.ticks_to_repeat, REPEAT_INTERVAL_TICKS);
}

#[test]
fn rotation_presses_execute_once_with_zero_or_many_elapsed_ticks() {
    let high_scores = HighScoreManager::new();

    for (elapsed, fallen_rows) in [
        (Duration::ZERO, 0),
        (Duration::from_millis(1), 0),
        (Duration::from_millis(200), 12),
    ] {
        let mut state = state_with_piece(&high_scores, pieces::T);
        state.rows_cleared = 190;
        state.update_with_elapsed(
            elapsed,
            GameInput {
                rotate_right: true,
                ..Default::default()
            },
        );

        assert_eq!(state.active_piece_orientation, 1);
        assert_eq!(state.active_piece_row, 1 + fallen_rows);
        assert_eq!(state.lock_reset_moves_remaining, RESET_MOVES);
        assert!(!state.has_touched_ground);

        state.update_with_elapsed(Duration::ZERO, GameInput::default());
        state.update_with_elapsed(Duration::from_millis(50), GameInput::default());

        assert_eq!(state.active_piece_orientation, 1);
        assert_eq!(state.active_piece_row, 4 + fallen_rows);
        assert_eq!(occupied_cells(&state.grid_locked), 0);
    }
}

#[test]
fn hold_presses_execute_after_catch_up_without_aging_the_replacement() {
    let high_scores = HighScoreManager::new();

    for elapsed in [
        Duration::ZERO,
        Duration::from_millis(1),
        Duration::from_millis(200),
    ] {
        let mut state = state_with_piece(&high_scores, pieces::I);
        state.rows_cleared = 190;
        state.held_piece = Some(pieces::T);
        state.update_with_elapsed(
            elapsed,
            GameInput {
                hold_piece: true,
                ..Default::default()
            },
        );

        assert_eq!(state.active_piece.name, "T");
        assert_eq!(state.active_piece_row, 1);
        assert_eq!(state.fall_progress, 0.0);
        assert_eq!(state.held_piece.unwrap().name, "I");
        assert!(state.last_piece_swapped);

        state.update_with_elapsed(Duration::ZERO, GameInput::default());
        state.update_with_elapsed(Duration::from_millis(50), GameInput::default());

        assert_eq!(state.active_piece.name, "T");
        assert_eq!(state.active_piece_row, 4);
        assert_eq!(state.held_piece.unwrap().name, "I");
        assert_eq!(state.score, 0);
    }
}

#[test]
fn hard_drop_presses_execute_once_after_catch_up_and_leave_a_fresh_successor() {
    let high_scores = HighScoreManager::new();

    for (elapsed, drop_start_row) in [
        (Duration::ZERO, 1),
        (Duration::from_millis(1), 1),
        (Duration::from_millis(200), 13),
    ] {
        let mut state = state_with_piece(&high_scores, pieces::O);
        state.rows_cleared = 190;
        let next_piece = state.bag_manager.peek(1);
        state.update_with_elapsed(
            elapsed,
            GameInput {
                hard_drop: true,
                ..Default::default()
            },
        );

        let (trail, strength) = state.get_hard_drop_trail().expect("one hard drop");
        assert_eq!(trail.start_row, drop_start_row);
        assert_eq!(trail.landing_row, 20);
        assert_eq!(strength, 1.0);
        assert_eq!(occupied_cells(&state.grid_locked), 4);
        assert_eq!(state.score, 2 * (20 - drop_start_row) as usize);
        assert_eq!(state.active_piece.name, next_piece.name);
        assert_eq!(state.active_piece_row, 1);
        assert_eq!(state.fall_progress, 0.0);

        state.update_with_elapsed(Duration::ZERO, GameInput::default());
        state.update_with_elapsed(Duration::from_millis(50), GameInput::default());

        assert_eq!(occupied_cells(&state.grid_locked), 4);
        assert_eq!(state.active_piece.name, next_piece.name);
        assert_eq!(state.active_piece_row, 4);
    }
}

#[test]
fn pause_freezes_the_partial_tick_and_resume_discards_all_paused_time() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    let held_input = GameInput {
        shift_right: true,
        ..Default::default()
    };
    state.update_with_elapsed(Duration::ZERO, held_input);
    state.update_with_elapsed(
        Duration::from_millis(180),
        GameInput {
            toggle_pause: true,
            ..held_input
        },
    );

    assert!(state.is_paused);
    assert_eq!(state.tick, 43);
    assert_eq!(state.ticks_to_repeat, 1);

    let frozen = state;
    state.update_with_elapsed(
        Duration::from_secs(60),
        GameInput {
            soft_drop: true,
            shift_left: true,
            rotate_right: true,
            hard_drop: true,
            hold_piece: true,
            ..Default::default()
        },
    );

    assert!(state.is_paused);
    assert!(state.held_piece.is_none());
    assert_matching_motion(&state, &frozen);

    state.update_with_elapsed(
        Duration::from_secs(3600),
        GameInput {
            toggle_pause: true,
            ..held_input
        },
    );

    assert!(!state.is_paused);
    assert_matching_motion(&state, &frozen);

    state.update_with_elapsed(Duration::from_millis(3), held_input);

    assert_eq!(state.tick, frozen.tick);
    assert_eq!(state.active_piece_col, frozen.active_piece_col);

    state.update_with_elapsed(Duration::from_millis(1), held_input);

    assert_eq!(state.tick, frozen.tick + 1);
    assert_eq!(state.active_piece_col, frozen.active_piece_col + 1);
    assert_eq!(state.ticks_to_repeat, REPEAT_INTERVAL_TICKS);
}

#[test]
fn menu_resume_does_not_repeat_a_direction_released_while_paused() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    let held_input = GameInput {
        shift_right: true,
        ..Default::default()
    };
    state.update_with_elapsed(Duration::ZERO, held_input);
    state.update_with_elapsed(
        Duration::from_millis(180),
        GameInput {
            toggle_pause: true,
            ..held_input
        },
    );
    let paused_col = state.active_piece_col;
    assert_eq!(state.ticks_to_repeat, 1);

    state.update_with_elapsed(Duration::from_secs(60), GameInput::default());
    state.toggle_pause();
    state.update_with_elapsed(Duration::from_millis(4), GameInput::default());

    assert!(!state.is_paused);
    assert_eq!(state.tick, 44);
    assert_eq!(state.active_piece_col, paused_col);
}

#[test]
fn menu_resume_preserves_held_das_phase_and_ignores_paused_actions() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    let held_input = GameInput {
        shift_right: true,
        ..Default::default()
    };
    state.update_with_elapsed(Duration::ZERO, held_input);
    state.update_with_elapsed(
        Duration::from_millis(180),
        GameInput {
            toggle_pause: true,
            ..held_input
        },
    );

    assert!(state.is_paused);
    assert_eq!(state.tick, 43);
    assert_eq!(state.ticks_to_repeat, 1);

    let frozen = state;
    state.update_with_elapsed(
        Duration::from_secs(60),
        GameInput {
            rotate_right: true,
            hard_drop: true,
            hold_piece: true,
            ..held_input
        },
    );

    assert!(state.is_paused);
    assert!(state.held_piece.is_none());
    assert_eq!(occupied_cells(&state.grid_locked), 0);
    assert_matching_motion(&state, &frozen);
    assert!(state.held_input.shift_right);
    assert!(!state.held_input.rotate_right);
    assert!(!state.held_input.hard_drop);
    assert!(!state.held_input.hold_piece);
    assert!(!state.held_input.toggle_pause);

    state.toggle_pause();

    assert!(!state.is_paused);
    assert_matching_motion(&state, &frozen);

    state.update_with_elapsed(Duration::from_millis(3), held_input);

    assert_eq!(state.tick, frozen.tick);
    assert_eq!(state.active_piece_col, frozen.active_piece_col);
    assert_eq!(state.ticks_to_repeat, 1);

    state.update_with_elapsed(Duration::from_millis(1), held_input);

    assert_eq!(state.tick, frozen.tick + 1);
    assert_eq!(state.active_piece_col, frozen.active_piece_col + 1);
    assert_eq!(state.ticks_to_repeat, REPEAT_INTERVAL_TICKS);
    assert_eq!(state.active_piece_row, frozen.active_piece_row);
    assert_eq!(
        state.active_piece_orientation,
        frozen.active_piece_orientation
    );
    assert!(state.held_piece.is_none());
    assert_eq!(occupied_cells(&state.grid_locked), 0);
    assert_eq!(state.score, frozen.score);
}

#[test]
fn blocked_gravity_does_not_bank_an_unbounded_fall_after_moving_off_a_surface() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    state.rows_cleared = 190;
    state.grid_locked.set_cell(10, 4, Some(Block::new(WHITE)));
    state.grid_locked.set_cell(10, 5, Some(Block::new(WHITE)));

    state.update_with_elapsed(Duration::from_millis(200), GameInput::default());

    assert_eq!(state.active_piece_row, 8);
    assert!(state.fall_progress <= f64::from(TICKS_PER_SECOND));
    assert_eq!(occupied_cells(&state.grid_locked), 2);

    state.grid_locked.set_cell(10, 4, None);
    state.grid_locked.set_cell(10, 5, None);
    state.update_with_elapsed(duration_for_ticks(1), GameInput::default());

    assert_eq!(state.active_piece_row, 9);
    assert!(state.fall_progress < f64::from(TICKS_PER_SECOND));
    assert_eq!(state.score, 0);
}

#[test]
fn catch_up_stops_at_top_out_before_later_ticks_or_newly_sampled_actions() {
    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    state.active_piece_row = 20;
    state.active_piece_col = 0;
    state.fall_progress = f64::from(TICKS_PER_SECOND);

    let next_piece = block_next_spawn(&mut state);
    state.update_with_elapsed(Duration::ZERO, GameInput::default());
    state.ticks_to_lock = 1;

    assert!(state.has_touched_ground);
    assert_eq!(occupied_cells(&state.grid_active), 4);
    assert_eq!(occupied_cells(&state.grid_ghost), 4);

    state.update_with_elapsed(
        Duration::from_secs(10),
        GameInput {
            soft_drop: true,
            shift_left: true,
            shift_right: true,
            rotate_right: true,
            hard_drop: true,
            hold_piece: true,
            toggle_pause: true,
        },
    );

    assert!(state.is_game_over);
    assert!(!state.is_paused);
    assert_eq!(state.tick, 1);
    assert_eq!(state.active_piece.name, next_piece.name);
    assert_eq!(state.active_piece_row, 1);
    assert_eq!(state.active_piece_col, next_piece.get_initial_col());
    assert_eq!(state.score, 0);
    assert!(state.held_piece.is_none());
    assert_eq!(occupied_cells(&state.grid_locked), 5);
    assert_eq!(occupied_cells(&state.grid_active), 0);
    assert_eq!(occupied_cells(&state.grid_ghost), 0);
}

#[test]
fn scaled_timers_and_presentation_effects_keep_their_wall_time() {
    assert_eq!(REPEAT_DELAY_TICKS, 44);
    assert_eq!(REPEAT_INTERVAL_TICKS, 16);
    assert_eq!(LOCK_DELAY_TICKS, 120);
    assert_eq!(RESET_MOVES, 15);
    assert_eq!(LOCK_IMPACT_TICKS, 40);
    assert_eq!(HARD_DROP_TRAIL_TICKS, 36);
    assert_eq!(LINE_CLEAR_EFFECT_TICKS, 160);
    assert_eq!(LEVEL_FLARE_TICKS, 180);

    let high_scores = HighScoreManager::new();
    let mut state = state_with_piece(&high_scores, pieces::O);
    state.hard_drop();
    state.clear_effect_ticks_remaining = LINE_CLEAR_EFFECT_TICKS;
    state.level_flare_ticks_remaining = LEVEL_FLARE_TICKS;

    assert_eq!(state.impact_ticks_remaining, 88);
    assert_eq!(state.get_impact_effect(), 1.0);

    state.update_with_elapsed(Duration::from_millis(150), GameInput::default());

    assert!(state.get_hard_drop_trail().is_none());
    assert_eq!(state.impact_ticks_remaining, 52);
    assert_eq!(state.clear_effect_ticks_remaining, 124);
    assert_eq!(state.level_flare_ticks_remaining, 144);

    let clear_lifetime = duration_for_ticks(160);
    state.update_with_elapsed(
        clear_lifetime - Duration::from_millis(150),
        GameInput::default(),
    );

    assert_eq!(state.impact_ticks_remaining, 0);
    assert_eq!(state.clear_effect_ticks_remaining, 0);
    assert_eq!(state.level_flare_ticks_remaining, 20);

    state.update_with_elapsed(
        Duration::from_millis(750) - clear_lifetime,
        GameInput::default(),
    );

    assert_eq!(state.level_flare_ticks_remaining, 0);
}

#[test]
fn debris_and_splashes_keep_sixty_hz_physics_independent_of_render_cadence() {
    let high_scores = HighScoreManager::new();
    let mut initial = state_with_piece(&high_scores, pieces::O);
    initial.shrapnel_voxels[0] = ShrapnelVoxel {
        position: Vec3::new(0.0, 5.0, 0.0),
        velocity: Vec3::new(2.0, 3.0, 1.0),
        angular_velocity: Vec3::new(1.0, 2.0, 3.0),
        size: 0.35,
        active: true,
        ..Default::default()
    };
    initial.shrapnel_voxels[1] = ShrapnelVoxel {
        position: Vec3::new(0.0, LAVA_Y + 0.175, 0.0),
        angular_velocity: Vec3::new(1.0, 2.0, 3.0),
        size: 0.35,
        submersion: f32::EPSILON,
        active: true,
        ..Default::default()
    };
    initial.lava_splashes[0] = LavaSplash {
        size: 0.35,
        active: true,
        ..Default::default()
    };
    let mut reference = initial;

    for _ in 0..12 {
        reference.update_shrapnel(1.0 / 60.0);
    }

    for fps in FRAME_RATES.into_iter().chain([1]) {
        let mut state = initial;
        advance_at_fps(
            &mut state,
            Duration::from_millis(200),
            fps,
            GameInput::default(),
        );

        for (actual, expected) in state.shrapnel_voxels[..2]
            .iter()
            .zip(&reference.shrapnel_voxels[..2])
        {
            assert_eq!(actual.position, expected.position);
            assert_eq!(actual.velocity, expected.velocity);
            assert_eq!(actual.rotation, expected.rotation);
            assert_eq!(actual.angular_velocity, expected.angular_velocity);
            assert_eq!(actual.age, expected.age);
            assert_eq!(actual.submersion, expected.submersion);
            assert!(actual.active);
        }

        assert_eq!(state.lava_splashes[0].age, reference.lava_splashes[0].age);
        assert!(state.lava_splashes[0].active);
    }
}
