use crate::block::Block;
use crate::effects::{draw_clear_flash, draw_embers, draw_hard_drop_trail};
use crate::game_state::GameState;
use crate::grid::{Grid, FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, GRID_COUNT_ROWS};
use crate::high_score_manager::HighScoreManager;
use crate::lighting::{lit, SceneLights};
use crate::menu::Menu;
use crate::piece::Piece;
use crate::pixel_font::{
    digit_text_width, draw_digit_text, draw_small_text, small_text_width, DIGIT_HEIGHT,
    SMALL_GLYPH_HEIGHT,
};
use crate::postfx::PostProcess;
use crate::render3d::{
    cell_center, draw_block_cube, draw_block_cube_scaled, draw_ghost_cell, draw_lamp_glow,
    draw_quad, draw_quad_uv, draw_shrapnel, draw_tumbling_cube, draw_well, lintel_front_bounds,
    screen_to_world_on_plane, well_camera, world_to_screen, world_to_screen_with_shake,
    BLOCK_INSET, RENDER_HEIGHT, RENDER_WIDTH, WELL_DEPTH, WELL_HEIGHT, WELL_WIDTH,
};
use crate::textures::{SceneTextures, BLOCK_TEXTURE_SIZE, STONE_TEXTURE_SIZE};
use macroquad::prelude::*;
use macroquad::texture::{render_target_ex, RenderTargetParams};
use num_format::{Locale, ToFormattedString};
use std::rc::Rc;

/// The window is sized for the 3D scene rather than for a pixel-exact 2D grid:
/// it has to fit the well plus a side panel on each side, with the well framed
/// by [`crate::render3d::well_camera`].
pub const WINDOW_WIDTH: f32 = 1200.0;
pub const WINDOW_HEIGHT: f32 = 900.0;

/// Cleared before anything is drawn. The stone backdrop covers the whole frame,
/// so this only shows through if a tile ever fails to.
pub const BACKGROUND_COLOR: Color = color_u8!(10, 9, 7, 255);

#[derive(Clone)]
pub struct RenderSurface {
    pub target: RenderTarget,
    pub textures: SceneTextures,
    post_process: Rc<PostProcess>,
}

impl RenderSurface {
    pub fn new() -> Self {
        // `render_target()` has no depth attachment. A Camera3D can still be
        // pointed at it, but overlapping geometry then falls back to submission
        // order and lower grid rows paint over upper ones. The explicit depth
        // target makes the cube stack independent of traversal order.
        let target = render_target_ex(
            RENDER_WIDTH,
            RENDER_HEIGHT,
            RenderTargetParams {
                depth: true,
                ..Default::default()
            },
        );
        target.texture.set_filter(FilterMode::Nearest);
        Self {
            target,
            textures: SceneTextures::new(),
            post_process: Rc::new(PostProcess::new(RENDER_WIDTH, RENDER_HEIGHT)),
        }
    }

    fn camera_2d(&self) -> Camera2D {
        let mut camera = Camera2D::from_display_rect(Rect::new(
            0.0,
            0.0,
            RENDER_WIDTH as f32,
            RENDER_HEIGHT as f32,
        ));
        camera.render_target = Some(self.target.clone());
        camera
    }

    fn camera_3d(&self, shake: Vec2) -> Camera3D {
        well_camera(Some(self.target.clone()), shake)
    }

    pub fn begin_frame(&self) {
        set_camera(&self.camera_2d());
        clear_background(BACKGROUND_COLOR);
    }

    fn restore_2d(&self) {
        set_camera(&self.camera_2d());
    }

    pub fn present(&self) {
        set_default_camera();
        clear_background(color_u8!(2, 2, 2, 255));

        let integer_scale = (screen_width() / RENDER_WIDTH as f32)
            .floor()
            .min((screen_height() / RENDER_HEIGHT as f32).floor())
            .max(1.0);
        let size = vec2(
            RENDER_WIDTH as f32 * integer_scale,
            RENDER_HEIGHT as f32 * integer_scale,
        );
        let origin = vec2(
            ((screen_width() - size.x) * 0.5).floor(),
            ((screen_height() - size.y) * 0.5).floor(),
        );

        self.post_process.blit(&self.target.texture, origin, size);
    }
}

/// Dark inset face of an instrument panel.
const COLOR_PANEL: Color = color_u8!(20, 20, 18, 255);

/// Slightly darker nameplate strip across the top of a panel's face.
const COLOR_PANEL_HEADER: Color = color_u8!(14, 14, 13, 255);

/// Tint for the gunmetal frame material before lighting.
const COLOR_PANEL_FRAME: Color = color_u8!(96, 98, 96, 255);

/// Outer bevel highlight along a frame's lit edges.
const COLOR_PANEL_LIGHT: Color = color_u8!(156, 156, 150, 255);

/// Lit lower edge of a recessed opening.
const COLOR_PANEL_INNER_LIGHT: Color = color_u8!(66, 66, 62, 255);

/// Shadowed bevel edges, both outer and inner.
const COLOR_PANEL_DARK: Color = color_u8!(5, 5, 4, 255);

/// Divider rules inside a panel face.
const COLOR_PANEL_BORDER: Color = color_u8!(78, 74, 62, 255);

/// Dark copy drawn under engraved labels.
const COLOR_ENGRAVE_SHADOW: Color = color_u8!(4, 4, 3, 230);

const COLOR_SCREW: Color = color_u8!(150, 152, 148, 255);
const COLOR_AMBER: Color = color_u8!(231, 148, 49, 255);
const COLOR_TEXT: Color = color_u8!(242, 229, 193, 255);
const COLOR_TEXT_MUTED: Color = color_u8!(169, 157, 121, 255);

/// Horizontal/depth references for preview pieces. Their world-space `y` is
/// derived from the card's screen-space content area so previews stay centred
/// and padded even when the camera or panel layout changes.
const HOLD_PREVIEW_REFERENCE: Vec3 = Vec3::new(-10.2, 0.0, 0.0);
const NEXT_PREVIEW_REFERENCE: Vec3 = Vec3::new(10.2, 0.0, 0.0);

/// Edge length of a preview block. Pieces outside the well are drawn smaller
/// than playfield blocks so a 4-wide I piece fits the side panel.
const PREVIEW_SCALE: f32 = 0.74;

/// How far the stack's colours drain toward dead steel once the game is over.
const GAME_OVER_WASH: f32 = 0.7;

const LABEL_TEXT_SIZE: f32 = 20.0;
const STATS_TEXT_SIZE: f32 = 18.0;
const MENU_TITLE_TEXT_SIZE: f32 = 42.0;
const MENU_ITEM_TEXT_SIZE: f32 = 19.0;

