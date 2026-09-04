use super::{GameInput, GameState, RotationDirection, LOCK_DELAY_TICKS, TICKS_PER_SECOND};
use crate::{
    block::Block,
    grid::{Grid, FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, GRID_COUNT_ROWS},
    high_score_manager::HighScoreManager,
    piece::{pieces, Piece},
    scoring::{score_lock, LockEvent, ScoringState, SpinKind},
};
use macroquad::prelude::WHITE;
use std::time::Duration;

const ROW: isize = FIRST_VISIBLE_ROW_ID as isize + 6;
const COL: isize = 3;
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

fn put(grid: &mut Grid, row: isize, col: isize) {
    assert!((0..GRID_COUNT_ROWS as isize).contains(&row));
    assert!((0..GRID_COUNT_COLS as isize).contains(&col));
    grid.set_cell(row as usize, col as usize, Some(Block::new(WHITE)));
}

fn fill_except(grid: &mut Grid, row: isize, holes: &[isize]) {
    for col in 0..GRID_COUNT_COLS as isize {
        if !holes.contains(&col) {
            put(grid, row, col);
        }
    }
}

fn corner_grid(row: isize, col: isize, corners: &[usize]) -> Grid {
    let mut grid = Grid::new();
    let offsets = [(0, 0), (0, 2), (2, 2), (2, 0)];

    for &corner in corners {
        let (dr, dc) = offsets[corner];
        put(&mut grid, row + dr, col + dc);
    }

    grid
}

fn posed_state(
    high_scores: &HighScoreManager,
    piece: Piece,
    orientation: usize,
    row: isize,
    col: isize,
    grid: Grid,
) -> GameState<'_> {
    let mut state = GameState::new(high_scores);
    state.set_active_piece_and_reset_state(piece);
    state.grid_locked = grid;
    state.active_piece_orientation = orientation;
    state.active_piece_row = row;
    state.active_piece_col = col;
    state.refresh_cached_blocks();
    state.lowest_piece_row = state.lock_reference_row();

    assert!(!state.collide(None, None, None), "invalid scoring fixture");

    state.update_with_elapsed(Duration::ZERO, GameInput::default());

    state
}

/// Reverse the requested SRS candidate to build a legal starting pose, then
/// exercise the real input/rotation path and require that exact candidate.
fn rotate_into(
    high_scores: &HighScoreManager,
    grid: Grid,
    to: usize,
    row: isize,
    col: isize,
    direction: RotationDirection,
    kick_index: usize,
) -> GameState<'_> {
    let from = match direction {
        RotationDirection::Clockwise => (to + 3) % 4,
        RotationDirection::Counterclockwise => (to + 1) % 4,
    };
    let a = pieces::T.orientations[from].offsets[kick_index];
    let b = pieces::T.orientations[to].offsets[kick_index];
    let mut state = posed_state(
        high_scores,
        pieces::T,
        from,
        row + a.1 - b.1,
        col - (a.0 - b.0),
        grid,
    );

    state.update_with_elapsed(Duration::ZERO, rotation_input(direction));

    assert_eq!(state.active_piece_orientation, to);
    assert_eq!(state.active_piece_row, row);
    assert_eq!(state.active_piece_col, col);
    let rotation = state.last_rotation.expect("successful real rotation");
    assert_eq!(rotation.from_orientation, from);
    assert_eq!(rotation.to_orientation, to);
    assert_eq!(rotation.kick_index, kick_index);

    state
}

fn north_slot(spin: SpinKind, lines: usize, row: isize, col: isize) -> Grid {
    let corners: &[usize] = match spin {
        SpinKind::None => &[0, 3],
        SpinKind::Mini => &[0, 2, 3],
        SpinKind::Full => &[0, 1, 3],
    };
    let mut grid = corner_grid(row, col, corners);

    if lines >= 1 {
        fill_except(&mut grid, row + 1, &[col, col + 1, col + 2]);
    }

    if lines == 2 {
        fill_except(&mut grid, row, &[col + 1]);
    }

    grid
}

fn lock_without_drop_distance(state: &mut GameState<'_>) {
    assert!(state.collide(Some(state.active_piece_row + 1), None, None));

    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            hard_drop: true,
            ..Default::default()
        },
    );
}

