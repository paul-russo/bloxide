//! In-well presentation effects drawn during the 3D pass: embers drifting up
//! from the furnace, the streak left behind by a hard drop, and the flash of
//! rows being cleared.
//!
//! Embers are stateless: each one's position is a pure function of time and
//! its index, so they need no update step, cost nothing while paused, and are
//! identical on every run of the screenshot harness.

use macroquad::prelude::*;

use crate::game_state::HardDropTrail;
use crate::grid::{FIRST_VISIBLE_ROW_ID, VISIBLE_GRID_COUNT_ROWS};
use crate::lighting::SceneLights;
use crate::render3d::{cell_center, draw_quad, BLOCK_INSET, WELL_HEIGHT, WELL_WIDTH};

const EMBER_COUNT: usize = 26;

/// Embers live just in front of the back wall, so the stack occludes them and
/// they only show in the empty part of the well.
const EMBER_Z: f32 = -0.42;

/// Depth of the hard-drop streak: inside the blocks' volume, so the landed
/// piece and any neighbours hide it and it only shows in the cells the piece
/// fell through.
const TRAIL_Z: f32 = 0.0;

/// The clear flash sits just ahead of the block faces.
const FLASH_Z: f32 = BLOCK_INSET * 0.5 + 0.03;

fn hash01(index: usize, salt: u32) -> f32 {
    let mut h = (index as u32).wrapping_mul(0x9E37_79B1) ^ salt.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

/// Sparks rising from the furnace floor, swaying as they cool from yellow to
/// red and fade. Brightness tracks the furnace so they surge when it flares.
pub fn draw_embers(time: f64, lights: &SceneLights) {
    let half_w = WELL_WIDTH * 0.5;
    let floor_y = -WELL_HEIGHT * 0.5;
    let furnace = lights.furnace_level();

    for index in 0..EMBER_COUNT {
        let period = 2.4 + hash01(index, 1) * 2.2;
        let phase = ((time / period as f64) + hash01(index, 2) as f64).fract() as f32;
        let rise = 4.0 + hash01(index, 3) * 7.0;
        let sway = (time as f32 * (0.9 + hash01(index, 4)) + index as f32).sin() * 0.3;
        let x = (hash01(index, 5) * 2.0 - 1.0) * (half_w - 0.3) + sway * phase;
        let y = floor_y + 0.2 + phase * rise;
        let size = if hash01(index, 6) > 0.7 { 0.12 } else { 0.07 };
        let fade = (1.0 - phase).powf(1.6);
        let flicker = 0.7 + 0.3 * ((time as f32 * 17.0 + index as f32 * 3.1).sin() * 0.5 + 0.5);
        let heat = (1.0 - phase * 1.4).clamp(0.0, 1.0);
        let color = Color::new(
            1.0,
            0.25 + 0.55 * heat,
            0.05 + 0.15 * heat,
            (fade * flicker * furnace).clamp(0.0, 1.0),
        );

        draw_quad(
            Vec3::new(x - size * 0.5, y + size * 0.5, EMBER_Z),
            Vec3::X * size,
            Vec3::NEG_Y * size,
            None,
            [color; 4],
        );
    }
}

/// Translucent streaks down every column a hard-dropped piece fell through,
/// fading out upward and over time.
pub fn draw_hard_drop_trail(trail: &HardDropTrail, strength: f32) {
    let lines_dropped = (trail.landing_row - trail.start_row).max(0);
    if lines_dropped == 0 || strength <= 0.0 {
        return;
    }

    let well_top = WELL_HEIGHT * 0.5;
    let width = BLOCK_INSET * 0.7;
    let base = trail.color;
    let alpha = 0.4 * strength;

    for canvas_col in 0..trail.bounds_width {
        let Some(top_canvas_row) = (0..trail.bounds_height)
            .find(|&canvas_row| trail.canvas[canvas_row][canvas_col].is_some())
        else {
            continue;
        };

        let grid_col = trail.col + canvas_col as isize;
        if grid_col < 0 {
            continue;
        }

        // The streak covers where this column's topmost block started down to
        // where it landed; the landed piece itself covers the rest.
        let start_grid_row = trail.start_row + top_canvas_row as isize;
        let end_grid_row = trail.landing_row + top_canvas_row as isize;
        let top_y = (well_top - (start_grid_row - FIRST_VISIBLE_ROW_ID as isize) as f32)
            .min(well_top);
        let bottom_y = cell_center(
            (end_grid_row - FIRST_VISIBLE_ROW_ID as isize).max(0) as usize,
            grid_col as usize,
        )
        .y + 0.5;
        if bottom_y >= top_y {
            continue;
        }

        let center_x = cell_center(0, grid_col as usize).x;
        let faint = Color::new(base.r, base.g, base.b, 0.0);
        let bright = Color::new(
            (base.r * 1.4).min(1.0),
            (base.g * 1.4).min(1.0),
            (base.b * 1.4).min(1.0),
            alpha,
        );

        draw_quad(
            Vec3::new(center_x - width * 0.5, top_y, TRAIL_Z),
            Vec3::X * width,
            Vec3::NEG_Y * (top_y - bottom_y),
            None,
            [faint, faint, bright, bright],
        );
    }
}

/// A hot flash across each cleared row during the first part of the clear
/// effect: white-hot at the instant of the clear, cooling through amber as it
/// fades. Larger clears flash harder.
pub fn draw_clear_flash(row_mask: u32, clear_remaining: f32, clear_count: usize) {
    const FLASH_PORTION: f32 = 0.3;
    let progress = ((clear_remaining - (1.0 - FLASH_PORTION)) / FLASH_PORTION).clamp(0.0, 1.0);
    if row_mask == 0 || progress <= 0.0 {
        return;
    }

    let heat = progress.powf(1.5);
    let intensity = (0.45 + 0.15 * clear_count as f32).min(1.0);
    let color = Color::new(
        1.0,
        0.55 + 0.45 * heat,
        0.15 + 0.75 * heat,
        heat * intensity,
    );
    let half_w = WELL_WIDTH * 0.5;

    for visible_row in 0..VISIBLE_GRID_COUNT_ROWS {
        if row_mask & (1 << visible_row) == 0 {
            continue;
        }

        let top_y = cell_center(visible_row, 0).y + 0.5;
        draw_quad(
            Vec3::new(-half_w, top_y, FLASH_Z),
            Vec3::X * WELL_WIDTH,
            Vec3::NEG_Y,
            None,
            [color; 4],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ember_hash_is_deterministic_and_in_unit_range() {
        for index in 0..EMBER_COUNT {
            for salt in 1..7 {
                let value = hash01(index, salt);
                assert_eq!(value, hash01(index, salt));
                assert!((0.0..1.0).contains(&value));
            }
        }
    }
}