/// Vertical pitch between stacked menu entries.
const MENU_ITEM_HEIGHT: f32 = 48.0;

/// Padding above the menu title and below the last menu item.
const MENU_PADDING_Y: f32 = 30.0;

/// Width of the menu panel, as a fraction of the well's on-screen width.
const MENU_WIDTH_SCALE: f32 = 1.34;

/// Shared HUD measurements are expressed in logical window points and snapped
/// to the low-resolution framebuffer by [`hud_layout`]. Keeping them together
/// prevents independent panels from drifting onto different pixel phases.
const HUD_PANEL_WIDTH: f32 = 176.0;
const HUD_SIDE_GAP: f32 = 88.0;
const HUD_TOP_OFFSET: f32 = -6.0;
const HUD_LOWER_OFFSET: f32 = 320.0;
const HUD_HOLD_HEIGHT: f32 = 144.0;
const HUD_NEXT_HEIGHT: f32 = 280.0;
const HUD_LOWER_HEIGHT: f32 = 164.0;
const HUD_HEADER_DIVIDER_OFFSET: f32 = 36.0;
const HUD_PREVIEW_TOP_PADDING: f32 = 12.0;
const HUD_PREVIEW_BOTTOM_PADDING: f32 = 12.0;

/// How the blocks of a grid should be rendered inside the well.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum BlockStyle {
    /// Solid shaded cubes, used for the locked stack and the active piece.
    Solid,
    /// Wireframe outlines, used for the hard-drop landing preview so it never
    /// obscures the stack the player is aiming at.
    Ghost,
}

/// Ratio between the framebuffer and the logical window size.
///
/// With `high_dpi` enabled, macroquad's default 2D camera maps to the physical
/// framebuffer, which on a Retina display is larger than the window we asked
/// for. Every HUD size and gap in this file is written in logical points, so
/// each one is scaled by this to reach its intended on-screen size instead of
/// rendering at half the size on such displays.
fn hud_scale() -> f32 {
    RENDER_HEIGHT as f32 / WINDOW_HEIGHT
}

fn frame_width() -> f32 {
    RENDER_WIDTH as f32
}

fn frame_height() -> f32 {
    RENDER_HEIGHT as f32
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + ((b.r - a.r) * t),
        a.g + ((b.g - a.g) * t),
        a.b + ((b.b - a.b) * t),
        a.a + ((b.a - a.a) * t),
    )
}

/// Glyph pixel size for a label of `size` logical points: the 5x7 face is
/// scaled so its cell height matches the requested size on screen.
fn text_pixel(size: f32) -> f32 {
    (size * hud_scale() / SMALL_GLYPH_HEIGHT).round().max(1.0)
}

/// Width, height and glyph pixel size of `text` set in the label face at
/// `size` logical points.
fn pixel_text_metrics(text: &str, size: f32) -> (f32, f32, f32) {
    let pixel = text_pixel(size);
    (small_text_width(text, pixel), SMALL_GLYPH_HEIGHT * pixel, pixel)
}

fn draw_pixel_text(text: &str, x: f32, baseline_y: f32, size: f32, color: Color) {
    draw_small_text(text, x, baseline_y, text_pixel(size), color);
}

/// Labels cut into a metal plate: a dark offset copy under the lit glyphs.
fn draw_engraved_text(text: &str, x: f32, baseline_y: f32, size: f32, color: Color) {
    draw_pixel_text(text, x + 1.0, baseline_y + 1.0, size, COLOR_ENGRAVE_SHADOW);
    draw_pixel_text(text, x, baseline_y, size, color);
}

/// Fill a screen-space rectangle with a tiled material, one texel per pixel,
/// each tile lit at its corners by the scene lights as if the rectangle were a
/// plate mounted on the wall at `PANEL_Z`. The last column and row of tiles
/// are clipped through their UVs rather than stretched.
fn draw_tiled_rect(rect: Rect, texture: &Texture2D, tile: f32, tint: Color, lights: &SceneLights) {
    let columns = (rect.w / tile).ceil() as usize;
    let rows = (rect.h / tile).ceil() as usize;

    for row in 0..rows {
        for column in 0..columns {
            let x = rect.x + column as f32 * tile;
            let y = rect.y + row as f32 * tile;
            let width = tile.min(rect.x + rect.w - x);
            let height = tile.min(rect.y + rect.h - y);
            let origin = Vec3::new(x, y, 0.0);
            let corners = [
                origin,
                origin + Vec3::X * width,
                origin + Vec3::new(width, height, 0.0),
                origin + Vec3::Y * height,
            ];
            let colors = corners.map(|corner| {
                lit(tint, lights.at(screen_to_world_on_plane(corner.truncate(), PANEL_Z)))
            });

            draw_quad_uv(
                origin,
                Vec3::X * width,
                Vec3::Y * height,
                Vec2::ZERO,
                Vec2::new(width / tile, height / tile),
                Some(texture),
                colors,
            );
        }
    }
}

/// One-pixel chamfer around `rect`: lit along the top and left, shadowed along
/// the bottom and right. `recessed` swaps them for an inset opening.
fn draw_bevel(rect: Rect, light: Color, dark: Color, recessed: bool) {
    let (top_left, bottom_right) = if recessed {
        (dark, light)
    } else {
        (light, dark)
    };

    draw_rectangle(rect.x, rect.y, rect.w, 1.0, top_left);
    draw_rectangle(rect.x, rect.y, 1.0, rect.h, top_left);
    draw_rectangle(rect.x, rect.y + rect.h - 1.0, rect.w, 1.0, bottom_right);
    draw_rectangle(rect.x + rect.w - 1.0, rect.y, 1.0, rect.h, bottom_right);
}

/// A 3x3 slotted screw head.
fn draw_screw(center_x: f32, center_y: f32) {
    let x = center_x.round() - 1.0;
    let y = center_y.round() - 1.0;
    draw_rectangle(x, y, 3.0, 3.0, shaded(COLOR_SCREW, 0.55));
    draw_rectangle(x, y, 2.0, 2.0, shaded(COLOR_SCREW, 1.05));
    draw_rectangle(x, y, 1.0, 1.0, shaded(COLOR_SCREW, 1.4));
    draw_rectangle(x, y + 1.0, 3.0, 1.0, shaded(COLOR_SCREW, 0.35));
}

fn shaded(color: Color, shade: f32) -> Color {
    Color::new(
        (color.r * shade).clamp(0.0, 1.0),
        (color.g * shade).clamp(0.0, 1.0),
        (color.b * shade).clamp(0.0, 1.0),
        color.a,
    )
}

/// Thickness of an instrument panel's steel frame, in framebuffer pixels.
const PANEL_FRAME: f32 = 6.0;