#[test]
fn both_rotation_directions_classify_every_front_corner_orientation() {
    let high_scores = HighScoreManager::new();

    for orientation in 0..4 {
        let a = orientation;
        let b = (a + 1) % 4;
        let c = (a + 2) % 4;
        let d = (a + 3) % 4;

        for direction in DIRECTIONS {
            for (occupied, expected) in [
                (vec![a, b, c], SpinKind::Full),
                (vec![a, c, d], SpinKind::Mini),
                (vec![a, c], SpinKind::None),
            ] {
                let grid = corner_grid(ROW, COL, &occupied);
                let state = rotate_into(&high_scores, grid, orientation, ROW, COL, direction, 0);

                assert_eq!(
                    state.classify_t_spin(),
                    expected,
                    "{orientation} {direction:?}"
                );
            }
        }
    }
}

#[test]
fn real_spins_score_before_compaction_and_use_the_pre_clear_level() {
    let high_scores = HighScoreManager::new();

    for direction in DIRECTIONS {
        for level in [1, 2] {
            for (spin, lines, base) in [
                (SpinKind::None, 0, 0),
                (SpinKind::None, 1, 100),
                (SpinKind::Mini, 0, 100),
                (SpinKind::Mini, 1, 200),
                (SpinKind::Full, 0, 400),
                (SpinKind::Full, 1, 800),
                (SpinKind::Full, 2, 1200),
            ] {
                let grid = north_slot(spin, lines, ROW, COL);
                let mut state = rotate_into(&high_scores, grid, 0, ROW, COL, direction, 0);
                state.rows_cleared = (level - 1) * 10;
                assert_eq!(state.classify_t_spin(), spin);

                lock_without_drop_distance(&mut state);

                assert_eq!(
                    state.score,
                    base * level,
                    "{spin:?}, {lines}, {direction:?}"
                );
                assert_eq!(state.rows_cleared, (level - 1) * 10 + lines);
                assert!(state.last_rotation.is_none());
                assert!(!state.is_game_over);
                let (expected, _) =
                    score_lock(LockEvent { spin, lines, level }, ScoringState::default());
                assert_eq!(state.scoring_state, expected);
            }
        }
    }
}

#[test]
fn fourth_kick_mini_doubles_remain_mini_in_both_directions() {
    let high_scores = HighScoreManager::new();

    for direction in DIRECTIONS {
        let orientation = match direction {
            RotationDirection::Clockwise => 3,
            RotationDirection::Counterclockwise => 1,
        };
        let mut grid = Grid::new();
        let middle_holes = if orientation == 1 {
            [COL + 1, COL + 2]
        } else {
            [COL, COL + 1]
        };
        fill_except(&mut grid, ROW + 1, &middle_holes);
        fill_except(&mut grid, ROW + 2, &[COL + 1]);
        put(&mut grid, ROW, if orientation == 1 { COL } else { COL + 2 });
        put(&mut grid, ROW - 2, COL + 1);
        put(&mut grid, ROW + 3, COL + 1);
        let mut state = rotate_into(&high_scores, grid, orientation, ROW, COL, direction, 3);

        assert_eq!(state.classify_t_spin(), SpinKind::Mini);

        lock_without_drop_distance(&mut state);

        assert_eq!(state.rows_cleared, 2);
        assert_eq!(state.score, 400);
    }
}

fn fifth_kick_slot(orientation: usize, triple: bool) -> Grid {
    let mut grid = Grid::new();
    let middle_holes = if orientation == 1 {
        [COL + 1, COL + 2]
    } else {
        [COL, COL + 1]
    };
    fill_except(&mut grid, ROW, &[COL + 1]);
    fill_except(&mut grid, ROW + 1, &middle_holes);

    if triple {
        fill_except(&mut grid, ROW + 2, &[COL + 1]);
    } else {
        put(
            &mut grid,
            ROW + 2,
            if orientation == 1 { COL } else { COL + 2 },
        );
    }

    put(&mut grid, ROW - 2, COL + 1);
    put(&mut grid, ROW + 3, COL + 1);

    grid
}

