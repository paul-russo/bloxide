use crate::{
    bag_manager::BagManager,
    grid::{
        ClearedBlock, Grid, FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, GRID_COUNT_ROWS,
        MAX_CLEARED_CELLS,
    },
    high_score_manager::HighScoreManager,
    piece::{BlockCanvas, Piece},
    render3d::{
        cell_center, floor_gap_contains, LavaSplash, ShrapnelVoxel, FLOOR_Y, LAVA_Y,
        MAX_LAVA_SPLASHES, MAX_SHRAPNEL_VOXELS, SPLASH_SECONDS,
    },
    scoring::{score_lock, LockEvent, ScoringState, SpinKind},
};
use macroquad::prelude::{Color, Vec3};
use rand::Rng;
use std::time::{Duration, Instant};

const TICKS_PER_SECOND: u32 = 240;
const NANOS_PER_SECOND: u128 = 1_000_000_000;
const ROWS_PER_SECOND_PER_G: f64 = 60.0;
const SOFT_DROP_ROWS_PER_SECOND: f64 = 30.0;
const REPEAT_DELAY_TICKS: isize = 44; // 183.333ms before repeating horizontal movement.
const REPEAT_INTERVAL_TICKS: isize = 16; // 66.667ms, or 15 repeats per second.
const LOCK_DELAY_TICKS: isize = 120; // 500ms before the active piece locks.
const RESET_MOVES: isize = 15; // Successful post-contact moves per lowest row before grounded lock.
const EFFECT_TICK_INTERVAL: usize = TICKS_PER_SECOND as usize / 60;
const LOCK_IMPACT_TICKS: usize = 10 * EFFECT_TICK_INTERVAL;
const LINE_CLEAR_EFFECT_TICKS: usize = 40 * EFFECT_TICK_INTERVAL;
const MAX_IMPACT_CONTACTS: usize = 4;
const HARD_DROP_TRAIL_TICKS: usize = 9 * EFFECT_TICK_INTERVAL;
const LEVEL_FLARE_TICKS: usize = 45 * EFFECT_TICK_INTERVAL;

/// The path a hard-dropped piece just travelled, so the renderer can streak
/// it. Rows are grid rows of the piece's canvas origin, before and after the
/// drop.
#[derive(Copy, Clone, Debug)]
pub struct HardDropTrail {
    pub start_row: isize,
    pub landing_row: isize,
    pub col: isize,
    pub canvas: BlockCanvas,
    pub bounds_height: usize,
    pub bounds_width: usize,
    pub color: Color,
}