/// Depth at which HUD plates are considered to hang for lighting purposes:
/// on the wall, level with the cabinet's front.
const PANEL_Z: f32 = 1.0;

/// A recessed instrument housing in the spirit of mid-90s shooter HUDs: a
/// riveted gunmetal frame, lit by the room, around a dark inset face. The
/// frame is neutral; amber is reserved for live state and selection.
fn draw_instrument_panel(rect: Rect, textures: &SceneTextures, lights: &SceneLights) {
    let scale = hud_scale();
    let shadow = (6.0 * scale).round().max(1.0);
    let inner = Rect::new(
        rect.x + PANEL_FRAME,
        rect.y + PANEL_FRAME,
        rect.w - PANEL_FRAME * 2.0,
        rect.h - PANEL_FRAME * 2.0,
    );

    draw_rectangle(
        rect.x + shadow,
        rect.y + shadow,
        rect.w,
        rect.h,
        color_u8!(3, 3, 2, 200),
    );
    draw_tiled_rect(
        rect,
        textures.gunmetal(),
        BLOCK_TEXTURE_SIZE as f32,
        COLOR_PANEL_FRAME,
        lights,
    );
    draw_bevel(rect, COLOR_PANEL_LIGHT, COLOR_PANEL_DARK, false);

    draw_rectangle(inner.x, inner.y, inner.w, inner.h, COLOR_PANEL);
    draw_bevel(inner, COLOR_PANEL_INNER_LIGHT, COLOR_PANEL_DARK, true);

    for (x, y) in [
        (rect.x + 3.0, rect.y + 3.0),
        (rect.x + rect.w - 3.0, rect.y + 3.0),
        (rect.x + 3.0, rect.y + rect.h - 3.0),
        (rect.x + rect.w - 3.0, rect.y + rect.h - 3.0),
    ] {
        draw_screw(x, y);
    }
}

fn hash01(seed: f32) -> f32 {
    ((seed * 12.9898).sin() * 43_758.547).fract().abs()
}

/// Depth of the stone wall behind the cabinet, used to place it in the scene's
/// lighting. It sits a little behind the well's back wall.
const BACKDROP_WALL_Z: f32 = -1.0;

/// Base tint of the backdrop masonry before lighting. Under ambient alone the
/// wall is barely there; it is the lamps and the furnace that reveal it.
const COLOR_STONE: Color = color_u8!(70, 68, 64, 255);

/// Cut-stone wall behind the cabinet, lit by the same lamps and furnace as the
/// 3D scene. Each tile's corners sample the scene lights at the world position
/// beneath that pixel, so the pools on the stone line up with the fixtures that
/// cast them.
pub fn draw_background(textures: &SceneTextures, lights: &SceneLights) {
    let tile = STONE_TEXTURE_SIZE as f32;
    let columns = (frame_width() / tile).ceil() as usize;
    let rows = (frame_height() / tile).ceil() as usize;

    for row in 0..rows {
        for column in 0..columns {
            let top_left = Vec3::new(column as f32 * tile, row as f32 * tile, 0.0);
            let light_at = |corner: Vec3| {
                lights.at(screen_to_world_on_plane(corner.truncate(), BACKDROP_WALL_Z))
            };
            let corners = [
                top_left,
                top_left + Vec3::X * tile,
                top_left + Vec3::new(tile, tile, 0.0),
                top_left + Vec3::Y * tile,
            ];
            let colors = corners.map(|corner| lit(COLOR_STONE, light_at(corner)));

            draw_quad(
                top_left,
                Vec3::X * tile,
                Vec3::Y * tile,
                Some(textures.stone()),
                colors,
            );
        }
    }
}

/// Draw `text` horizontally centred on `center_x`, sitting on `baseline_y`.
///
/// Position arguments are in framebuffer pixels, since they normally come
/// straight out of [`world_to_screen`]. `size` is in logical points and is
/// scaled by [`hud_scale`] here.
fn draw_text_centered_at(text: &str, center_x: f32, baseline_y: f32, size: f32, color: Color) {
    let (width, _, _) = pixel_text_metrics(text, size);
    draw_pixel_text(text, center_x - (width * 0.5), baseline_y, size, color);
}

fn draw_text_right_at(text: &str, right_x: f32, baseline_y: f32, size: f32, color: Color) {
    let (width, _, _) = pixel_text_metrics(text, size);
    draw_pixel_text(text, right_x - width, baseline_y, size, color);
}

/// Screen-space rectangle covering the front face of the well opening.
///
/// 2D overlays are positioned against this rather than against window constants
/// so they stay registered with the 3D playfield if the camera or window changes.
fn well_screen_rect() -> Rect {
    let front_z = WELL_DEPTH / 2.0;
    let top_left = world_to_screen(Vec3::new(-WELL_WIDTH / 2.0, WELL_HEIGHT / 2.0, front_z));
    let bottom_right = world_to_screen(Vec3::new(WELL_WIDTH / 2.0, -WELL_HEIGHT / 2.0, front_z));

    Rect::new(
        top_left.x,
        top_left.y,
        bottom_right.x - top_left.x,
        bottom_right.y - top_left.y,
    )
}

fn snap_rect(rect: Rect) -> Rect {
    Rect::new(
        rect.x.round(),
        rect.y.round(),
        rect.w.round(),
        rect.h.round(),
    )
}

/// Screen-space rectangle covering the front face of the lintel above the well,
/// where the score readout is mounted.
fn lintel_screen_rect() -> Rect {
    let (top_left, bottom_right) = lintel_front_bounds();
    let top_left = world_to_screen(top_left);
    let bottom_right = world_to_screen(bottom_right);

    snap_rect(Rect::new(
        top_left.x,
        top_left.y,
        bottom_right.x - top_left.x,
        bottom_right.y - top_left.y,
    ))
}

#[derive(Copy, Clone)]
struct HudLayout {
    hold: Rect,
    next: Rect,
    controls: Rect,
    stats: Rect,
    score: Rect,
}

fn hud_layout() -> HudLayout {
    let scale = hud_scale();
    let well = well_screen_rect();
    let panel_width = (HUD_PANEL_WIDTH * scale).round();
    let side_gap = (HUD_SIDE_GAP * scale).round();
    let top_y = (well.y + HUD_TOP_OFFSET * scale).round();
    let lower_y = (well.y + HUD_LOWER_OFFSET * scale).round();
    let left_x = (well.x - side_gap - panel_width).round();
    let right_x = (well.x + well.w + side_gap).round();

    HudLayout {
        hold: Rect::new(
            left_x,
            top_y,
            panel_width,
            (HUD_HOLD_HEIGHT * scale).round(),
        ),
        next: Rect::new(
            right_x,
            top_y,
            panel_width,
            (HUD_NEXT_HEIGHT * scale).round(),
        ),
        controls: Rect::new(
            left_x,
            lower_y,
            panel_width,
            (HUD_LOWER_HEIGHT * scale).round(),
        ),
        stats: Rect::new(
            right_x,
            lower_y,
            panel_width,
            (HUD_LOWER_HEIGHT * scale).round(),
        ),
        score: lintel_screen_rect(),
    }
}