#[test]
fn fifth_kicks_upgrade_three_corner_minis_to_full_doubles() {
    let high_scores = HighScoreManager::new();

    for direction in DIRECTIONS {
        let orientation = match direction {
            RotationDirection::Clockwise => 1,
            RotationDirection::Counterclockwise => 3,
        };
        let grid = fifth_kick_slot(orientation, false);
        let mut state = rotate_into(&high_scores, grid, orientation, ROW, COL, direction, 4);
        let missing_front_col = if orientation == 1 { COL + 2 } else { COL };

        assert!(!state
            .grid_locked
            .occupied_or_outside(ROW + 2, missing_front_col));
        assert_eq!(state.classify_t_spin(), SpinKind::Full);

        lock_without_drop_distance(&mut state);

        assert_eq!(state.rows_cleared, 2);
        assert_eq!(state.score, 1200);
    }
}

#[test]
fn both_directions_can_score_real_t_spin_triples() {
    let high_scores = HighScoreManager::new();

    for direction in DIRECTIONS {
        let orientation = match direction {
            RotationDirection::Clockwise => 1,
            RotationDirection::Counterclockwise => 3,
        };
        let grid = fifth_kick_slot(orientation, true);
        let mut state = rotate_into(&high_scores, grid, orientation, ROW, COL, direction, 4);

        assert_eq!(state.classify_t_spin(), SpinKind::Full);

        lock_without_drop_distance(&mut state);

        assert_eq!(state.rows_cleared, 3);
        assert_eq!(state.score, 1600);
    }
}

fn floor_mini_grid() -> Grid {
    let row = GRID_COUNT_ROWS as isize - 2;
    let mut grid = Grid::new();
    put(&mut grid, row, COL + 2);
    put(&mut grid, row, COL - 1);
    // Blocks a subsequent CCW floor kick without obstructing the incoming R.
    put(&mut grid, row - 1, COL + 2);

    grid
}

#[test]
fn real_wall_and_floor_spins_count_outer_boundaries() {
    let high_scores = HighScoreManager::new();

    for (direction, orientation, col, corner_col) in [
        (RotationDirection::Clockwise, 1, -1, 1),
        (
            RotationDirection::Counterclockwise,
            3,
            GRID_COUNT_COLS as isize - 2,
            GRID_COUNT_COLS as isize - 2,
        ),
    ] {
        let mut grid = Grid::new();
        put(&mut grid, ROW + 2, corner_col);
        let mut state = rotate_into(&high_scores, grid, orientation, ROW, col, direction, 1);

        assert_eq!(state.classify_t_spin(), SpinKind::Mini);

        lock_without_drop_distance(&mut state);

        assert_eq!(state.score, 100);
    }

    let row = GRID_COUNT_ROWS as isize - 2;
    let mut state = rotate_into(
        &high_scores,
        floor_mini_grid(),
        0,
        row,
        COL,
        RotationDirection::Counterclockwise,
        2,
    );

    assert_eq!(state.classify_t_spin(), SpinKind::Mini);

    lock_without_drop_distance(&mut state);

    assert_eq!(state.score, 100);
}

#[test]
fn hidden_rows_are_not_a_scoring_wall_at_the_visible_skyline() {
    let high_scores = HighScoreManager::new();
    let row = FIRST_VISIBLE_ROW_ID as isize - 1;

    for direction in DIRECTIONS {
        let grid = corner_grid(row, 0, &[2, 3]);
        let mut state = rotate_into(&high_scores, grid, 0, row, 0, direction, 0);

        assert_eq!(state.classify_t_spin(), SpinKind::None);

        lock_without_drop_distance(&mut state);

        assert_eq!(state.score, 0);
        assert!(!state.is_game_over);
    }
}

#[test]
fn three_corners_require_a_t_and_a_successful_rotation() {
    let high_scores = HighScoreManager::new();
    let grid = north_slot(SpinKind::Full, 1, ROW, COL);
    let mut unrotated = posed_state(&high_scores, pieces::T, 0, ROW, COL, grid);

    assert_eq!(unrotated.classify_t_spin(), SpinKind::None);

    lock_without_drop_distance(&mut unrotated);

    assert_eq!(unrotated.score, 100);

    let mut other = posed_state(&high_scores, pieces::I, 0, ROW, COL, Grid::new());
    other.update_with_elapsed(Duration::ZERO, rotation_input(RotationDirection::Clockwise));
    let row = other.active_piece_row;
    let col = other.active_piece_col;
    other.grid_locked = corner_grid(row, col, &[0, 1, 3]);

    assert!(!other.collide(None, None, None));
    assert!(other.last_rotation.is_some());
    assert_eq!(other.classify_t_spin(), SpinKind::None);
}