/// Find every downward contact made by the active piece. Coordinates are in
/// grid space: columns use cell centres and rows identify the supporting block
/// (or the floor boundary at [`GRID_COUNT_ROWS`]). A tetromino can contribute at
/// most four independent contacts.
fn impact_contact_origins(
    grid_locked: &Grid,
    active_piece_row: isize,
    active_piece_col: isize,
    canvas: &BlockCanvas,
    bounds_height: usize,
    bounds_width: usize,
) -> ([(f32, f32); MAX_IMPACT_CONTACTS], usize) {
    let mut contacts = [(0.0, 0.0); MAX_IMPACT_CONTACTS];
    let mut contact_count = 0;
    let mut occupied_col_sum = 0.0;
    let mut occupied_count = 0.0;
    let mut lowest_occupied_row = active_piece_row;

    for canvas_row in 0..bounds_height {
        for canvas_col in 0..bounds_width {
            if canvas[canvas_row][canvas_col].is_none() {
                continue;
            }

            let grid_row = active_piece_row + canvas_row as isize;
            let grid_col = active_piece_col + canvas_col as isize;
            let below_row = grid_row + 1;
            let column_center = grid_col as f32 + 0.5;

            occupied_col_sum += column_center;
            occupied_count += 1.0;
            lowest_occupied_row = lowest_occupied_row.max(grid_row);

            let touches_floor = below_row >= GRID_COUNT_ROWS as isize;
            let touches_stack = below_row >= 0
                && grid_col >= 0
                && grid_col < GRID_COUNT_COLS as isize
                && grid_locked.has_block_at_cell(below_row as usize, grid_col as usize);

            if touches_floor || touches_stack {
                contacts[contact_count] = (column_center, below_row as f32);
                contact_count += 1;
            }
        }
    }

    if contact_count == 0 && occupied_count > 0.0 {
        // Locking should always have at least one downward contact, but retain a
        // geometrically sensible fallback for malformed or future piece rules.
        contacts[0] = (
            occupied_col_sum / occupied_count,
            lowest_occupied_row as f32 + 1.0,
        );
        contact_count = 1;
    }

    (contacts, contact_count)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GameInput {
    pub soft_drop: bool,
    pub shift_left: bool,
    pub shift_right: bool,
    pub rotate_left: bool,
    pub rotate_right: bool,
    pub hard_drop: bool,
    pub hold_piece: bool,
    pub toggle_pause: bool,
}

#[derive(Clone, Copy, Debug)]
enum RotationDirection {
    Clockwise,
    Counterclockwise,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RotationRecord {
    from_orientation: usize,
    to_orientation: usize,
    kick_index: usize,
}

#[derive(Clone, Copy, Debug)]
enum ShiftDirection {
    Left,
    Right,
    Neither,
}

#[derive(Copy, Clone)]
pub struct GameState<'a> {
    grid_locked: Grid,
    grid_active: Grid,
    grid_ghost: Grid,
    bag_manager: BagManager,
    active_piece: Piece,
    score: usize,
    scoring_state: ScoringState,
    last_rotation: Option<RotationRecord>,
    tick: usize,
    last_update: Instant,
    // Nanoseconds scaled by the tick rate; one tick costs NANOS_PER_SECOND.
    // Keeping the remainder avoids rounding 1/240 second to whole nanoseconds.
    tick_accumulator: u128,
    // Only continuous keys are stored; sampled presses execute once immediately.
    held_input: GameInput,
    active_piece_col: isize,
    active_piece_row: isize,
    active_piece_orientation: usize,
    // Fall distance in 1/TICKS_PER_SECOND cells. At level 1, adding exactly
    // one unit per tick avoids floating-point drift at the one-second boundary.
    fall_progress: f64,
    ticks_to_repeat: isize,
    ticks_to_lock: isize,
    lock_reset_moves_remaining: isize,
    has_touched_ground: bool,
    // Lowest rotation-stable reference row reached by this piece, not its canvas origin.
    lowest_piece_row: isize,
    shift_direction: ShiftDirection,
    held_piece: Option<Piece>,
    last_piece_swapped: bool,
    rows_cleared: usize,
    is_game_over: bool,
    is_paused: bool,
    high_score_manager: &'a HighScoreManager,
    // Cached block canvas to avoid repeated allocations
    cached_blocks: BlockCanvas,
    cached_bounds_height: usize,
    cached_bounds_width: usize,
    // Dirty flag to track if piece state changed
    piece_dirty: bool,
    // Cached ghost row
    cached_ghost_row: isize,
    // Short-lived presentation cues consumed by the renderer. Keeping these in
    // the game state makes effects deterministic and ensures pause freezes them.
    impact_ticks_remaining: usize,
    clear_effect_ticks_remaining: usize,
    last_clear_count: usize,
    /// Bit `n` is set when visible row `n` was part of the last line clear.
    last_clear_row_mask: u32,
    impact_origins: [(f32, f32); MAX_IMPACT_CONTACTS],
    impact_origin_count: usize,
    impact_color: Color,
    shrapnel_voxels: [ShrapnelVoxel; MAX_SHRAPNEL_VOXELS],
    lava_splashes: [LavaSplash; MAX_LAVA_SPLASHES],
    next_splash_index: usize,
    hard_drop_trail: Option<HardDropTrail>,
    hard_drop_trail_ticks_remaining: usize,
    level_flare_ticks_remaining: usize,
}

impl<'a> GameState<'a> {
    pub fn new(high_score_manager: &'a HighScoreManager) -> Self {
        let grid_locked = Grid::new();
        let grid_active = Grid::new();
        let grid_ghost = Grid::new();
        let mut bag_manager = BagManager::new();
        let active_piece = bag_manager.next();
        let score: usize = 0;
        let tick: usize = 0;
        let active_piece_col = active_piece.get_initial_col();
        let active_piece_row = active_piece.get_initial_row();
        let active_piece_orientation: usize = 0;

        // Initialize cached blocks
        let (cached_blocks, cached_bounds_height, cached_bounds_width) =
            active_piece.get_blocks(active_piece_orientation);

        Self {
            grid_locked,
            grid_active,
            grid_ghost,
            bag_manager,
            active_piece,
            score,
            scoring_state: ScoringState::default(),
            last_rotation: None,
            tick,
            last_update: Instant::now(),
            tick_accumulator: 0,
            held_input: GameInput::default(),
            active_piece_col,
            active_piece_row,
            active_piece_orientation,
            fall_progress: 0.0,
            ticks_to_repeat: REPEAT_DELAY_TICKS,
            ticks_to_lock: LOCK_DELAY_TICKS,
            lock_reset_moves_remaining: RESET_MOVES,
            has_touched_ground: false,
            lowest_piece_row: active_piece_row
                - active_piece.orientations[active_piece_orientation].offsets[0].1,
            shift_direction: ShiftDirection::Neither,
            held_piece: None,
            last_piece_swapped: false,
            rows_cleared: 0,
            is_game_over: false,
            is_paused: false,
            high_score_manager,
            cached_blocks,
            cached_bounds_height,
            cached_bounds_width,
            piece_dirty: true,
            cached_ghost_row: 0,
            impact_ticks_remaining: 0,
            clear_effect_ticks_remaining: 0,
            last_clear_count: 0,
            last_clear_row_mask: 0,
            impact_origins: [(0.0, 0.0); MAX_IMPACT_CONTACTS],
            impact_origin_count: 0,
            impact_color: active_piece.color,
            shrapnel_voxels: [ShrapnelVoxel::default(); MAX_SHRAPNEL_VOXELS],
            lava_splashes: [LavaSplash::default(); MAX_LAVA_SPLASHES],
            next_splash_index: 0,
            hard_drop_trail: None,
            hard_drop_trail_ticks_remaining: 0,
            level_flare_ticks_remaining: 0,
        }
    }

    fn reset_piece_state(&mut self) {
        self.active_piece_orientation = 0;
        self.active_piece_col = self.active_piece.get_initial_col();
        self.active_piece_row = self.active_piece.get_initial_row();
        self.fall_progress = 0.0;
        self.last_rotation = None;
        self.last_piece_swapped = false;
        self.ticks_to_lock = LOCK_DELAY_TICKS;
        self.lock_reset_moves_remaining = RESET_MOVES;
        self.has_touched_ground = false;
        self.lowest_piece_row = self.lock_reference_row();
        self.refresh_cached_blocks();
    }

    fn refresh_cached_blocks(&mut self) {
        let (blocks, height, width) = self.active_piece.get_blocks(self.active_piece_orientation);
        self.cached_blocks = blocks;
        self.cached_bounds_height = height;
        self.cached_bounds_width = width;
        self.piece_dirty = true;
    }

    fn lock_reference_row(&self) -> isize {
        // SRS test zero includes the I/O canvas-origin correction. Removing it
        // keeps pure rotations at the same height; real kicks still translate it.
        self.active_piece_row
            - self.active_piece.orientations[self.active_piece_orientation].offsets[0].1
    }

    fn update_lock_contact(&mut self) -> bool {
        let row = self.lock_reference_row();
        if row > self.lowest_piece_row {
            self.lowest_piece_row = row;
            self.lock_reset_moves_remaining = RESET_MOVES;
            self.ticks_to_lock = LOCK_DELAY_TICKS;
        }

        let grounded = self.collide(Some(self.active_piece_row + 1), None, None);
        if grounded && !self.has_touched_ground {
            self.has_touched_ground = true;
            self.ticks_to_lock = LOCK_DELAY_TICKS;
        }

        grounded
    }

    fn try_reset_lock_delay_for_move(&mut self) {
        if self.has_touched_ground && self.lock_reset_moves_remaining > 0 {
            self.lock_reset_moves_remaining -= 1;
            self.ticks_to_lock = LOCK_DELAY_TICKS;
        }
    }

    fn lock_if_due(&mut self) {
        if self.update_lock_contact()
            && (self.ticks_to_lock <= 0 || self.lock_reset_moves_remaining <= 0)
        {
            self.lock_active_piece_and_get_next();
        }
    }

    fn end_game(&mut self) {
        self.is_game_over = true;
        self.grid_active.clear();
        self.grid_ghost.clear();

        self.high_score_manager.add_score(self.score);
    }

    pub fn toggle_pause(&mut self) {
        if self.is_game_over {
            return;
        }

        self.is_paused = !self.is_paused;

        // The pause menu can resume outside update(). Exclude paused wall time
        // while preserving the simulation's tick phase and partial tick.
        self.last_update = Instant::now();
    }

    /// Check if the active piece, if it were locked, would be entirely outside the visible bounds of the playfield.
    fn check_for_lock_out(&self) -> bool {
        self.grid_locked.invisible_check(
            self.active_piece_row,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        )
    }

    fn set_active_piece_and_reset_state(&mut self, next_active_piece: Piece) {
        self.active_piece = next_active_piece;
        self.reset_piece_state();

        // Check for a piece spawned overlapping at least one block in the playfield (Block Out)
        let is_block_out = self.collide(None, None, None);

        if is_block_out {
            self.end_game();
            return;
        }

        self.update_lock_contact();
    }

    fn next_piece(&mut self) {
        let next_active_piece = self.bag_manager.next();
        self.set_active_piece_and_reset_state(next_active_piece);
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

    fn lock_active_piece_and_get_next(&mut self) {
        if self.check_for_lock_out() {
            self.end_game();
            return;
        }

        let (contact_origins, contact_count) = impact_contact_origins(
            &self.grid_locked,
            self.active_piece_row,
            self.active_piece_col,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        );
        self.impact_origins = contact_origins;
        self.impact_origin_count = contact_count;
        self.impact_color = self.active_piece.color;

        self.grid_locked.set_cells(
            self.active_piece_row,
            self.active_piece_col,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        );

        self.impact_ticks_remaining = self.impact_ticks_remaining.max(LOCK_IMPACT_TICKS);

        // Corners belong to the locked position, before any rows collapse.
        let spin = self.classify_t_spin();
        self.resolve_lock_score(spin);

        self.next_piece();
    }

    fn classify_t_spin(&self) -> SpinKind {
        if self.active_piece.name != "T" {
            return SpinKind::None;
        }

        let Some(rotation) = self.last_rotation else {
            return SpinKind::None;
        };
        let turn = (rotation.to_orientation + 4 - rotation.from_orientation) % 4;
        if rotation.to_orientation != self.active_piece_orientation || !matches!(turn, 1 | 3) {
            return SpinKind::None;
        }

        // NW, NE, SE, SW around the T's untrimmed (1, 1) rotation centre.
        let row = self.active_piece_row;
        let col = self.active_piece_col;
        let corners = [
            self.grid_locked.occupied_or_outside(row, col),
            self.grid_locked.occupied_or_outside(row, col + 2),
            self.grid_locked.occupied_or_outside(row + 2, col + 2),
            self.grid_locked.occupied_or_outside(row + 2, col),
        ];
        if corners.iter().filter(|&&occupied| occupied).count() < 3 {
            return SpinKind::None;
        }

        let front = self.active_piece_orientation;
        if (corners[front] && corners[(front + 1) % 4]) || rotation.kick_index == 4 {
            SpinKind::Full
        } else {
            SpinKind::Mini
        }
    }

    fn hard_drop(&mut self) {
        let landing_row = self.grid_locked.find_landing_row(
            self.active_piece_row,
            self.active_piece_col,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        );

        let lines_dropped = (landing_row - self.active_piece_row).max(0);

        if lines_dropped > 0 {
            self.last_rotation = None;
            self.hard_drop_trail = Some(HardDropTrail {
                start_row: self.active_piece_row,
                landing_row,
                col: self.active_piece_col,
                canvas: self.cached_blocks,
                bounds_height: self.cached_bounds_height,
                bounds_width: self.cached_bounds_width,
                color: self.active_piece.color,
            });
            self.hard_drop_trail_ticks_remaining = HARD_DROP_TRAIL_TICKS;
        }

        self.active_piece_row = landing_row;

        // Longer drops land with a little more visual weight, while the cap
        // keeps the pulse brief enough not to distract from the next piece.
        let drop_impact_ticks = (lines_dropped as usize).min(12) * EFFECT_TICK_INTERVAL;
        self.impact_ticks_remaining = LOCK_IMPACT_TICKS + drop_impact_ticks;

        self.score += 2 * lines_dropped as usize;

        self.lock_active_piece_and_get_next();
    }

    fn collide(
        &self,
        row_id: Option<isize>,
        col_id: Option<isize>,
        orientation: Option<usize>,
    ) -> bool {
        let orientation = orientation.unwrap_or(self.active_piece_orientation);

        // Use cached blocks if orientation matches, otherwise compute fresh
        if orientation == self.active_piece_orientation {
            self.grid_locked.collision_check(
                row_id.unwrap_or(self.active_piece_row),
                col_id.unwrap_or(self.active_piece_col),
                &self.cached_blocks,
                self.cached_bounds_height,
                self.cached_bounds_width,
            )
        } else {
            let (blocks, height, width) = self.active_piece.get_blocks(orientation);
            self.grid_locked.collision_check(
                row_id.unwrap_or(self.active_piece_row),
                col_id.unwrap_or(self.active_piece_col),
                &blocks,
                height,
                width,
            )
        }
    }

    fn try_rotate(&mut self, direction: RotationDirection) {
        let next_orientation = match direction {
            RotationDirection::Clockwise => (self.active_piece_orientation + 1) % 4,
            RotationDirection::Counterclockwise => (self.active_piece_orientation + 3) % 4,
        };

        let offsets_a = self.active_piece.orientations[self.active_piece_orientation].offsets;
        let offsets_b = self.active_piece.orientations[next_orientation].offsets;

        for index in 0..offsets_a.len() {
            let offset_col = offsets_a[index].0 - offsets_b[index].0;
            let offset_row = offsets_a[index].1 - offsets_b[index].1;

            // I'm subtracting the offset from active_piece_row instead of adding it here,
            // because rows are counted from the bottom in the Guideline. They're counted
            // in the other direction in my Grid implementation, so the offsets I copied
            // from the Guideline have to get applied backwards here.
            let next_active_piece_row = self.active_piece_row - offset_row;
            let next_active_piece_col = self.active_piece_col + offset_col;

            let has_collision = self.collide(
                Some(next_active_piece_row),
                Some(next_active_piece_col),
                Some(next_orientation),
            );

            if !has_collision {
                self.last_rotation = Some(RotationRecord {
                    from_orientation: self.active_piece_orientation,
                    to_orientation: next_orientation,
                    kick_index: index,
                });
                self.active_piece_orientation = next_orientation;
                self.active_piece_row = next_active_piece_row;
                self.active_piece_col = next_active_piece_col;
                self.refresh_cached_blocks();
                self.try_reset_lock_delay_for_move();

                return;
            }
        }
    }

    fn set_shift_direction_and_reset_ticks(&mut self, new_shift_direction: ShiftDirection) {
        self.ticks_to_repeat = REPEAT_DELAY_TICKS;
        self.shift_direction = new_shift_direction;
    }

    fn try_move_horizontal(
        &mut self,
        is_shift_left: bool,
        is_shift_right: bool,
        elapsed_ticks: isize,
    ) {
        let mut col_offset = 0;

        if !is_shift_left && !is_shift_right {
            return self.set_shift_direction_and_reset_ticks(ShiftDirection::Neither);
        }

        match self.shift_direction {
            ShiftDirection::Left => {
                if !is_shift_left {
                    if is_shift_right {
                        self.set_shift_direction_and_reset_ticks(ShiftDirection::Right);
                        col_offset = 1;
                    }
                } else {
                    self.ticks_to_repeat -= elapsed_ticks;
                }
            }

            ShiftDirection::Right => {
                if !is_shift_right {
                    if is_shift_left {
                        self.set_shift_direction_and_reset_ticks(ShiftDirection::Left);
                        col_offset = -1;
                    }
                } else {
                    self.ticks_to_repeat -= elapsed_ticks;
                }
            }

            ShiftDirection::Neither => {
                if is_shift_left {
                    self.set_shift_direction_and_reset_ticks(ShiftDirection::Left);
                    col_offset = -1;
                } else if is_shift_right {
                    self.set_shift_direction_and_reset_ticks(ShiftDirection::Right);
                    col_offset = 1;
                }
            }
        }

        if self.ticks_to_repeat <= 0 {
            col_offset = match self.shift_direction {
                ShiftDirection::Left => -1,
                ShiftDirection::Right => 1,
                ShiftDirection::Neither => unreachable!(),
            };

            self.ticks_to_repeat = REPEAT_INTERVAL_TICKS;
        }

        let next_active_piece_col = self.active_piece_col + col_offset;

        // Horizontal collision check
        if next_active_piece_col != self.active_piece_col {
            let has_collision = self.collide(None, Some(next_active_piece_col), None);

            if !has_collision {
                self.active_piece_col = next_active_piece_col;
                self.last_rotation = None;
                self.piece_dirty = true;
                self.try_reset_lock_delay_for_move();
            }
        }
    }

    fn try_gravity_drop(&mut self, is_soft_drop: bool) {
        let natural_rows_per_second = self.get_gravity() * ROWS_PER_SECOND_PER_G;
        let rows_per_second = if is_soft_drop {
            natural_rows_per_second.max(SOFT_DROP_ROWS_PER_SECOND)
        } else {
            natural_rows_per_second
        };
        self.fall_progress += rows_per_second;

        while self.fall_progress >= TICKS_PER_SECOND as f64 {
            let next_active_piece_row = self.active_piece_row + 1;
            let has_collision = self.collide(Some(next_active_piece_row), None, None);
            if has_collision {
                // Keep one pending row so a blocked fall is retried each tick,
                // but never bank a burst of fall distance against a surface.
                self.fall_progress = TICKS_PER_SECOND as f64;
                return;
            }

            self.fall_progress -= TICKS_PER_SECOND as f64;

            if is_soft_drop {
                self.score += 1;
            }

            self.active_piece_row = next_active_piece_row;
            // This profile requires rotation to remain the last successful
            // movement. Blocked falls and zero-distance hard drops preserve it.
            self.last_rotation = None;
            self.piece_dirty = true;
        }
    }

    pub fn update(&mut self, input: GameInput) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);
        self.last_update = now;

        self.update_with_elapsed(elapsed, input);
    }

    /// Elapsed time precedes this input sample. Catch up using the previous
    /// held keys, then apply new presses once, even when no tick is due.
    /// This deterministic entry point never reads the wall clock.
    fn update_with_elapsed(&mut self, elapsed: Duration, input: GameInput) {
        if self.is_game_over {
            return;
        }

        if !self.is_paused {
            self.tick_accumulator += elapsed.as_nanos() * u128::from(TICKS_PER_SECOND);

            while self.tick_accumulator >= NANOS_PER_SECOND {
                self.tick_accumulator -= NANOS_PER_SECOND;
                self.step_tick();
                if self.is_game_over {
                    return;
                }
            }
        }

        if input.toggle_pause {
            self.is_paused = !self.is_paused;
        }

        // Menu resume happens outside update(), so keep continuous keys current
        // even while paused without advancing DAS or retaining one-shot presses.
        self.held_input = GameInput {
            soft_drop: input.soft_drop,
            shift_left: input.shift_left,
            shift_right: input.shift_right,
            ..Default::default()
        };

        if self.is_paused {
            self.refresh_piece_grids();
            return;
        }

        if input.hold_piece {
            self.swap_active_piece();
            if self.is_game_over {
                return;
            }
        }

        self.update_lock_contact();

        // Opposite presses in the same sample cancel, rather than rotating twice.
        let rotation = match (input.rotate_left, input.rotate_right) {
            (true, false) => Some(RotationDirection::Counterclockwise),
            (false, true) => Some(RotationDirection::Clockwise),
            _ => None,
        };
        if let Some(direction) = rotation {
            self.try_rotate(direction);
            self.lock_if_due();
            if self.is_game_over {
                return;
            }
        }

        if input.hard_drop {
            self.hard_drop();
            if self.is_game_over {
                return;
            }
        }

        self.try_move_horizontal(input.shift_left, input.shift_right, 0);
        self.lock_if_due();
        if self.is_game_over {
            return;
        }

        self.refresh_piece_grids();
    }

    fn step_tick(&mut self) {
        // Charge the interval that began in contact, before movement can reset
        // it. A piece landing later in this tick receives the full 120 ticks.
        if self.update_lock_contact() {
            self.ticks_to_lock -= 1;
        }

        self.tick += 1;
        self.impact_ticks_remaining = self.impact_ticks_remaining.saturating_sub(1);
        self.clear_effect_ticks_remaining = self.clear_effect_ticks_remaining.saturating_sub(1);
        self.hard_drop_trail_ticks_remaining =
            self.hard_drop_trail_ticks_remaining.saturating_sub(1);
        self.level_flare_ticks_remaining = self.level_flare_ticks_remaining.saturating_sub(1);

        // Keep the debris integrator and its per-step drag at the original
        // 60 Hz. A slow render frame still runs every owed physics step.
        if self.tick.is_multiple_of(EFFECT_TICK_INTERVAL) {
            self.update_shrapnel(1.0 / 60.0);
        }

        let input = self.held_input;
        self.try_move_horizontal(input.shift_left, input.shift_right, 1);
        self.try_gravity_drop(input.soft_drop);
        self.lock_if_due();
    }

    fn refresh_piece_grids(&mut self) {
        if !self.piece_dirty {
            return;
        }

        self.grid_active.clear().set_cells(
            self.active_piece_row,
            self.active_piece_col,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        );

        self.cached_ghost_row = self.grid_locked.find_landing_row(
            self.active_piece_row,
            self.active_piece_col,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        );

        self.grid_ghost.clear().set_cells(
            self.cached_ghost_row,
            self.active_piece_col,
            &self.cached_blocks,
            self.cached_bounds_height,
            self.cached_bounds_width,
        );

        self.piece_dirty = false;
    }

    fn increase_rows_cleared(&mut self, new_rows_cleared: usize) {
        let level_before = self.get_level();
        self.rows_cleared += new_rows_cleared;

        if self.get_level() > level_before {
            self.level_flare_ticks_remaining = LEVEL_FLARE_TICKS;
        }
    }

    fn update_shrapnel(&mut self, dt: f32) {
        const GRAVITY: f32 = 28.0;
        const DRAG: f32 = 0.985;
        const WALL_X: f32 = (GRID_COUNT_COLS as f32 / 2.0) - 0.2;
        const BACK_WALL_Z: f32 = -0.35;
        const FRONT_Z: f32 = 0.35;

        // Debris sits on the melt heating up and then sinks, over roughly this
        // long once it lands.
        const SINK_SECONDS: f32 = 1.4;

        for splash in self.lava_splashes.iter_mut().filter(|splash| splash.active) {
            splash.age += dt;
            if splash.age >= SPLASH_SECONDS {
                splash.active = false;
            }
        }

        let mut landings: [(Vec3, f32); MAX_SHRAPNEL_VOXELS] =
            [(Vec3::ZERO, 0.0); MAX_SHRAPNEL_VOXELS];
        let mut landing_count = 0;

        for voxel in self.shrapnel_voxels.iter_mut() {
            if !voxel.active {
                continue;
            }

            voxel.age += dt;

            if voxel.is_sinking() {
                // Settle on the surface and slow the tumble as the metal
                // takes hold; retire once fully under.
                voxel.submersion += dt / SINK_SECONDS;
                voxel.rotation += voxel.angular_velocity * dt;
                voxel.angular_velocity *= 0.9;
                if voxel.submersion >= 1.0 {
                    voxel.active = false;
                }
                continue;
            }

            voxel.velocity.y -= GRAVITY * dt;
            voxel.velocity.x *= DRAG;
            voxel.velocity.z *= DRAG;

            voxel.position += voxel.velocity * dt;
            voxel.rotation += voxel.angular_velocity * dt;

            // Bounce off left/right well walls
            if voxel.position.x < -WALL_X && voxel.velocity.x < 0.0 {
                voxel.position.x = -WALL_X;
                voxel.velocity.x = -voxel.velocity.x * 0.45;
                voxel.angular_velocity *= 0.7;
            } else if voxel.position.x > WALL_X && voxel.velocity.x > 0.0 {
                voxel.position.x = WALL_X;
                voxel.velocity.x = -voxel.velocity.x * 0.45;
                voxel.angular_velocity *= 0.7;
            }

            // Bounce off recessed back wall
            if voxel.position.z < BACK_WALL_Z && voxel.velocity.z < 0.0 {
                voxel.position.z = BACK_WALL_Z;
                voxel.velocity.z = -voxel.velocity.z * 0.45;
            }

            // The well has no solid floor, only a grate. Debris that comes
            // down on a bar takes one bounce off it; debris over a gap, or
            // anything coming down a second time, drops through into the pit.
            // Below the grate the shaft is closed at the front by the grille.
            if voxel.position.y < FLOOR_Y && voxel.velocity.y < 0.0 {
                let over_bar = !floor_gap_contains(voxel.position.x);
                let above_grate = voxel.position.y > FLOOR_Y - 0.3;
                if over_bar && above_grate && voxel.bounce_count == 0 {
                    voxel.position.y = FLOOR_Y;
                    voxel.velocity.y = -voxel.velocity.y * 0.35;
                    voxel.velocity.x += if voxel.position.x >= 0.0 { 0.6 } else { -0.6 };
                    voxel.bounce_count += 1;
                }
            }
            if voxel.position.y < FLOOR_Y && voxel.position.z > FRONT_Z && voxel.velocity.z > 0.0 {
                voxel.position.z = FRONT_Z;
                voxel.velocity.z = -voxel.velocity.z * 0.45;
            }

            // Touching the melt starts the sink.
            if voxel.position.y - voxel.size * 0.5 <= LAVA_Y {
                voxel.position.y = LAVA_Y + voxel.size * 0.5;
                voxel.velocity = Vec3::ZERO;
                voxel.submersion = f32::EPSILON;
                landings[landing_count] = (voxel.position, voxel.size);
                landing_count += 1;
            }
        }

        for &(position, size) in &landings[..landing_count] {
            self.spawn_lava_splash(position, size);
        }
    }

    /// Start a splash ring where a piece of debris went into the melt. The
    /// pool is a ring buffer; with many landings at once the oldest splash is
    /// simply replaced, which is invisible amid the rest.
    fn spawn_lava_splash(&mut self, position: Vec3, size: f32) {
        self.lava_splashes[self.next_splash_index] = LavaSplash {
            position,
            age: 0.0,
            size,
            active: true,
        };
        self.next_splash_index = (self.next_splash_index + 1) % MAX_LAVA_SPLASHES;
    }

    fn spawn_shrapnel_for_cleared_blocks(
        &mut self,
        cleared_blocks: &[Option<ClearedBlock>; MAX_CLEARED_CELLS],
        cleared_count: usize,
        clear_count: usize,
    ) {
        let is_carnage = clear_count >= 4;

        let voxels_per_block = match clear_count {
            1 => 4,
            2 => 6,
            3 => 7,
            _ => 8,
        };

        let (min_size, max_size) = match clear_count {
            1 => (0.32, 0.38),
            2 => (0.34, 0.40),
            3 => (0.36, 0.44),
            _ => (0.38, 0.46),
        };

        let (min_vel_y, max_vel_y) = match clear_count {
            1 => (2.2, 5.2),
            2 => (2.8, 6.2),
            3 => (3.4, 7.2),
            _ => (4.0, 8.5),
        };

        let (min_vel_z, max_vel_z) = match clear_count {
            1 => (1.2, 3.0),
            2 => (1.8, 4.5),
            3 => (2.6, 6.0),
            _ => (3.5, 8.0),
        };

        let spin_speed = match clear_count {
            1 => 7.0,
            2 => 9.5,
            3 => 12.0,
            _ => 14.5,
        };

        const SUB_VOXEL_OFFSETS: [(f32, f32, f32); 8] = [
            (-0.22, -0.22, -0.22),
            (0.22, -0.22, -0.22),
            (-0.22, 0.22, -0.22),
            (0.22, 0.22, -0.22),
            (-0.22, -0.22, 0.22),
            (0.22, -0.22, 0.22),
            (-0.22, 0.22, 0.22),
            (0.22, 0.22, 0.22),
        ];

        let mut pool_index = 0;
        let mut rng = rand::thread_rng();

        for i in 0..cleared_count {
            let Some(cleared) = cleared_blocks[i] else {
                continue;
            };

            let block_center = cell_center(cleared.visible_row, cleared.col);

            let mut corner_indices = [0usize, 1, 2, 3, 4, 5, 6, 7];
            for k in 0..voxels_per_block {
                let swap_idx = rng.gen_range(k..8);
                corner_indices.swap(k, swap_idx);
            }

            for &corner_idx in &corner_indices[..voxels_per_block] {
                let start_index = pool_index;
                while pool_index < MAX_SHRAPNEL_VOXELS && self.shrapnel_voxels[pool_index].active {
                    pool_index += 1;
                }

                if pool_index >= MAX_SHRAPNEL_VOXELS {
                    pool_index = 0;
                    while pool_index < start_index && self.shrapnel_voxels[pool_index].active {
                        pool_index += 1;
                    }
                }

                let target_index = pool_index % MAX_SHRAPNEL_VOXELS;
                pool_index = (target_index + 1) % MAX_SHRAPNEL_VOXELS;

                let (ox, oy, oz) = SUB_VOXEL_OFFSETS[corner_idx];
                let jitter = Vec3::new(
                    rng.gen_range(-0.04..0.04),
                    rng.gen_range(-0.04..0.04),
                    rng.gen_range(-0.04..0.04),
                );
                let pos = block_center + Vec3::new(ox, oy, oz) + jitter;

                let local_dir = Vec3::new(ox, oy, oz).normalize_or_zero();
                let norm_x = (cleared.col as f32 - 4.5) / 4.5;
                let vel_x = norm_x * rng.gen_range(2.0..4.5)
                    + local_dir.x * rng.gen_range(1.0..2.5)
                    + rng.gen_range(-1.2..1.2);
                let vel_y =
                    rng.gen_range(min_vel_y..max_vel_y) + local_dir.y * rng.gen_range(0.5..1.8);
                let vel_z =
                    (rng.gen_range(min_vel_z..max_vel_z) + (local_dir.z.max(0.0) * 1.5)).max(0.2);

                let rot_vel = Vec3::new(
                    rng.gen_range(-spin_speed..spin_speed),
                    rng.gen_range(-spin_speed..spin_speed),
                    rng.gen_range(-spin_speed..spin_speed),
                );

                let size = rng.gen_range(min_size..max_size);

                self.shrapnel_voxels[target_index] = ShrapnelVoxel {
                    position: pos,
                    velocity: Vec3::new(vel_x, vel_y, vel_z),
                    rotation: Vec3::new(
                        rng.gen_range(0.0..std::f32::consts::TAU),
                        rng.gen_range(0.0..std::f32::consts::TAU),
                        rng.gen_range(0.0..std::f32::consts::TAU),
                    ),
                    angular_velocity: rot_vel,
                    color: cleared.color,
                    size,
                    age: 0.0,
                    submersion: 0.0,
                    bounce_count: 0,
                    is_carnage,
                    active: true,
                };
            }
        }
    }

    fn clear_filled_rows(&mut self) -> usize {
        let (rows_cleared, cleared_blocks, cleared_count) =
            self.grid_locked.clear_all_filled_rows_detailed();

        if rows_cleared > 0 {
            self.last_clear_count = rows_cleared;
            self.last_clear_row_mask = cleared_blocks[..cleared_count]
                .iter()
                .flatten()
                .fold(0, |mask, block| mask | (1 << block.visible_row));
            self.clear_effect_ticks_remaining = LINE_CLEAR_EFFECT_TICKS;
            self.spawn_shrapnel_for_cleared_blocks(&cleared_blocks, cleared_count, rows_cleared);
        }

        rows_cleared
    }

    fn resolve_lock_score(&mut self, spin: SpinKind) {
        let level = self.get_level();
        let lines = self.clear_filled_rows();
        let event = LockEvent { spin, lines, level };
        let (state, award) = score_lock(event, self.scoring_state);
        self.scoring_state = state;
        self.score += award.total();

        self.increase_rows_cleared(lines);
    }

    pub fn get_grid_locked(&self) -> &Grid {
        &self.grid_locked
    }

    pub fn get_grid_locked_mut(&mut self) -> &mut Grid {
        &mut self.grid_locked
    }

    /// Synthetic non-spin lock for rendering fixtures. Never borrow rotation
    /// provenance from the unrelated active piece.
    pub fn trigger_line_clear(&mut self) {
        self.resolve_lock_score(SpinKind::None);
    }

    /// End the run immediately, for the screenshot harness.
    pub fn trigger_game_over(&mut self) {
        self.end_game();
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

    pub fn get_rows_cleared(&self) -> usize {
        self.rows_cleared
    }

    pub fn get_level(&self) -> usize {
        // Minimum level is 1. Maximum is 20.
        (self.rows_cleared / 10 + 1).min(20)
    }

    pub fn get_gravity(&self) -> f64 {
        let level = self.get_level();
        let gravity_seconds = (0.8 - ((level - 1) as f64 * 0.007)).powi(level as i32 - 1);
        let rows_per_second = 1.0 / gravity_seconds;
        let gravity = rows_per_second / ROWS_PER_SECOND_PER_G;

        // Cap at conventional 1G (60 rows per second), not one row per new tick.
        gravity.min(1.0)
    }

    pub fn get_piece_previews(&self) -> [Piece; 3] {
        [
            self.bag_manager.peek(1),
            self.bag_manager.peek(2),
            self.bag_manager.peek(3),
        ]
    }

    pub fn get_held_piece(&self) -> Option<Piece> {
        self.held_piece
    }

    pub fn get_is_game_over(&self) -> bool {
        self.is_game_over
    }

    pub fn get_is_paused(&self) -> bool {
        self.is_paused
    }

    pub fn get_impact_effect(&self) -> f32 {
        self.impact_ticks_remaining as f32
            / (LOCK_IMPACT_TICKS + 12 * EFFECT_TICK_INTERVAL) as f32
    }

    pub fn get_clear_effect(&self) -> (usize, f32) {
        (
            self.last_clear_count,
            self.clear_effect_ticks_remaining as f32 / LINE_CLEAR_EFFECT_TICKS as f32,
        )
    }

    /// Visible rows involved in the most recent line clear, as a bitmask, for
    /// as long as the clear effect is running.
    pub fn get_clear_row_mask(&self) -> u32 {
        if self.clear_effect_ticks_remaining == 0 {
            0
        } else {
            self.last_clear_row_mask
        }
    }

    /// The most recent hard drop and how much of its streak remains (1.0 just
    /// after landing, fading to 0.0).
    pub fn get_hard_drop_trail(&self) -> Option<(HardDropTrail, f32)> {
        if self.hard_drop_trail_ticks_remaining == 0 {
            return None;
        }

        self.hard_drop_trail.map(|trail| {
            (
                trail,
                self.hard_drop_trail_ticks_remaining as f32 / HARD_DROP_TRAIL_TICKS as f32,
            )
        })
    }

    /// How strongly the lamps are flaring for a level-up (1.0 at the moment of
    /// the level change, fading to 0.0).
    pub fn get_level_flare(&self) -> f32 {
        self.level_flare_ticks_remaining as f32 / LEVEL_FLARE_TICKS as f32
    }

    /// How close the stack is to topping out, from 0.0 (comfortably low) to
    /// 1.0 (touching the ceiling), for the alert lighting.
    pub fn get_danger(&self) -> f32 {
        const DANGER_ROWS: f32 = 6.0;
        let highest_visible_row = (FIRST_VISIBLE_ROW_ID..GRID_COUNT_ROWS).find(|&row| {
            (0..GRID_COUNT_COLS).any(|col| self.grid_locked.has_block_at_cell(row, col))
        });

        match highest_visible_row {
            Some(row) => {
                let free_rows = (row - FIRST_VISIBLE_ROW_ID) as f32;
                (1.0 - free_rows / DANGER_ROWS).clamp(0.0, 1.0)
            }
            None => 0.0,
        }
    }

    pub fn get_impact_origins(&self) -> (&[(f32, f32)], Color) {
        (
            &self.impact_origins[..self.impact_origin_count],
            self.impact_color,
        )
    }

    pub fn get_shrapnel(&self) -> &[ShrapnelVoxel] {
        &self.shrapnel_voxels
    }

    pub fn get_lava_splashes(&self) -> &[LavaSplash] {
        &self.lava_splashes
    }
}

#[cfg(test)]
#[path = "game_state_rules_tests.rs"]
mod rules_tests;

#[cfg(test)]
#[path = "game_state_rotation_tests.rs"]
mod rotation_tests;

#[cfg(test)]
#[path = "game_state_scoring_tests.rs"]
mod scoring_tests;

#[cfg(test)]
mod tests {
    use super::impact_contact_origins;
    use crate::{
        block::Block,
        grid::{
            Grid, FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, GRID_COUNT_ROWS, VISIBLE_GRID_COUNT_ROWS,
        },
        piece::BlockCanvas,
    };
    use macroquad::prelude::WHITE;

    #[test]
    fn rotated_l_reports_each_contact_on_a_stepped_stack() {
        let mut locked = Grid::new();
        let block = Some(Block::new(WHITE));
        let active_row = FIRST_VISIBLE_ROW_ID as isize + 8;
        let upper_support = active_row as usize + 1;
        let lower_support = active_row as usize + 2;

        // Supporting bed:
        //    [e][f]
        // [g][h][i]
        locked
            .set_cell(upper_support, 4, block)
            .set_cell(upper_support, 5, block)
            .set_cell(lower_support, 3, block)
            .set_cell(lower_support, 4, block)
            .set_cell(lower_support, 5, block);

        // Clockwise-rotated L:
        // [a][b][c]
        // [d]
        let mut canvas: BlockCanvas = [[None; 5]; 5];
        canvas[0][0] = block;
        canvas[0][1] = block;
        canvas[0][2] = block;
        canvas[1][0] = block;

        let (contacts, contact_count) =
            impact_contact_origins(&locked, active_row, 3, &canvas, 2, 3);

        assert_eq!(contact_count, 3);
        assert_eq!(
            &contacts[..contact_count],
            &[
                (4.5, upper_support as f32),
                (5.5, upper_support as f32),
                (3.5, lower_support as f32),
            ]
        );
    }

    #[test]
    fn line_clears_spawn_active_shrapnel_voxels() {
        let high_score_manager = crate::high_score_manager::HighScoreManager::new();
        let mut game_state = super::GameState::new(&high_score_manager);

        // Fill bottom-most visible row (1-line clear)
        for col in 0..GRID_COUNT_COLS {
            game_state
                .grid_locked
                .set_cell(GRID_COUNT_ROWS - 1, col, Some(Block::new(WHITE)));
        }

        game_state.trigger_line_clear();

        let active_count = game_state
            .get_shrapnel()
            .iter()
            .filter(|v| v.active)
            .count();
        // 10 blocks * 4 voxels per block = 40 voxels
        assert_eq!(active_count, 40);

        // Advance physics by several ticks
        game_state.update_shrapnel(0.05);

        for voxel in game_state.get_shrapnel().iter().filter(|v| v.active) {
            assert!(voxel.age > 0.0);
        }
    }

    #[test]
    fn debris_falls_through_the_grate_splashes_and_sinks_into_the_melt() {
        let high_score_manager = crate::high_score_manager::HighScoreManager::new();
        let mut game_state = super::GameState::new(&high_score_manager);

        for col in 0..GRID_COUNT_COLS {
            game_state
                .grid_locked
                .set_cell(GRID_COUNT_ROWS - 1, col, Some(Block::new(WHITE)));
        }

        game_state.trigger_line_clear();
        assert!(game_state.get_lava_splashes().iter().all(|splash| !splash.active));

        // Run the burst for a second at 60 Hz: by then everything has fallen
        // through the grate, and some of it has reached the melt.
        let mut saw_below_floor = false;
        let mut saw_sinking = false;
        for _ in 0..60 {
            game_state.update_shrapnel(1.0 / 60.0);
            for voxel in game_state.get_shrapnel().iter().filter(|v| v.active) {
                saw_below_floor |= voxel.position.y < super::FLOOR_Y - 0.5;
                saw_sinking |= voxel.is_sinking();
                assert!(voxel.position.y >= super::LAVA_Y - 0.001, "nothing goes under");
                if voxel.is_sinking() {
                    assert!((voxel.position.y - voxel.size * 0.5 - super::LAVA_Y).abs() < 0.01);
                }
            }
        }
        assert!(saw_below_floor, "debris should drop through the open floor");
        assert!(saw_sinking, "debris should land in the melt");
        assert!(game_state.get_lava_splashes().iter().any(|splash| splash.active));

        // Given a few more seconds, every piece has sunk and been retired.
        for _ in 0..300 {
            game_state.update_shrapnel(1.0 / 60.0);
        }
        assert!(game_state.get_shrapnel().iter().all(|v| !v.active));
    }


    #[test]
    fn carnage_line_clears_spawn_maximum_shrapnel_voxels() {
        let high_score_manager = crate::high_score_manager::HighScoreManager::new();
        let mut game_state = super::GameState::new(&high_score_manager);

        // Fill bottom 4 visible rows (4-line clear / Carnage)
        for row in GRID_COUNT_ROWS - 4..GRID_COUNT_ROWS {
            for col in 0..GRID_COUNT_COLS {
                game_state.grid_locked.set_cell(row, col, Some(Block::new(WHITE)));
            }
        }

        game_state.trigger_line_clear();

        let active_count = game_state
            .get_shrapnel()
            .iter()
            .filter(|v| v.active)
            .count();
        // 40 blocks * 8 sub-voxels = 320 voxels (100% spawn rate)
        assert_eq!(active_count, 320);

        let carnage_count = game_state
            .get_shrapnel()
            .iter()
            .filter(|v| v.active && v.is_carnage)
            .count();
        assert_eq!(carnage_count, 320);
    }

    #[test]
    fn line_clears_report_which_visible_rows_went() {
        let high_score_manager = crate::high_score_manager::HighScoreManager::new();
        let mut game_state = super::GameState::new(&high_score_manager);
        let visible_rows = [VISIBLE_GRID_COUNT_ROWS - 3, VISIBLE_GRID_COUNT_ROWS - 1];

        for visible_row in visible_rows {
            let row = FIRST_VISIBLE_ROW_ID + visible_row;

            for col in 0..GRID_COUNT_COLS {
                game_state.grid_locked.set_cell(row, col, Some(Block::new(WHITE)));
            }
        }

        assert_eq!(game_state.get_clear_row_mask(), 0);
        game_state.trigger_line_clear();

        let expected_mask = visible_rows.iter().fold(0, |mask, &row| mask | (1 << row));
        assert_eq!(game_state.get_clear_row_mask(), expected_mask);
    }

    #[test]
    fn hard_drop_leaves_a_trail_from_where_the_piece_started() {
        let high_score_manager = crate::high_score_manager::HighScoreManager::new();
        let mut game_state = super::GameState::new(&high_score_manager);
        let start_row = game_state.active_piece_row;
        let color = game_state.active_piece.color;

        assert!(game_state.get_hard_drop_trail().is_none());
        game_state.hard_drop();

        let (trail, strength) = game_state.get_hard_drop_trail().expect("trail");
        assert_eq!(trail.start_row, start_row);
        assert!(trail.landing_row > start_row);
        assert_eq!(trail.color, color);
        assert!((strength - 1.0).abs() < 0.001);
    }

    #[test]
    fn danger_rises_as_the_stack_nears_the_ceiling() {
        let high_score_manager = crate::high_score_manager::HighScoreManager::new();
        let mut game_state = super::GameState::new(&high_score_manager);
        assert_eq!(game_state.get_danger(), 0.0);

        game_state
            .grid_locked
            .set_cell(GRID_COUNT_ROWS - 1, 0, Some(Block::new(WHITE)));
        assert_eq!(game_state.get_danger(), 0.0);

        game_state
            .grid_locked
            .set_cell(FIRST_VISIBLE_ROW_ID + 3, 0, Some(Block::new(WHITE)));
        let high = game_state.get_danger();
        assert!(high > 0.0 && high < 1.0);

        game_state
            .grid_locked
            .set_cell(FIRST_VISIBLE_ROW_ID, 0, Some(Block::new(WHITE)));
        assert_eq!(game_state.get_danger(), 1.0);
    }
}