fn hold_card_rect() -> Rect {
    hud_layout().hold
}

fn next_card_rect() -> Rect {
    hud_layout().next
}

fn stats_card_rect() -> Rect {
    hud_layout().stats
}

fn controls_card_rect() -> Rect {
    hud_layout().controls
}

fn preview_content_vertical_bounds(rect: Rect) -> (f32, f32) {
    let scale = hud_scale();
    (
        rect.y + ((HUD_HEADER_DIVIDER_OFFSET + HUD_PREVIEW_TOP_PADDING) * scale),
        rect.y + rect.h - (HUD_PREVIEW_BOTTOM_PADDING * scale),
    )
}

fn preview_anchor_at_screen_y(reference: Vec3, target_y: f32) -> Vec3 {
    let reference_y = world_to_screen(reference).y;
    let one_world_unit_down = world_to_screen(reference - Vec3::Y).y;
    let pixels_per_world_unit = one_world_unit_down - reference_y;
    debug_assert!(pixels_per_world_unit > 0.0);

    Vec3::new(
        reference.x,
        reference.y - ((target_y - reference_y) / pixels_per_world_unit),
        reference.z,
    )
}

fn hold_piece_anchor() -> Vec3 {
    let (content_top, content_bottom) = preview_content_vertical_bounds(hold_card_rect());
    preview_anchor_at_screen_y(
        HOLD_PREVIEW_REFERENCE,
        ((content_top + content_bottom) * 0.5).round(),
    )
}

fn next_piece_anchor(index: usize) -> Vec3 {
    debug_assert!(index < 3);
    let (content_top, content_bottom) = preview_content_vertical_bounds(next_card_rect());
    let slot_height = (content_bottom - content_top) / 3.0;
    let target_y = content_top + slot_height * (index as f32 + 0.5);
    preview_anchor_at_screen_y(NEXT_PREVIEW_REFERENCE, target_y.round())
}

/// Nameplate strip across the top of a panel face, with the label engraved
/// into it and a rule underneath.
fn draw_panel_header(rect: Rect, label: &str) {
    let scale = hud_scale();
    let inset = PANEL_FRAME + 1.0;
    let left = rect.x + (16.0 * scale);
    let right = rect.x + rect.w - (16.0 * scale);
    let baseline = rect.y + (24.0 * scale);
    let divider_y = rect.y + (HUD_HEADER_DIVIDER_OFFSET * scale);

    draw_rectangle(
        rect.x + inset,
        rect.y + inset,
        rect.w - inset * 2.0,
        divider_y - rect.y - inset,
        COLOR_PANEL_HEADER,
    );
    draw_engraved_text(label, left, baseline, LABEL_TEXT_SIZE, COLOR_TEXT_MUTED);
    draw_line(left, divider_y, right, divider_y, 1.0, COLOR_PANEL_BORDER);
}

fn draw_next_slot_guides() {
    let rect = next_card_rect();
    let (content_top, content_bottom) = preview_content_vertical_bounds(rect);
    let slot_height = (content_bottom - content_top) / 3.0;
    let inset = 16.0 * hud_scale();

    for slot in 1..3 {
        let y = (content_top + slot_height * slot as f32).round();
        draw_line(
            rect.x + inset,
            y,
            rect.x + rect.w - inset,
            y,
            1.0,
            color_u8!(69, 65, 54, 120),
        );
    }
}

fn draw_game_chrome(textures: &SceneTextures, lights: &SceneLights) {
    draw_instrument_panel(hold_card_rect(), textures, lights);
    draw_instrument_panel(next_card_rect(), textures, lights);
    draw_instrument_panel(stats_card_rect(), textures, lights);
    draw_instrument_panel(controls_card_rect(), textures, lights);
}

/// Render the empty well on its own, as a backdrop for the main menu screen.
///
/// Leaves the default 2D camera active so the caller can draw menu text
/// immediately afterwards.
pub fn draw_backdrop(surface: &RenderSurface) {
    let time = get_time();
    let lights = SceneLights::idle(time);
    draw_background(&surface.textures, &lights);
    set_camera(&surface.camera_3d(Vec2::ZERO));
    draw_well(&surface.textures, &lights);
    draw_embers(time, &lights);
    draw_lamp_glow(&lights);
    surface.restore_2d();
}

/// Glyph pixel size of the bold readout numerals.
const READOUT_PIXEL: f32 = 2.0;

/// Padding between a readout window's glass and its digits, in pixels.
const READOUT_PADDING: f32 = 4.0;

/// A dark glass window let into a metal face, with bold numerals glowing
/// inside it. `text` is right-aligned so growing numbers extend left, the way
/// a mechanical counter reads; `ghost` is the counter's full width of unlit
/// positions, shown faintly behind the live digits.
fn draw_readout_window(rect: Rect, text: &str, ghost: &str, color: Color) {
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, color_u8!(6, 6, 5, 255));
    draw_bevel(rect, COLOR_PANEL_INNER_LIGHT, color_u8!(1, 1, 1, 255), true);

    let top = rect.y + ((rect.h - DIGIT_HEIGHT as f32 * READOUT_PIXEL) * 0.5).round();
    let right = rect.x + rect.w - READOUT_PADDING;
    let ghost_x = right - digit_text_width(ghost, READOUT_PIXEL);
    let x = right - digit_text_width(text, READOUT_PIXEL);

    draw_digit_text(ghost, ghost_x, top, READOUT_PIXEL, shaded(color, 0.07));

    // A dim copy under the lit digits gives them a faint phosphor bloom.
    draw_digit_text(text, x + 1.0, top + 1.0, READOUT_PIXEL, shaded(color, 0.25));
    draw_digit_text(text, x, top, READOUT_PIXEL, color);
}

/// Height of a readout window tall enough for the bold numerals.
fn readout_height() -> f32 {
    DIGIT_HEIGHT as f32 * READOUT_PIXEL + READOUT_PADDING * 2.0
}