#[test]
fn successful_horizontal_gravity_and_soft_drop_movements_clear_rotation_provenance() {
    let high_scores = HighScoreManager::new();
    let initial = rotate_into(
        &high_scores,
        Grid::new(),
        1,
        ROW,
        COL,
        RotationDirection::Clockwise,
        0,
    );
    let mut horizontal = initial;
    let mut gravity = initial;
    let mut soft = initial;

    horizontal.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            shift_right: true,
            ..Default::default()
        },
    );
    gravity.update_with_elapsed(Duration::from_secs(1), GameInput::default());
    let soft_input = GameInput {
        soft_drop: true,
        ..Default::default()
    };
    soft.update_with_elapsed(Duration::ZERO, soft_input);
    soft.update_with_elapsed(Duration::from_millis(34), soft_input);

    assert_eq!(horizontal.active_piece_col, COL + 1);
    assert_eq!(gravity.active_piece_row, ROW + 1);
    assert_eq!(soft.active_piece_row, ROW + 1);
    assert_eq!(soft.score, 1);

    // Borrow the large states so array iteration stays within the test-thread stack.
    for state in [&horizontal, &gravity, &soft] {
        assert!(state.last_rotation.is_none());
    }
}

#[test]
fn positive_hard_drop_distance_invalidates_a_spin_but_zero_distance_preserves_it() {
    let high_scores = HighScoreManager::new();
    let grid = corner_grid(ROW, COL, &[0, 2, 3]);
    let mut above = rotate_into(
        &high_scores,
        grid,
        1,
        ROW - 2,
        COL,
        RotationDirection::Clockwise,
        0,
    );
    let mut at_slot = rotate_into(
        &high_scores,
        grid,
        1,
        ROW,
        COL,
        RotationDirection::Counterclockwise,
        0,
    );

    assert_eq!(at_slot.classify_t_spin(), SpinKind::Mini);
    assert_eq!(
        above.grid_locked.find_landing_row(
            above.active_piece_row,
            above.active_piece_col,
            &above.cached_blocks,
            above.cached_bounds_height,
            above.cached_bounds_width,
        ),
        ROW
    );

    above.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            hard_drop: true,
            ..Default::default()
        },
    );
    lock_without_drop_distance(&mut at_slot);

    assert_eq!(above.score, 4, "two drop cells, no stale Mini bonus");
    assert_eq!(at_slot.score, 100, "zero-distance drop retains the Mini");
    assert!(above.last_rotation.is_none());
    assert!(at_slot.last_rotation.is_none());
}

#[test]
fn failed_inputs_and_blocked_drops_preserve_the_last_successful_rotation() {
    let high_scores = HighScoreManager::new();
    let row = GRID_COUNT_ROWS as isize - 2;
    let mut state = rotate_into(
        &high_scores,
        floor_mini_grid(),
        0,
        row,
        COL,
        RotationDirection::Counterclockwise,
        2,
    );
    let rotation = state.last_rotation;
    let resets = state.lock_reset_moves_remaining;

    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            shift_right: true,
            ..Default::default()
        },
    );
    state.update_with_elapsed(
        Duration::ZERO,
        rotation_input(RotationDirection::Counterclockwise),
    );

    assert_eq!(state.active_piece_col, COL);
    assert_eq!(state.active_piece_orientation, 0);
    assert_eq!(state.last_rotation, rotation);
    assert_eq!(state.lock_reset_moves_remaining, resets);

    let soft_input = GameInput {
        soft_drop: true,
        ..Default::default()
    };
    state.update_with_elapsed(Duration::ZERO, soft_input);
    state.update_with_elapsed(Duration::from_millis(100), soft_input);
    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            rotate_left: true,
            rotate_right: true,
            ..Default::default()
        },
    );

    assert_eq!(state.active_piece_row, row);
    assert_eq!(state.last_rotation, rotation);
    assert_eq!(state.score, 0);
    assert_eq!(state.classify_t_spin(), SpinKind::Mini);

    lock_without_drop_distance(&mut state);

    assert_eq!(state.score, 100);
}