/// Draw the score readout mounted on the lintel above the well: an engraved
/// label beside a glass window, centred together on the beam.
fn draw_score(score: usize) {
    let text = score.to_formatted_string(&Locale::en);
    let rect = hud_layout().score;
    let center_y = rect.y + rect.h * 0.5;
    let (label_width, label_height, _) = pixel_text_metrics("SCORE", 18.0);
    let gap = 8.0;
    let ghost = "0,000,000";
    let window_width = (digit_text_width(ghost, READOUT_PIXEL) + READOUT_PADDING * 2.0)
        .max(digit_text_width(&text, READOUT_PIXEL) + READOUT_PADDING * 2.0);
    let window_height = readout_height();
    let group_width = label_width + gap + window_width;
    let group_x = (rect.x + (rect.w - group_width) * 0.5).round();

    draw_engraved_text(
        "SCORE",
        group_x,
        (center_y + label_height * 0.5).round(),
        18.0,
        COLOR_TEXT_MUTED,
    );
    draw_readout_window(
        Rect::new(
            group_x + label_width + gap,
            (center_y - window_height * 0.5).round(),
            window_width,
            window_height,
        ),
        &text,
        ghost,
        COLOR_AMBER,
    );
}

/// Draw the captions for the hold slot and the next queue.
///
/// These live in the 2D pass rather than beside the cubes they label: `draw_text`
/// emits screen-space geometry, so calling it while the 3D camera is active
/// would push the glyphs through the perspective matrix and off the frame.
fn draw_panel_labels() {
    draw_panel_header(hold_card_rect(), "HOLD");
    draw_panel_header(next_card_rect(), "NEXT");
    draw_panel_header(controls_card_rect(), "CONTROLS");
    draw_panel_header(stats_card_rect(), "STATUS");
    draw_next_slot_guides();
}

/// One indicator lamp in the level-progress bar.
fn draw_indicator_pip(x: f32, y: f32, size: f32, lit: bool) {
    draw_rectangle(x, y, size, size, color_u8!(8, 8, 7, 255));
    if lit {
        draw_rectangle(x + 1.0, y + 1.0, size - 2.0, size - 2.0, COLOR_AMBER);
        draw_rectangle(x + 1.0, y + 1.0, 1.0, 1.0, COLOR_TEXT);
    } else {
        draw_rectangle(x + 1.0, y + 1.0, size - 2.0, size - 2.0, color_u8!(44, 42, 38, 255));
    }
}

/// Draw the level and cleared-line counts beneath the next queue, as two bold
/// readouts over a row of ten indicator lamps counting lines toward the next
/// level.
fn draw_level_and_rows_cleared(level: usize, rows_cleared: usize) {
    let scale = hud_scale();
    let rect = stats_card_rect();
    let mid_x = (rect.x + rect.w * 0.5).round();
    let label_baseline = rect.y + (58.0 * scale);
    let window_top = rect.y + (64.0 * scale);
    let window_width = digit_text_width("00", READOUT_PIXEL) + READOUT_PADDING * 2.0;

    for (label, value, center_x) in [
        ("LEVEL", level, rect.x + rect.w * 0.25),
        ("LINES", rows_cleared, rect.x + rect.w * 0.75),
    ] {
        let (label_width, _, _) = pixel_text_metrics(label, STATS_TEXT_SIZE);
        draw_engraved_text(
            label,
            (center_x - label_width * 0.5).round(),
            label_baseline,
            STATS_TEXT_SIZE,
            COLOR_TEXT_MUTED,
        );
        draw_readout_window(
            Rect::new(
                (center_x - window_width * 0.5).round(),
                window_top,
                window_width,
                readout_height(),
            ),
            &format!("{:02}", value),
            "00",
            COLOR_TEXT,
        );
    }

    draw_line(
        mid_x,
        rect.y + (48.0 * scale),
        mid_x,
        window_top + readout_height(),
        1.0,
        color_u8!(72, 68, 57, 180),
    );

    let pip = 5.0;
    let gap = 2.0;
    let bar_width = pip * 10.0 + gap * 9.0;
    let bar_x = (rect.x + (rect.w - bar_width) * 0.5).round();
    let bar_y = rect.y + rect.h - PANEL_FRAME - (12.0 * scale) - pip;
    let completed = rows_cleared % 10;
    for segment in 0..10 {
        draw_indicator_pip(
            bar_x + segment as f32 * (pip + gap),
            bar_y,
            pip,
            segment < completed,
        );
    }
}

/// A raised keycap: lit along its top and left, shadowed bottom and right.
fn draw_keycap(label: &str, x: f32, y: f32, width: f32) {
    let scale = hud_scale();
    let rect = Rect::new(x, y, (width * scale).round(), (22.0 * scale).round());
    draw_rectangle(rect.x + 1.0, rect.y + 1.0, rect.w, rect.h, color_u8!(4, 4, 3, 255));
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, color_u8!(46, 46, 43, 255));
    draw_bevel(rect, color_u8!(112, 112, 106, 255), color_u8!(10, 10, 9, 255), false);
    draw_text_centered_at(
        label,
        rect.x + rect.w * 0.5,
        rect.y + (15.5 * scale),
        16.0,
        COLOR_TEXT,
    );
}

fn draw_controls() {
    let scale = hud_scale();
    let rect = controls_card_rect();
    let left = rect.x + (16.0 * scale);
    let right = rect.x + rect.w - (16.0 * scale);

    let rows = [
        ("L/R", "MOVE", 42.0),
        ("UP", "ROTATE", 28.0),
        ("SPACE", "DROP", 62.0),
        ("C", "HOLD", 28.0),
    ];

    for (index, (key, action, key_width)) in rows.iter().enumerate() {
        let y = rect.y + ((48.0 + index as f32 * 28.0) * scale);
        draw_keycap(key, left, y, *key_width);
        draw_text_right_at(action, right, y + (15.5 * scale), 16.0, COLOR_TEXT_MUTED);
    }
}

/// Draw the next-piece queue into the right side panel.
fn draw_piece_previews(piece_previews: [Piece; 3], textures: &SceneTextures, lights: SceneLights) {
    for (offset, piece) in piece_previews.iter().enumerate() {
        piece.draw(PiecePreviewArgs {
            center: next_piece_anchor(offset),
            scale: PREVIEW_SCALE,
            textures: textures.clone(),
            lights,
        });
    }
}

/// Draw the held piece into the left side panel, if one is held.
fn draw_held_piece(held_piece: Option<Piece>, textures: &SceneTextures, lights: SceneLights) {
    if let Some(piece) = held_piece {
        piece.draw(PiecePreviewArgs {
            center: hold_piece_anchor(),
            scale: PREVIEW_SCALE,
            textures: textures.clone(),
            lights,
        });
    }
}

fn draw_game_effects(game_state: &GameState<'_>, shake: Vec2) {
    let scale = hud_scale();
    let well = well_screen_rect();
    let impact = game_state.get_impact_effect();

    if impact > 0.0 {
        let elapsed = 1.0 - impact;
        let (impact_origins, impact_color) = game_state.get_impact_origins();
        let alpha = (impact * 1.8).min(1.0);

        for (contact_index, &(impact_col, support_row)) in impact_origins.iter().enumerate() {
            let support_top_y = if support_row < GRID_COUNT_ROWS as f32 {
                (WELL_HEIGHT * 0.5) - (support_row - FIRST_VISIBLE_ROW_ID as f32) - 0.5
                    + (BLOCK_INSET * 0.5)
            } else {
                -WELL_HEIGHT * 0.5
            };
            let world_origin = Vec3::new(
                impact_col - (WELL_WIDTH * 0.5),
                support_top_y,
                BLOCK_INSET * 0.5,
            );
            let projected_origin = world_to_screen_with_shake(world_origin, shake);
            let projected_edge =
                world_to_screen_with_shake(world_origin + Vec3::X * BLOCK_INSET * 0.5, shake);
            let block_width = ((projected_edge.x - projected_origin.x).abs() * 2.0).round();
            let origin = projected_origin.floor();

            // Each supporting block gets its own flash, constrained to that
            // block's top edge instead of expanding across unrelated cells.
            let burst_width = (4.0 + elapsed * (block_width - 4.0)).floor();
            draw_rectangle(
                (origin.x - burst_width * 0.5).floor(),
                origin.y,
                burst_width,
                2.0,
                Color::new(COLOR_AMBER.r, COLOR_AMBER.g, COLOR_AMBER.b, alpha),
            );
            draw_rectangle(
                (origin.x - burst_width * 0.25).floor(),
                origin.y - 2.0,
                (burst_width * 0.5).max(1.0),
                1.0,
                Color::new(COLOR_TEXT.r, COLOR_TEXT.g, COLOR_TEXT.b, alpha),
            );

            for spark_index in 0..14 {
                let seed = spark_index as f32
                    + contact_index as f32 * 47.0
                    + impact_col * 17.0
                    + support_row * 3.0;
                let edge_offset = (hash01(seed + 23.0) - 0.5) * block_width;
                let horizontal = (hash01(seed) - 0.5) * (52.0 + hash01(seed + 7.0) * 64.0);
                let vertical = -(18.0 + hash01(seed + 13.0) * 74.0);
                let gravity = 118.0 + hash01(seed + 19.0) * 52.0;
                let x = (origin.x + edge_offset + horizontal * elapsed).floor();
                let y = (origin.y + vertical * elapsed + gravity * elapsed * elapsed).floor();
                let size = if spark_index % 9 == 0 {
                    3.0
                } else if spark_index % 3 == 0 {
                    2.0
                } else {
                    1.0
                };
                let color = match spark_index % 4 {
                    0 => mix_color(impact_color, COLOR_TEXT, 0.3),
                    1 => COLOR_AMBER,
                    2 => color_u8!(105, 91, 65, 255),
                    _ => impact_color,
                };
                draw_rectangle(
                    x,
                    y,
                    size,
                    size,
                    Color::new(color.r, color.g, color.b, alpha),
                );
            }
        }
    }

    let pause_text = "ESC  PAUSE";
    draw_text_centered_at(
        pause_text,
        well.x + well.w * 0.5,
        frame_height() - (18.0 * scale),
        18.0,
        color_u8!(156, 136, 93, 210),
    );
}

pub trait Drawable {
    type Args;

    fn draw(&self, args: Self::Args);
}

impl<'a> Drawable for GameState<'a> {
    type Args = RenderSurface;

    /// Render a frame in two passes: the 3D scene with depth testing on, then
    /// the flat HUD on top of it.
    ///
    /// The order matters. `set_default_camera` disables depth testing, so the
    /// HUD can only be drawn after every 3D element has been submitted; drawing
    /// text first would leave it to be overwritten by the well.
    fn draw(&self, args: RenderSurface) {
        let time = get_time();
        let is_game_over = self.get_is_game_over();
        let lights = SceneLights::new(time, self.get_danger(), self.get_level_flare());
        draw_background(&args.textures, &lights);
        draw_game_chrome(&args.textures, &lights);

        let impact_shake = self.get_impact_effect().powi(2) * 0.08;
        let (clear_count, clear_remaining) = self.get_clear_effect();
        let clear_shake = if clear_remaining > 0.0 {
            clear_remaining.powi(2) * 0.05 * (clear_count as f32)
        } else {
            0.0
        };
        let shake_amount = (impact_shake + clear_shake).min(0.25);
        let shake = vec2(
            (time as f32 * 91.0).sin() * shake_amount,
            (time as f32 * 73.0).cos() * shake_amount,
        );
        set_camera(&args.camera_3d(shake));

        // Opaque scene first, effects that live behind the stack next, then
        // the stack, and finally everything translucent that sits in front.
        draw_well(&args.textures, &lights);
        draw_embers(time, &lights);
        if let Some((trail, strength)) = self.get_hard_drop_trail() {
            draw_hard_drop_trail(&trail, strength);
        }

        let wash = if is_game_over { GAME_OVER_WASH } else { 0.0 };
        self.get_grid_locked().draw(GridDrawArgs {
            style: BlockStyle::Solid,
            textures: args.textures.clone(),
            lights,
            wash,
        });
        self.get_grid_active().draw(GridDrawArgs {
            style: BlockStyle::Solid,
            textures: args.textures.clone(),
            lights,
            wash,
        });
        if !is_game_over {
            self.get_grid_ghost().draw(GridDrawArgs {
                style: BlockStyle::Ghost,
                textures: args.textures.clone(),
                lights,
                wash: 0.0,
            });
        }
        draw_piece_previews(self.get_piece_previews(), &args.textures, lights);
        draw_held_piece(self.get_held_piece(), &args.textures, lights);
        draw_shrapnel(self.get_shrapnel(), &args.textures);
        draw_clear_flash(self.get_clear_row_mask(), clear_remaining, clear_count);
        draw_lamp_glow(&lights);

        args.restore_2d();

        draw_score(self.get_score());
        draw_panel_labels();
        draw_level_and_rows_cleared(self.get_level(), self.get_rows_cleared());
        draw_controls();
        draw_game_effects(self, shake);
    }
}

pub struct GridDrawArgs {
    style: BlockStyle,
    textures: SceneTextures,
    lights: SceneLights,
    /// How far block colours are washed toward dead grey (0.0 = none). Used
    /// to drain the stack of life once the game is over.
    wash: f32,
}

impl Drawable for Grid {
    type Args = GridDrawArgs;