fn prepare_i_clear(state: &mut GameState<'_>, lines: usize) {
    state.grid_locked.clear();
    state.set_active_piece_and_reset_state(pieces::I);
    state.active_piece_orientation = if lines == 4 { 1 } else { 0 };
    state.active_piece_col = if lines == 4 { 3 } else { 2 };
    state.active_piece_row = GRID_COUNT_ROWS as isize - if lines == 4 { 5 } else { 3 };
    state.refresh_cached_blocks();
    state.lowest_piece_row = state.lock_reference_row();

    for row in GRID_COUNT_ROWS - lines..GRID_COUNT_ROWS {
        let holes: &[isize] = if lines == 4 { &[5] } else { &[3, 4, 5, 6] };
        fill_except(&mut state.grid_locked, row as isize, holes);
    }

    state.update_with_elapsed(Duration::ZERO, GameInput::default());
}

#[test]
fn actual_locks_apply_combo_and_back_to_back_once_across_level_changes() {
    let high_scores = HighScoreManager::new();
    let mut state = GameState::new(&high_scores);

    for (lines, increment, total_lines) in [
        (4, 800, 4),
        (4, 1250, 8),
        (0, 0, 8),
        (4, 1200, 12),
        (1, 300, 13),
        (4, 1800, 17),
    ] {
        if lines == 0 {
            state.grid_locked.clear();
            state.set_active_piece_and_reset_state(pieces::O);
            state.active_piece_row = GRID_COUNT_ROWS as isize - 2;
            state.active_piece_col = 0;
            state.piece_dirty = true;
            state.update_with_elapsed(Duration::ZERO, GameInput::default());
        } else {
            prepare_i_clear(&mut state, lines);
        }

        let before = state.score;
        state.lock_active_piece_and_get_next();

        assert_eq!(state.score - before, increment, "after {total_lines} lines");
        if lines > 0 {
            let award = state
                .get_score_announcement()
                .expect("scoring announcement");
            assert_eq!(award.total(), increment);
            assert_eq!(award.event.lines, lines);
        }

        assert_eq!(state.rows_cleared, total_lines);
        assert!(state.last_rotation.is_none());
        assert!(!state.is_game_over);
    }

    assert_eq!(state.score, 5350);
}

#[test]
fn accepted_hold_and_spawn_clear_provenance_without_resetting_scoring_chains() {
    let high_scores = HighScoreManager::new();

    for populated_hold in [false, true] {
        let mut state = GameState::new(&high_scores);
        prepare_i_clear(&mut state, 4);
        state.lock_active_piece_and_get_next();
        let chains = state.scoring_state;
        state.set_active_piece_and_reset_state(pieces::T);
        state.held_piece = populated_hold.then_some(pieces::I);
        state.update_with_elapsed(Duration::ZERO, rotation_input(RotationDirection::Clockwise));
        assert!(state.last_rotation.is_some());

        state.update_with_elapsed(
            Duration::ZERO,
            GameInput {
                hold_piece: true,
                ..Default::default()
            },
        );

        assert!(state.last_rotation.is_none());
        assert_eq!(state.scoring_state, chains);
        assert_eq!(state.score, 800);
        assert!(state.last_piece_swapped);

        state.update_with_elapsed(
            Duration::ZERO,
            rotation_input(RotationDirection::Counterclockwise),
        );
        let rotation = state.last_rotation;
        assert!(rotation.is_some());

        state.update_with_elapsed(
            Duration::ZERO,
            GameInput {
                hold_piece: true,
                ..Default::default()
            },
        );

        assert_eq!(
            state.last_rotation, rotation,
            "rejected hold is not movement"
        );
        assert_eq!(state.scoring_state, chains);
        assert_eq!(state.score, 800);
    }
}

#[test]
fn lock_out_does_not_award_an_otherwise_qualifying_hidden_spin() {
    let high_scores = HighScoreManager::new();
    let row = FIRST_VISIBLE_ROW_ID as isize - 3;
    let grid = north_slot(SpinKind::Full, 0, row, COL);
    let mut state = rotate_into(
        &high_scores,
        grid,
        0,
        row,
        COL,
        RotationDirection::Clockwise,
        0,
    );

    assert!(state.check_for_lock_out());
    assert_eq!(state.classify_t_spin(), SpinKind::Full);

    lock_without_drop_distance(&mut state);

    assert!(state.is_game_over);
    assert_eq!(state.score, 0);
    assert_eq!(state.scoring_state, ScoringState::default());
    assert_eq!(state.rows_cleared, 0);
}