    fn draw(&self, args: GridDrawArgs) {
        for row_id in FIRST_VISIBLE_ROW_ID..GRID_COUNT_ROWS {
            for col_id in 0..GRID_COUNT_COLS {
                let Some(block) = self.get_cell(row_id, col_id) else {
                    continue;
                };

                let center = cell_center(row_id - FIRST_VISIBLE_ROW_ID, col_id);
                match args.style {
                    BlockStyle::Solid => block.draw(BlockArgs {
                        center,
                        textures: args.textures.clone(),
                        lights: args.lights,
                        wash: args.wash,
                    }),
                    BlockStyle::Ghost => {
                        // An edge is on the silhouette when no ghost cell lies
                        // across it; cells outside the grid count as empty.
                        let occupied = |row: isize, col: isize| {
                            row >= 0
                                && col >= 0
                                && self.has_block_at_cell(row as usize, col as usize)
                        };
                        let row = row_id as isize;
                        let col = col_id as isize;
                        let exterior = [
                            !occupied(row - 1, col),
                            !occupied(row, col + 1),
                            !occupied(row + 1, col),
                            !occupied(row, col - 1),
                        ];

                        draw_ghost_cell(center, block.color, exterior);
                    }
                }
            }
        }
    }
}

/// Placement of a piece drawn outside the well, in the hold slot or next queue.
///
/// Previews always show the piece in its spawn orientation, so no orientation is
/// carried here.
pub struct PiecePreviewArgs {
    /// World-space point that the piece's bounding box is centred on.
    center: Vec3,
    /// Edge length of one block cube, in world units.
    scale: f32,
    textures: SceneTextures,
    lights: SceneLights,
}

impl Drawable for Piece {
    type Args = PiecePreviewArgs;

    fn draw(&self, args: PiecePreviewArgs) {
        let PiecePreviewArgs {
            center,
            scale,
            textures,
            lights,
        } = args;

        let (blocks, _, _) = self.get_blocks(0);
        let (min_row, max_row, min_col, max_col) = self.get_trimmed_bounds(0);

        // Centre the trimmed shape on the anchor. `origin` is the centre of the
        // shape's top-left block, so the loop below only has to step outward.
        let span_cols = (max_col - min_col) as f32;
        let span_rows = (max_row - min_row) as f32;
        let origin_x = center.x - (span_cols * scale / 2.0) + (scale / 2.0);
        let origin_y = center.y + (span_rows * scale / 2.0) - (scale / 2.0);

        for row_id in min_row..max_row {
            for col_id in min_col..max_col {
                if let Some(block) = blocks[row_id][col_id] {
                    let position = Vec3::new(
                        origin_x + ((col_id - min_col) as f32 * scale),
                        origin_y - ((row_id - min_row) as f32 * scale),
                        center.z,
                    );

                    draw_block_cube_scaled(position, block.color, scale, &textures, &lights);
                }
            }
        }
    }
}

/// Placement and appearance of a single solid block inside the well.
pub struct BlockArgs {
    center: Vec3,
    textures: SceneTextures,
    lights: SceneLights,
    wash: f32,
}

/// Dead, unlit steel that game-over blocks fade toward.
const COLOR_DEAD_BLOCK: Color = color_u8!(92, 90, 86, 255);

impl Drawable for Block {
    type Args = BlockArgs;

    fn draw(&self, args: BlockArgs) {
        let BlockArgs {
            center,
            textures,
            lights,
            wash,
        } = args;
        let color = mix_color(self.color, COLOR_DEAD_BLOCK, wash);

        draw_block_cube(center, color, &textures, &lights);
    }
}

/// Edge length of the tumbling cube used as the menu cursor, in world units.
const MENU_CURSOR_SIZE: f32 = 0.55;

/// Depth of the menu cursor cube: ahead of every opaque element in the scene,
/// so it always passes the depth test left behind by the 3D pass.
const MENU_CURSOR_Z: f32 = 1.7;

/// Draw a slowly tumbling block cube at a framebuffer position, on top of the
/// 2D menu. Switches to the 3D camera and back, so callers must be in the 2D
/// pass when they call it.
fn draw_menu_cursor(surface: &RenderSurface, screen_center: Vec2) {
    let time = get_time() as f32;
    let center = screen_to_world_on_plane(screen_center, MENU_CURSOR_Z);
    let rotation = Vec3::new(time * 0.9, time * 1.4, 0.35);

    set_camera(&surface.camera_3d(Vec2::ZERO));
    draw_tumbling_cube(center, rotation, MENU_CURSOR_SIZE, COLOR_AMBER, &surface.textures);
    surface.restore_2d();
}

impl<'a> Drawable for Menu<'a> {
    type Args = RenderSurface;

    fn draw(&self, surface: RenderSurface) {
        if !self.is_visible {
            return;
        }

        let scale = hud_scale();
        let well = well_screen_rect();
        let lights = SceneLights::idle(get_time());
        let item_height = MENU_ITEM_HEIGHT * scale;
        let is_main = self.title.eq_ignore_ascii_case("bloxide");
        let title = self.title.to_ascii_uppercase();

        draw_rectangle(
            0.0,
            0.0,
            frame_width(),
            frame_height(),
            color_u8!(6, 5, 4, if is_main { 96 } else { 180 }),
        );

        let panel_width = well.w * MENU_WIDTH_SCALE;
        let panel_height =
            (MENU_PADDING_Y * 2.0 + 84.0) * scale + (item_height * self.items.len() as f32);
        let panel = snap_rect(Rect::new(
            well.x + ((well.w - panel_width) / 2.0),
            well.y + ((well.h - panel_height) / 2.0),
            panel_width,
            panel_height,
        ));
        let panel_x = panel.x;
        let panel_y = panel.y;
        let panel_width = panel.w;
        let panel_height = panel.h;
        let center_x = panel_x + (panel_width / 2.0);

        draw_instrument_panel(panel, &surface.textures, &lights);

        let title_baseline = panel_y + ((MENU_PADDING_Y + MENU_TITLE_TEXT_SIZE) * scale);
        let title_size = if is_main { MENU_TITLE_TEXT_SIZE } else { 34.0 };

        // Embossed: a deep shadow down-right and a one-pixel lit rim up-left,
        // so the title reads as letters cast into the plate.
        draw_text_centered_at(
            &title,
            center_x + (3.0 * scale),
            title_baseline + (3.0 * scale),
            title_size,
            color_u8!(3, 3, 2, 220),
        );
        draw_text_centered_at(
            &title,
            center_x - 1.0,
            title_baseline - 1.0,
            title_size,
            color_u8!(255, 250, 232, 255),
        );
        draw_text_centered_at(&title, center_x, title_baseline, title_size, COLOR_TEXT);
        let section_y = (title_baseline + (18.0 * scale)).round();
        draw_line(
            panel_x + (24.0 * scale),
            section_y,
            panel_x + panel_width - (24.0 * scale),
            section_y,
            1.0,
            COLOR_PANEL_BORDER,
        );

        let items_top = title_baseline + (32.0 * scale);
        let mut cursor_center = None;
        for (index, item) in self.items.iter().enumerate() {
            let row = snap_rect(Rect::new(
                panel_x + (24.0 * scale),
                items_top + (index as f32 * item_height),
                panel_width - (48.0 * scale),
                item_height - (7.0 * scale),
            ));
            let active = self.active_index == index;
            let baseline = row.y + (28.0 * scale);

            if active {
                let (_, text_height, _) = pixel_text_metrics(item.label, MENU_ITEM_TEXT_SIZE);
                cursor_center = Some(vec2(row.x + (8.0 * scale), baseline - text_height * 0.5));
            }

            draw_pixel_text(
                item.label,
                row.x + (30.0 * scale),
                baseline,
                MENU_ITEM_TEXT_SIZE,
                if active { COLOR_TEXT } else { COLOR_TEXT_MUTED },
            );
        }

        draw_text_centered_at(
            "UP/DOWN MOVE  ENTER SELECT",
            center_x,
            panel_y + panel_height - (16.0 * scale),
            18.0,
            COLOR_TEXT_MUTED,
        );

        if let Some(center) = cursor_center {
            draw_menu_cursor(&surface, center);
        }
    }
}