#[test]
fn synthetic_harness_clears_do_not_use_the_active_pieces_spin_record() {
    let high_scores = HighScoreManager::new();
    let grid = north_slot(SpinKind::Full, 0, ROW, COL);
    let mut state = rotate_into(
        &high_scores,
        grid,
        0,
        ROW,
        COL,
        RotationDirection::Clockwise,
        0,
    );
    fill_except(&mut state.grid_locked, GRID_COUNT_ROWS as isize - 1, &[]);

    assert_eq!(state.classify_t_spin(), SpinKind::Full);

    state.trigger_line_clear();

    assert_eq!(state.score, 100);
    assert_eq!(state.rows_cleared, 1);
}

#[test]
fn pause_preserves_spin_provenance_and_the_lock_scores_once_after_resume() {
    let high_scores = HighScoreManager::new();
    let grid = north_slot(SpinKind::Full, 1, ROW, COL);
    let mut state = rotate_into(
        &high_scores,
        grid,
        0,
        ROW,
        COL,
        RotationDirection::Counterclockwise,
        0,
    );
    let rotation = state.last_rotation;
    state.toggle_pause();
    state.update_with_elapsed(
        Duration::from_secs(30),
        GameInput {
            shift_left: true,
            rotate_right: true,
            hard_drop: true,
            ..Default::default()
        },
    );

    assert_eq!(state.last_rotation, rotation);
    assert_eq!(state.score, 0);
    assert_eq!(state.ticks_to_lock, LOCK_DELAY_TICKS);

    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            toggle_pause: true,
            ..Default::default()
        },
    );
    state.update_with_elapsed(Duration::from_millis(500), GameInput::default());

    assert_eq!(state.score, 800);
    assert_eq!(state.rows_cleared, 1);
    assert!(state.last_rotation.is_none());

    state.update_with_elapsed(Duration::ZERO, GameInput::default());

    assert_eq!(state.score, 800);
}

#[test]
fn spin_locks_score_identically_across_catch_up_partitions() {
    let high_scores = HighScoreManager::new();
    let grid = north_slot(SpinKind::Full, 1, ROW, COL);
    let initial = rotate_into(
        &high_scores,
        grid,
        0,
        ROW,
        COL,
        RotationDirection::Clockwise,
        0,
    );
    let mut single = initial;
    let mut irregular = initial;
    single.update_with_elapsed(Duration::from_millis(500), GameInput::default());

    for millis in [17, 49, 133, 301] {
        irregular.update_with_elapsed(Duration::from_millis(millis), GameInput::default());
    }

    assert_eq!(single.tick, TICKS_PER_SECOND as usize / 2);
    assert_eq!(single.tick, irregular.tick);
    assert_eq!(single.score, 800);
    assert_eq!(irregular.score, single.score);
    assert_eq!(irregular.scoring_state, single.scoring_state);
    assert_eq!(irregular.rows_cleared, single.rows_cleared);
    assert_eq!(irregular.active_piece.name, single.active_piece.name);
    assert_eq!(
        irregular.get_score_announcement(),
        single.get_score_announcement()
    );
    assert_eq!(
        irregular.score_announcement_ticks_remaining,
        single.score_announcement_ticks_remaining
    );
}