impl Drawable for HighScoreManager {
    type Args = RenderSurface;

    fn draw(&self, surface: RenderSurface) {
        let label = "PERSONAL BEST";
        let score = self.get_high_score().to_formatted_string(&Locale::en);
        let lights = SceneLights::idle(get_time());
        let rect = snap_rect(Rect::new(frame_width() - 112.0, 8.0, 102.0, 34.0));
        draw_instrument_panel(rect, &surface.textures, &lights);
        draw_engraved_text(label, rect.x + 8.0, rect.y + 14.0, 16.0, COLOR_TEXT_MUTED);
        draw_text_right_at(
            &score,
            rect.x + rect.w - 8.0,
            rect.y + 28.0,
            18.0,
            COLOR_AMBER,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piece::pieces;

    fn preview_vertical_bounds(piece: Piece, center: Vec3) -> (f32, f32) {
        let (blocks, _, _) = piece.get_blocks(0);
        let (min_row, max_row, min_col, max_col) = piece.get_trimmed_bounds(0);
        let span_cols = (max_col - min_col) as f32;
        let span_rows = (max_row - min_row) as f32;
        let origin_x = center.x - (span_cols * PREVIEW_SCALE / 2.0) + (PREVIEW_SCALE / 2.0);
        let origin_y = center.y + (span_rows * PREVIEW_SCALE / 2.0) - (PREVIEW_SCALE / 2.0);
        let half_cube = BLOCK_INSET * PREVIEW_SCALE * 0.5;
        let mut top = f32::INFINITY;
        let mut bottom = f32::NEG_INFINITY;

        for row_id in min_row..max_row {
            for col_id in min_col..max_col {
                if blocks[row_id][col_id].is_none() {
                    continue;
                }

                let position = Vec3::new(
                    origin_x + ((col_id - min_col) as f32 * PREVIEW_SCALE),
                    origin_y - ((row_id - min_row) as f32 * PREVIEW_SCALE),
                    center.z,
                );

                for y_sign in [-1.0, 1.0] {
                    for z_sign in [-1.0, 1.0] {
                        let corner = position + Vec3::new(0.0, y_sign, z_sign) * half_cube;
                        let screen_y = world_to_screen(corner).y;
                        top = top.min(screen_y);
                        bottom = bottom.max(screen_y);
                    }
                }
            }
        }

        (top, bottom)
    }

    #[test]
    fn hud_panels_share_a_snapped_symmetric_grid() {
        let layout = hud_layout();
        let well = well_screen_rect();

        assert_eq!(layout.hold.x, layout.controls.x);
        assert_eq!(layout.next.x, layout.stats.x);
        assert_eq!(layout.hold.y, layout.next.y);
        assert_eq!(layout.controls.y, layout.stats.y);
        assert_eq!(layout.controls.h, layout.stats.h);

        let left_gap = well.x - (layout.hold.x + layout.hold.w);
        let right_gap = layout.next.x - (well.x + well.w);
        assert!((left_gap - right_gap).abs() <= 1.0);

        for rect in [
            layout.hold,
            layout.next,
            layout.controls,
            layout.stats,
            layout.score,
        ] {
            for value in [rect.x, rect.y, rect.w, rect.h] {
                assert_eq!(value.fract(), 0.0);
            }
        }
    }

    #[test]
    fn hold_preview_is_vertically_centered_below_its_header() {
        let (content_top, content_bottom) = preview_content_vertical_bounds(hold_card_rect());
        let anchor_y = world_to_screen(hold_piece_anchor()).y;
        let expected_center = (content_top + content_bottom) * 0.5;

        assert!((anchor_y - expected_center).abs() <= 0.5);
    }

    #[test]
    fn every_next_piece_fits_inside_each_padded_queue_slot() {
        let pieces = [
            pieces::I,
            pieces::J,
            pieces::L,
            pieces::O,
            pieces::S,
            pieces::T,
            pieces::Z,
        ];
        let (content_top, content_bottom) = preview_content_vertical_bounds(next_card_rect());
        let slot_height = (content_bottom - content_top) / 3.0;
        let minimum_slot_padding = 2.0;
        let anchors = [
            next_piece_anchor(0),
            next_piece_anchor(1),
            next_piece_anchor(2),
        ];
        let first_gap = world_to_screen(anchors[1]).y - world_to_screen(anchors[0]).y;
        let second_gap = world_to_screen(anchors[2]).y - world_to_screen(anchors[1]).y;

        assert!((first_gap - second_gap).abs() <= 1.0);

        for (slot_index, anchor) in anchors.into_iter().enumerate() {
            let slot_top = content_top + slot_height * slot_index as f32;
            let slot_bottom = slot_top + slot_height;
            for piece in pieces {
                let (top, bottom) = preview_vertical_bounds(piece, anchor);
                assert!(
                    top >= slot_top + minimum_slot_padding,
                    "{} exceeds the padded slot top",
                    piece.name
                );
                assert!(
                    bottom <= slot_bottom - minimum_slot_padding,
                    "{} exceeds the padded slot bottom",
                    piece.name
                );
            }
        }
    }
}