#[test]
fn score_announcements_expire_after_two_playing_seconds_and_freeze_on_pause() {
    let high_scores = HighScoreManager::new();
    let mut state = GameState::new(&high_scores);
    prepare_i_clear(&mut state, 4);
    state.lock_active_piece_and_get_next();
    let shown = state.get_score_announcement().expect("Tetris award");
    assert_eq!(shown.total(), 800);
    assert_eq!(state.get_score_announcement_remaining(), 1.0);

    state.update_with_elapsed(Duration::from_millis(500), GameInput::default());
    assert_eq!(state.get_score_announcement_remaining(), 0.75);
    state.toggle_pause();
    let remaining = state.score_announcement_ticks_remaining;
    state.update_with_elapsed(
        Duration::from_secs(30),
        GameInput {
            hold_piece: true,
            hard_drop: true,
            rotate_left: true,
            ..Default::default()
        },
    );

    assert_eq!(state.get_score_announcement(), Some(shown));
    assert_eq!(state.score_announcement_ticks_remaining, remaining);
    assert_eq!(state.get_score_announcement_remaining(), 0.75);
    assert_eq!(state.score, 800);

    state.update_with_elapsed(
        Duration::from_secs(30),
        GameInput {
            toggle_pause: true,
            ..Default::default()
        },
    );
    state.update_with_elapsed(
        Duration::from_millis(1500) - Duration::from_nanos(1),
        GameInput::default(),
    );

    assert_eq!(state.get_score_announcement(), Some(shown));
    assert_eq!(state.score_announcement_ticks_remaining, 1);
    assert!(state.get_score_announcement_remaining() > 0.0);

    state.update_with_elapsed(Duration::from_nanos(1), GameInput::default());

    assert!(state.get_score_announcement().is_none());
    assert_eq!(state.get_score_announcement_remaining(), 0.0);
    assert_eq!(state.tick, 2 * TICKS_PER_SECOND as usize);
}

#[test]
fn zero_point_locks_do_not_erase_or_extend_a_previous_announcement() {
    let high_scores = HighScoreManager::new();
    let mut state = GameState::new(&high_scores);
    prepare_i_clear(&mut state, 4);
    state.lock_active_piece_and_get_next();
    state.update_with_elapsed(Duration::from_millis(500), GameInput::default());
    let shown = state.get_score_announcement();
    let remaining = state.score_announcement_ticks_remaining;

    state.set_active_piece_and_reset_state(pieces::O);
    state.active_piece_row = GRID_COUNT_ROWS as isize - 2;
    state.active_piece_col = 0;
    state.piece_dirty = true;
    state.update_with_elapsed(Duration::ZERO, GameInput::default());
    state.lock_active_piece_and_get_next();

    assert_eq!(state.score, 800);
    assert_eq!(state.get_score_announcement(), shown);
    assert_eq!(state.score_announcement_ticks_remaining, remaining);

    fill_except(&mut state.grid_locked, GRID_COUNT_ROWS as isize - 1, &[]);
    state.trigger_line_clear();
    let replacement = state.get_score_announcement().expect("new Single award");

    assert_eq!(replacement.event.lines, 1);
    assert_eq!(replacement.total(), 100);
    assert_eq!(state.score, 900);
    assert_eq!(
        state.score_announcement_ticks_remaining,
        2 * TICKS_PER_SECOND as usize
    );
}

#[test]
fn announcements_keep_the_pre_clear_level_and_full_bonus_breakdown() {
    let high_scores = HighScoreManager::new();
    let grid = north_slot(SpinKind::Full, 1, ROW, COL);
    let mut state = rotate_into(
        &high_scores,
        grid,
        0,
        ROW,
        COL,
        RotationDirection::Clockwise,
        0,
    );
    state.rows_cleared = 9;

    lock_without_drop_distance(&mut state);
    let spin = state.get_score_announcement().expect("T-spin Single");

    assert_eq!(state.get_level(), 2);
    assert_eq!(
        spin.event,
        LockEvent {
            spin: SpinKind::Full,
            lines: 1,
            level: 1
        }
    );
    assert_eq!(spin.base, 800);
    assert_eq!(spin.total(), 800);

    prepare_i_clear(&mut state, 4);
    state.lock_active_piece_and_get_next();
    let tetris = state.get_score_announcement().expect("B2B Tetris");

    assert_eq!(tetris.event.level, 2);
    assert_eq!(tetris.base, 1600);
    assert_eq!(tetris.back_to_back_bonus, 800);
    assert_eq!(tetris.combo_bonus, 100);
    assert_eq!(tetris.combo, Some(1));
    assert_eq!(tetris.total(), 2500);
    assert_eq!(state.score, 3300);
}

#[test]
fn drop_points_alone_do_not_create_a_clear_announcement() {
    let high_scores = HighScoreManager::new();
    let mut state = GameState::new(&high_scores);

    state.update_with_elapsed(
        Duration::ZERO,
        GameInput {
            hard_drop: true,
            ..Default::default()
        },
    );

    assert!(state.score > 0);
    assert_eq!(state.rows_cleared, 0);
    assert!(state.get_score_announcement().is_none());
}
