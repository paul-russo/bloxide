use crate::block::Block;
use crate::effects::{draw_clear_flash, draw_embers, draw_hard_drop_trail};
use crate::game_state::GameState;
use crate::grid::{Grid, FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, GRID_COUNT_ROWS};
use crate::high_score_manager::HighScoreManager;
use crate::lighting::{lit, SceneLights};
use crate::menu::Menu;
use crate::piece::Piece;
use crate::pixel_font::{
    digit_text_glyphs, digit_text_width, small_text_glyphs, small_text_width, DIGIT_HEIGHT,
    SMALL_GLYPH_HEIGHT,
};
use crate::postfx::PostProcess;
use crate::render3d::{
    cell_center, draw_block_cube, draw_block_cube_scaled, draw_ghost_cell, draw_lamp_glow,
    draw_quad_corners, draw_quad_corners_uv, draw_shrapnel, draw_tumbling_cube, draw_well,
    lintel_front_bounds, set_depth_test, well_camera, world_to_screen, world_to_screen_with_shake,
    ScreenPlane, BLOCK_INSET, RENDER_HEIGHT, RENDER_WIDTH, WELL_DEPTH, WELL_HEIGHT, WELL_WIDTH,
};
use crate::scoring::{ScoreAward, SpinKind};
use crate::textures::{Material, SceneTextures, BLOCK_TEXTURE_SIZE, STONE_TEXTURE_SIZE};
use macroquad::prelude::*;
use macroquad::texture::{render_target_ex, RenderTargetParams};
use num_format::{Locale, ToFormattedString};

/// The window is sized for the 3D scene rather than for a pixel-exact 2D grid:
/// it has to fit the well plus a side panel on each side, with the well framed
/// by [`crate::render3d::well_camera`].
pub const WINDOW_WIDTH: f32 = 1200.0;
pub const WINDOW_HEIGHT: f32 = 900.0;

/// Cleared before anything is drawn. The stone backdrop covers the whole frame,
/// so this only shows through if a tile ever fails to.
pub const BACKGROUND_COLOR: Color = color_u8!(10, 9, 7, 255);

pub struct RenderSurface {
    pub target: RenderTarget,
    textures: SceneTextures,
    post_process: PostProcess,
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
            post_process: PostProcess::new(RENDER_WIDTH, RENDER_HEIGHT),
        }
    }

    fn camera(&self, shake: Vec2) -> Camera3D {
        well_camera(Some(self.target.clone()), shake)
    }

    /// Start a frame: aim the camera at the render target, shaken by `shake`,
    /// clear it, and hand back the frame's drawing context.
    ///
    /// The whole frame is drawn through this one camera. Screen-fixed elements
    /// are placed on planes in front of it (see [`ScreenPlane`]), so a frame
    /// needs no camera switches and macroquad can submit it as a few batches.
    pub fn begin_frame(&self, time: f64, shake: Vec2) -> Frame<'_> {
        set_camera(&self.camera(shake));
        clear_background(BACKGROUND_COLOR);

        Frame {
            textures: &self.textures,
            time,
            shake,
        }
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

/// The drawing context for one frame: the materials, the moment the frame
/// depicts, and the camera shake it is drawn with.
///
/// A frame is one camera pass in three layers, drawn in this order: the
/// screen-fixed elements behind the scene ([`Frame::backdrop`]), the
/// depth-tested 3D scene ([`Frame::begin_scene`]), and the flat HUD on top
/// ([`Frame::hud`]). The outer two layers are painted in submission order with
/// depth testing off; only the scene between them uses the depth buffer.
pub struct Frame<'a> {
    textures: &'a SceneTextures,
    time: f64,
    shake: Vec2,
}

/// Depth of the stone wall behind the cabinet, used to place it in the scene's
/// lighting. It sits a little behind the well's back wall.
const BACKDROP_WALL_Z: f32 = -1.0;

/// Depth of the HUD plane. The HUD layer is not depth tested, so this only has
/// to lie inside the camera's clip range; ahead of the cabinet front keeps the
/// plane's pixel mapping clear of any scene geometry.
const HUD_Z: f32 = 1.5;

impl<'a> Frame<'a> {
    fn surface(&self, z: f32) -> Surface<'a> {
        Surface {
            plane: ScreenPlane::new(z, self.shake),
            textures: self.textures,
        }
    }

    /// Surface for the screen-fixed elements behind the scene: the stone wall
    /// and the HUD housings mounted on it. Painted without depth testing, so
    /// it must be drawn before the scene, which then covers it wherever there
    /// is geometry.
    pub fn backdrop(&self) -> Surface<'a> {
        set_depth_test(false);
        self.surface(BACKDROP_WALL_Z)
    }

    /// Switch to depth-tested drawing for the 3D scene. Call once the
    /// backdrop is complete and before any scene geometry.
    pub fn begin_scene(&self) {
        set_depth_test(true);
    }

    /// Surface for the flat HUD on top of the scene. Switches depth testing
    /// off for the rest of the frame, so everything drawn afterwards is
    /// layered in submission order over the scene.
    pub fn hud(&self) -> Surface<'a> {
        set_depth_test(false);
        self.surface(HUD_Z)
    }

    /// World position that appears at framebuffer pixel `screen` on the plane
    /// at depth `z`, for 3D geometry anchored to a screen-space layout.
    pub fn point_at(&self, screen: Vec2, z: f32) -> Vec3 {
        ScreenPlane::new(z, self.shake).world(screen)
    }
}

/// A screen-space drawing surface: a plane facing the camera, addressed in
/// framebuffer pixels, with the materials to draw from. Everything the HUD
/// draws goes through one of these, so it joins the scene's batch.
pub struct Surface<'a> {
    plane: ScreenPlane,
    textures: &'a SceneTextures,
}

impl Surface<'_> {
    /// The point of the resting scene under a pixel of this surface, for
    /// sampling the scene lights.
    fn resting_world(&self, screen: Vec2) -> Vec3 {
        self.plane.resting_world(screen)
    }

    /// Fill a pixel rectangle with a flat colour.
    fn fill(&self, rect: Rect, color: Color) {
        draw_quad_corners(self.plane.corners(rect), self.textures.white(), [color; 4]);
    }

    /// One-pixel horizontal rule along row `y`, from `x1` to `x2`.
    fn hline(&self, x1: f32, x2: f32, y: f32, color: Color) {
        self.fill(Rect::new(x1, y, x2 - x1, 1.0), color);
    }

    /// One-pixel vertical rule down column `x`, from `y1` to `y2`.
    fn vline(&self, x: f32, y1: f32, y2: f32, color: Color) {
        self.fill(Rect::new(x, y1, 1.0, y2 - y1), color);
    }

    /// A material tile over `rect`, showing the material from its top-left
    /// texel to `uv_max`, with a colour at each corner.
    fn tile(&self, rect: Rect, material: Material, uv_max: Vec2, colors: [Color; 4]) {
        draw_quad_corners_uv(
            self.plane.corners(rect),
            Vec2::ZERO,
            uv_max,
            material,
            colors,
        );
    }

    /// Draw `text` in the 5x7 face with its left edge at `x` and its baseline
    /// at `baseline_y`. Characters without a glyph leave their cell empty.
    fn small_text(&self, text: &str, x: f32, baseline_y: f32, pixel: f32, color: Color) {
        for (character, cell) in small_text_glyphs(text, x, baseline_y, pixel) {
            if let Some(glyph) = self.textures.small_glyph(character) {
                draw_quad_corners(self.plane.corners(cell), glyph, [color; 4]);
            }
        }
    }

    /// Draw `text` in bold numerals with its left edge at `x` and its top at
    /// `top_y`. Characters without a glyph leave their cell empty.
    fn digit_text(&self, text: &str, x: f32, top_y: f32, pixel: f32, color: Color) {
        for (character, cell) in digit_text_glyphs(text, x, top_y, pixel) {
            if let Some(glyph) = self.textures.digit_glyph(character) {
                draw_quad_corners(self.plane.corners(cell), glyph, [color; 4]);
            }
        }
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
const HUD_AWARD_GAP: f32 = 16.0;
const AWARD_TEXT_SIZE: f32 = 16.0;
const AWARD_FIRST_BASELINE: f32 = 58.0;
const AWARD_LINE_PITCH: f32 = 20.0;

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

fn draw_pixel_text(hud: &Surface, text: &str, x: f32, baseline_y: f32, size: f32, color: Color) {
    hud.small_text(text, x, baseline_y, text_pixel(size), color);
}

/// Labels cut into a metal plate: a dark offset copy under the lit glyphs.
fn draw_engraved_text(
    hud: &Surface,
    text: &str,
    x: f32,
    baseline_y: f32,
    size: f32,
    color: Color,
) {
    draw_pixel_text(hud, text, x + 1.0, baseline_y + 1.0, size, COLOR_ENGRAVE_SHADOW);
    draw_pixel_text(hud, text, x, baseline_y, size, color);
}

/// Fill a screen-space rectangle with a tiled material, one texel per pixel,
/// each tile lit at its corners by the scene lights as if the rectangle were a
/// plate mounted on the wall at `PANEL_Z`. The last column and row of tiles
/// are clipped through their UVs rather than stretched.
fn draw_tiled_rect(
    surface: &Surface,
    rect: Rect,
    material: Material,
    tile: f32,
    tint: Color,
    lights: &SceneLights,
) {
    let columns = (rect.w / tile).ceil() as usize;
    let rows = (rect.h / tile).ceil() as usize;
    let lighting = ScreenPlane::new(PANEL_Z, Vec2::ZERO);

    for row in 0..rows {
        for column in 0..columns {
            let x = rect.x + column as f32 * tile;
            let y = rect.y + row as f32 * tile;
            let width = tile.min(rect.x + rect.w - x);
            let height = tile.min(rect.y + rect.h - y);
            let corners = [
                Vec2::new(x, y),
                Vec2::new(x + width, y),
                Vec2::new(x + width, y + height),
                Vec2::new(x, y + height),
            ];
            let colors = corners.map(|corner| lit(tint, lights.at(lighting.resting_world(corner))));

            surface.tile(
                Rect::new(x, y, width, height),
                material,
                Vec2::new(width / tile, height / tile),
                colors,
            );
        }
    }
}

/// One-pixel chamfer around `rect`: lit along the top and left, shadowed along
/// the bottom and right. `recessed` swaps them for an inset opening.
fn draw_bevel(surface: &Surface, rect: Rect, light: Color, dark: Color, recessed: bool) {
    let (top_left, bottom_right) = if recessed {
        (dark, light)
    } else {
        (light, dark)
    };

    surface.fill(Rect::new(rect.x, rect.y, rect.w, 1.0), top_left);
    surface.fill(Rect::new(rect.x, rect.y, 1.0, rect.h), top_left);
    surface.fill(Rect::new(rect.x, rect.y + rect.h - 1.0, rect.w, 1.0), bottom_right);
    surface.fill(Rect::new(rect.x + rect.w - 1.0, rect.y, 1.0, rect.h), bottom_right);
}

/// A 3x3 slotted screw head.
fn draw_screw(surface: &Surface, center_x: f32, center_y: f32) {
    let x = center_x.round() - 1.0;
    let y = center_y.round() - 1.0;
    surface.fill(Rect::new(x, y, 3.0, 3.0), shaded(COLOR_SCREW, 0.55));
    surface.fill(Rect::new(x, y, 2.0, 2.0), shaded(COLOR_SCREW, 1.05));
    surface.fill(Rect::new(x, y, 1.0, 1.0), shaded(COLOR_SCREW, 1.4));
    surface.fill(Rect::new(x, y + 1.0, 3.0, 1.0), shaded(COLOR_SCREW, 0.35));
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
fn draw_instrument_panel(surface: &Surface, rect: Rect, lights: &SceneLights) {
    let scale = hud_scale();
    let shadow = (6.0 * scale).round().max(1.0);
    let inner = Rect::new(
        rect.x + PANEL_FRAME,
        rect.y + PANEL_FRAME,
        rect.w - PANEL_FRAME * 2.0,
        rect.h - PANEL_FRAME * 2.0,
    );

    surface.fill(
        Rect::new(rect.x + shadow, rect.y + shadow, rect.w, rect.h),
        color_u8!(3, 3, 2, 200),
    );
    draw_tiled_rect(
        surface,
        rect,
        surface.textures.gunmetal(),
        BLOCK_TEXTURE_SIZE as f32,
        COLOR_PANEL_FRAME,
        lights,
    );
    draw_bevel(surface, rect, COLOR_PANEL_LIGHT, COLOR_PANEL_DARK, false);

    surface.fill(inner, COLOR_PANEL);
    draw_bevel(surface, inner, COLOR_PANEL_INNER_LIGHT, COLOR_PANEL_DARK, true);

    for (x, y) in [
        (rect.x + 3.0, rect.y + 3.0),
        (rect.x + rect.w - 3.0, rect.y + 3.0),
        (rect.x + 3.0, rect.y + rect.h - 3.0),
        (rect.x + rect.w - 3.0, rect.y + rect.h - 3.0),
    ] {
        draw_screw(surface, x, y);
    }
}

fn hash01(seed: f32) -> f32 {
    ((seed * 12.9898).sin() * 43_758.547).fract().abs()
}

/// Base tint of the backdrop masonry before lighting. Under ambient alone the
/// wall is barely there; it is the lamps and the furnace that reveal it.
const COLOR_STONE: Color = color_u8!(70, 68, 64, 255);

/// Cut-stone wall behind the cabinet, lit by the same lamps and furnace as the
/// 3D scene. Each tile's corners sample the scene lights at the world position
/// beneath that pixel, so the pools on the stone line up with the fixtures that
/// cast them. Drawn on the backdrop surface, whose plane is the wall itself.
pub fn draw_background(backdrop: &Surface, lights: &SceneLights) {
    let tile = STONE_TEXTURE_SIZE as f32;
    let columns = (frame_width() / tile).ceil() as usize;
    let rows = (frame_height() / tile).ceil() as usize;

    for row in 0..rows {
        for column in 0..columns {
            let top_left = Vec2::new(column as f32 * tile, row as f32 * tile);
            let corners = [
                top_left,
                top_left + Vec2::X * tile,
                top_left + Vec2::new(tile, tile),
                top_left + Vec2::Y * tile,
            ];
            let colors =
                corners.map(|corner| lit(COLOR_STONE, lights.at(backdrop.resting_world(corner))));

            backdrop.tile(
                Rect::new(top_left.x, top_left.y, tile, tile),
                backdrop.textures.stone(),
                Vec2::ONE,
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
fn draw_text_centered_at(
    hud: &Surface,
    text: &str,
    center_x: f32,
    baseline_y: f32,
    size: f32,
    color: Color,
) {
    let (width, _, _) = pixel_text_metrics(text, size);
    draw_pixel_text(hud, text, center_x - (width * 0.5), baseline_y, size, color);
}

fn draw_text_right_at(
    hud: &Surface,
    text: &str,
    right_x: f32,
    baseline_y: f32,
    size: f32,
    color: Color,
) {
    let (width, _, _) = pixel_text_metrics(text, size);
    draw_pixel_text(hud, text, right_x - width, baseline_y, size, color);
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

/// Framebuffer rectangles of the HUD panels. Computed once per frame, since
/// every panel projects the well through the camera to find its place.
#[derive(Copy, Clone)]
struct HudLayout {
    hold: Rect,
    award: Rect,
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
    let award_top = (top_y + (HUD_HOLD_HEIGHT + HUD_AWARD_GAP) * scale).round();
    let award_bottom = (lower_y - HUD_AWARD_GAP * scale).round();

    HudLayout {
        hold: Rect::new(
            left_x,
            top_y,
            panel_width,
            (HUD_HOLD_HEIGHT * scale).round(),
        ),
        award: Rect::new(left_x, award_top, panel_width, award_bottom - award_top),
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

fn hold_piece_anchor(layout: &HudLayout) -> Vec3 {
    let (content_top, content_bottom) = preview_content_vertical_bounds(layout.hold);
    preview_anchor_at_screen_y(
        HOLD_PREVIEW_REFERENCE,
        ((content_top + content_bottom) * 0.5).round(),
    )
}

fn next_piece_anchor(layout: &HudLayout, index: usize) -> Vec3 {
    debug_assert!(index < 3);
    let (content_top, content_bottom) = preview_content_vertical_bounds(layout.next);
    let slot_height = (content_bottom - content_top) / 3.0;
    let target_y = content_top + slot_height * (index as f32 + 0.5);
    preview_anchor_at_screen_y(NEXT_PREVIEW_REFERENCE, target_y.round())
}

/// Nameplate strip across the top of a panel face, with the label engraved
/// into it and a rule underneath.
fn draw_panel_header(hud: &Surface, rect: Rect, label: &str) {
    let scale = hud_scale();
    let inset = PANEL_FRAME + 1.0;
    let left = rect.x + (16.0 * scale);
    let right = rect.x + rect.w - (16.0 * scale);
    let baseline = rect.y + (24.0 * scale);
    let divider_y = rect.y + (HUD_HEADER_DIVIDER_OFFSET * scale);

    hud.fill(
        Rect::new(
            rect.x + inset,
            rect.y + inset,
            rect.w - inset * 2.0,
            divider_y - rect.y - inset,
        ),
        COLOR_PANEL_HEADER,
    );
    draw_engraved_text(hud, label, left, baseline, LABEL_TEXT_SIZE, COLOR_TEXT_MUTED);
    hud.hline(left, right, divider_y, COLOR_PANEL_BORDER);
}

fn draw_next_slot_guides(hud: &Surface, layout: &HudLayout) {
    let rect = layout.next;
    let (content_top, content_bottom) = preview_content_vertical_bounds(rect);
    let slot_height = (content_bottom - content_top) / 3.0;
    let inset = 16.0 * hud_scale();

    for slot in 1..3 {
        let y = (content_top + slot_height * slot as f32).round();
        hud.hline(
            rect.x + inset,
            rect.x + rect.w - inset,
            y,
            color_u8!(69, 65, 54, 120),
        );
    }
}

/// The instrument housings, mounted on the wall behind the cabinet.
fn draw_game_chrome(backdrop: &Surface, layout: &HudLayout, lights: &SceneLights) {
    draw_instrument_panel(backdrop, layout.hold, lights);
    draw_instrument_panel(backdrop, layout.next, lights);
    draw_instrument_panel(backdrop, layout.stats, lights);
    draw_instrument_panel(backdrop, layout.controls, lights);
}

/// Render the empty well on its own, as a backdrop for the main menu screen.
///
/// Leaves the frame in its scene layer, so the caller starts the HUD layer
/// for the menu on top.
pub fn draw_backdrop(frame: &Frame) {
    let lights = SceneLights::idle(frame.time);
    let backdrop = frame.backdrop();
    draw_background(&backdrop, &lights);

    frame.begin_scene();
    draw_well(frame.textures, &lights, frame.time, &[]);
    draw_embers(frame.time, &lights, frame.textures);
    draw_lamp_glow(&lights, frame.textures);
}

/// Glyph pixel size of the bold readout numerals.
const READOUT_PIXEL: f32 = 2.0;

/// Padding between a readout window's glass and its digits, in pixels.
const READOUT_PADDING: f32 = 4.0;

/// A dark glass window let into a metal face, with bold numerals glowing
/// inside it. `text` is right-aligned so growing numbers extend left, the way
/// a mechanical counter reads; `ghost` is the counter's full width of unlit
/// positions, shown faintly behind the live digits.
fn draw_readout_window(hud: &Surface, rect: Rect, text: &str, ghost: &str, color: Color) {
    hud.fill(rect, color_u8!(6, 6, 5, 255));
    draw_bevel(hud, rect, COLOR_PANEL_INNER_LIGHT, color_u8!(1, 1, 1, 255), true);

    let top = rect.y + ((rect.h - DIGIT_HEIGHT as f32 * READOUT_PIXEL) * 0.5).round();
    let right = rect.x + rect.w - READOUT_PADDING;
    let ghost_x = right - digit_text_width(ghost, READOUT_PIXEL);
    let x = right - digit_text_width(text, READOUT_PIXEL);

    hud.digit_text(ghost, ghost_x, top, READOUT_PIXEL, shaded(color, 0.07));

    // A dim copy under the lit digits gives them a faint phosphor bloom.
    hud.digit_text(text, x + 1.0, top + 1.0, READOUT_PIXEL, shaded(color, 0.25));
    hud.digit_text(text, x, top, READOUT_PIXEL, color);
}

/// Height of a readout window tall enough for the bold numerals.
fn readout_height() -> f32 {
    DIGIT_HEIGHT as f32 * READOUT_PIXEL + READOUT_PADDING * 2.0
}

/// Draw the score readout mounted on the lintel above the well: an engraved
/// label beside a glass window, centred together on the beam.
fn draw_score(hud: &Surface, layout: &HudLayout, score: usize) {
    let text = score.to_formatted_string(&Locale::en);
    let rect = layout.score;
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
        hud,
        "SCORE",
        group_x,
        (center_y + label_height * 0.5).round(),
        18.0,
        COLOR_TEXT_MUTED,
    );
    draw_readout_window(
        hud,
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
fn draw_panel_labels(hud: &Surface, layout: &HudLayout) {
    draw_panel_header(hud, layout.hold, "HOLD");
    draw_panel_header(hud, layout.next, "NEXT");
    draw_panel_header(hud, layout.controls, "CONTROLS");
    draw_panel_header(hud, layout.stats, "STATUS");
    draw_next_slot_guides(hud, layout);
}

/// One indicator lamp in the level-progress bar.
fn draw_indicator_pip(hud: &Surface, x: f32, y: f32, size: f32, lit: bool) {
    hud.fill(Rect::new(x, y, size, size), color_u8!(8, 8, 7, 255));
    if lit {
        hud.fill(
            Rect::new(x + 1.0, y + 1.0, size - 2.0, size - 2.0),
            COLOR_AMBER,
        );
        hud.fill(Rect::new(x + 1.0, y + 1.0, 1.0, 1.0), COLOR_TEXT);
    } else {
        hud.fill(
            Rect::new(x + 1.0, y + 1.0, size - 2.0, size - 2.0),
            color_u8!(44, 42, 38, 255),
        );
    }
}

/// Draw the level and cleared-line counts beneath the next queue, as two bold
/// readouts over a row of ten indicator lamps counting lines toward the next
/// level.
fn draw_level_and_rows_cleared(hud: &Surface, layout: &HudLayout, level: usize, rows_cleared: usize) {
    let scale = hud_scale();
    let rect = layout.stats;
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
            hud,
            label,
            (center_x - label_width * 0.5).round(),
            label_baseline,
            STATS_TEXT_SIZE,
            COLOR_TEXT_MUTED,
        );
        draw_readout_window(
            hud,
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

    hud.vline(
        mid_x,
        rect.y + (48.0 * scale),
        window_top + readout_height(),
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
            hud,
            bar_x + segment as f32 * (pip + gap),
            bar_y,
            pip,
            segment < completed,
        );
    }
}

/// A raised keycap: lit along its top and left, shadowed bottom and right.
fn draw_keycap(hud: &Surface, label: &str, x: f32, y: f32, width: f32) {
    let scale = hud_scale();
    let rect = Rect::new(x, y, (width * scale).round(), (22.0 * scale).round());
    hud.fill(
        Rect::new(rect.x + 1.0, rect.y + 1.0, rect.w, rect.h),
        color_u8!(4, 4, 3, 255),
    );
    hud.fill(rect, color_u8!(46, 46, 43, 255));
    draw_bevel(
        hud,
        rect,
        color_u8!(112, 112, 106, 255),
        color_u8!(10, 10, 9, 255),
        false,
    );
    draw_text_centered_at(
        hud,
        label,
        rect.x + rect.w * 0.5,
        rect.y + (15.5 * scale),
        16.0,
        COLOR_TEXT,
    );
}

fn draw_controls(hud: &Surface, layout: &HudLayout) {
    let scale = hud_scale();
    let rect = layout.controls;
    let left = rect.x + (16.0 * scale);
    let right = rect.x + rect.w - (16.0 * scale);

    let rows = [
        ("L/R", "MOVE", 42.0),
        ("Z/X", "ROTATE", 42.0),
        ("SPACE", "DROP", 62.0),
        ("C", "HOLD", 28.0),
    ];

    for (index, (key, action, key_width)) in rows.iter().enumerate() {
        let y = rect.y + ((48.0 + index as f32 * 28.0) * scale);
        draw_keycap(hud, key, left, y, *key_width);
        draw_text_right_at(hud, action, right, y + (15.5 * scale), 16.0, COLOR_TEXT_MUTED);
    }
}

struct ScoreAnnouncementText {
    headline: &'static str,
    clear: &'static str,
    points: String,
    back_to_back: &'static str,
    combo: String,
}

fn score_announcement_text(award: ScoreAward) -> ScoreAnnouncementText {
    let headline = match award.event.spin {
        SpinKind::None => "LINE CLEAR",
        SpinKind::Mini => "T-SPIN MINI",
        SpinKind::Full => "T-SPIN",
    };
    let clear = match award.event.lines {
        0 => "NO LINES",
        1 => "SINGLE",
        2 => "DOUBLE",
        3 => "TRIPLE",
        4 => "TETRIS",
        _ => "CLEAR",
    };

    // Keep even unusually long endless runs inside the small instrument panel.
    // The full score is still retained by gameplay; only this transient label
    // abbreviates billion-plus awards and very large combo counts.
    let total = award.total() as u64;
    let points = if total < 1_000_000_000 {
        format!("+{}", total.to_formatted_string(&Locale::en))
    } else if total < 1_000_000_000_000 {
        format!("+{:.1}B", total as f64 / 1_000_000_000.0)
    } else {
        format!("+{:.1}T", total as f64 / 1_000_000_000_000.0)
    };
    let combo = match award.combo {
        Some(count) if count > 99_999 => String::from("COMBO 99999+"),
        Some(count) if count > 0 => format!("COMBO {count}"),
        _ => String::new(),
    };

    ScoreAnnouncementText {
        headline,
        clear,
        points,
        back_to_back: if award.back_to_back_bonus > 0 { "B2B" } else { "" },
        combo,
    }
}

fn draw_score_announcement(hud: &Surface, layout: &HudLayout, award: ScoreAward) {
    let text = score_announcement_text(award);
    let rect = layout.award;
    draw_panel_header(hud, rect, text.headline);

    for (index, (line, color)) in [
        (text.clear, COLOR_TEXT),
        (text.points.as_str(), COLOR_AMBER),
        (text.back_to_back, COLOR_AMBER),
        (text.combo.as_str(), COLOR_TEXT_MUTED),
    ]
    .into_iter()
    .enumerate()
    {
        if line.is_empty() {
            continue;
        }

        draw_text_centered_at(
            hud,
            line,
            rect.x + rect.w * 0.5,
            rect.y + (AWARD_FIRST_BASELINE + index as f32 * AWARD_LINE_PITCH) * hud_scale(),
            AWARD_TEXT_SIZE,
            color,
        );
    }
}

/// Draw the next-piece queue into the right side panel.
fn draw_piece_previews(
    layout: &HudLayout,
    piece_previews: [Piece; 3],
    textures: &SceneTextures,
    lights: SceneLights,
) {
    for (offset, piece) in piece_previews.iter().enumerate() {
        piece.draw(PiecePreviewArgs {
            center: next_piece_anchor(layout, offset),
            scale: PREVIEW_SCALE,
            textures,
            lights,
        });
    }
}

/// Draw the held piece into the left side panel, if one is held.
fn draw_held_piece(
    layout: &HudLayout,
    held_piece: Option<Piece>,
    textures: &SceneTextures,
    lights: SceneLights,
) {
    if let Some(piece) = held_piece {
        piece.draw(PiecePreviewArgs {
            center: hold_piece_anchor(layout),
            scale: PREVIEW_SCALE,
            textures,
            lights,
        });
    }
}

fn draw_game_effects(hud: &Surface, layout: &HudLayout, game_state: &GameState<'_>, shake: Vec2) {
    let scale = hud_scale();
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
            hud.fill(
                Rect::new(
                    (origin.x - burst_width * 0.5).floor(),
                    origin.y,
                    burst_width,
                    2.0,
                ),
                Color::new(COLOR_AMBER.r, COLOR_AMBER.g, COLOR_AMBER.b, alpha),
            );
            hud.fill(
                Rect::new(
                    (origin.x - burst_width * 0.25).floor(),
                    origin.y - 2.0,
                    (burst_width * 0.5).max(1.0),
                    1.0,
                ),
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
                hud.fill(
                    Rect::new(x, y, size, size),
                    Color::new(color.r, color.g, color.b, alpha),
                );
            }
        }
    }

    // The pause hint sits under the controls panel, clear of the pit below
    // the well.
    let controls = layout.controls;
    draw_text_centered_at(
        hud,
        "ESC  PAUSE",
        controls.x + controls.w * 0.5,
        controls.y + controls.h + (22.0 * scale),
        18.0,
        color_u8!(156, 136, 93, 210),
    );
}

/// Camera shake for the frame: an impact pulse when a piece locks, and a
/// heavier rumble while rows are clearing.
pub fn camera_shake(game_state: &GameState<'_>, time: f64) -> Vec2 {
    let impact_shake = game_state.get_impact_effect().powi(2) * 0.08;
    let (clear_count, clear_remaining) = game_state.get_clear_effect();
    let clear_shake = if clear_remaining > 0.0 {
        clear_remaining.powi(2) * 0.05 * (clear_count as f32)
    } else {
        0.0
    };
    let shake_amount = (impact_shake + clear_shake).min(0.25);

    vec2(
        (time as f32 * 91.0).sin() * shake_amount,
        (time as f32 * 73.0).cos() * shake_amount,
    )
}

pub trait Drawable<Args> {
    fn draw(&self, args: Args);
}

impl Drawable<&Frame<'_>> for GameState<'_> {
    /// Render a frame in its three layers: the wall and instrument housings
    /// behind everything, the depth-tested 3D scene, and the flat HUD on top.
    ///
    /// The order matters. The outer layers are painted without depth testing,
    /// so the scene can only cover the backdrop if the backdrop comes first,
    /// and the HUD can only sit over the scene if it comes last.
    fn draw(&self, frame: &Frame<'_>) {
        let time = frame.time;
        let textures = frame.textures;
        let is_game_over = self.get_is_game_over();
        let lights = SceneLights::new(time, self.get_danger(), self.get_level_flare());
        let layout = hud_layout();
        let announcement = self.get_score_announcement();

        let backdrop = frame.backdrop();
        draw_background(&backdrop, &lights);
        draw_game_chrome(&backdrop, &layout, &lights);

        if announcement.is_some() {
            draw_instrument_panel(&backdrop, layout.award, &lights);
        }

        // Opaque scene first, effects that live behind the stack next, then
        // the stack, and finally everything translucent that sits in front.
        frame.begin_scene();
        draw_well(textures, &lights, time, self.get_lava_splashes());
        draw_embers(time, &lights, textures);
        if let Some((trail, strength)) = self.get_hard_drop_trail() {
            draw_hard_drop_trail(&trail, strength, textures);
        }

        let wash = if is_game_over { GAME_OVER_WASH } else { 0.0 };
        self.get_grid_locked().draw(GridDrawArgs {
            style: BlockStyle::Solid,
            textures,
            lights,
            time,
            wash,
        });
        self.get_grid_active().draw(GridDrawArgs {
            style: BlockStyle::Solid,
            textures,
            lights,
            time,
            wash,
        });
        if !is_game_over {
            self.get_grid_ghost().draw(GridDrawArgs {
                style: BlockStyle::Ghost,
                textures,
                lights,
                time,
                wash: 0.0,
            });
        }
        draw_piece_previews(&layout, self.get_piece_previews(), textures, lights);
        draw_held_piece(&layout, self.get_held_piece(), textures, lights);
        draw_shrapnel(self.get_shrapnel(), textures);
        let (clear_count, clear_remaining) = self.get_clear_effect();
        draw_clear_flash(
            self.get_clear_row_mask(),
            clear_remaining,
            clear_count,
            textures,
        );
        draw_lamp_glow(&lights, textures);

        let hud = frame.hud();
        draw_score(&hud, &layout, self.get_score());
        draw_panel_labels(&hud, &layout);
        draw_level_and_rows_cleared(&hud, &layout, self.get_level(), self.get_rows_cleared());
        draw_controls(&hud, &layout);
        if let Some(award) = announcement {
            draw_score_announcement(&hud, &layout, award);
        }

        draw_game_effects(&hud, &layout, self, frame.shake);
    }
}

pub struct GridDrawArgs<'a> {
    style: BlockStyle,
    textures: &'a SceneTextures,
    lights: SceneLights,
    time: f64,
    /// How far block colours are washed toward dead grey (0.0 = none). Used
    /// to drain the stack of life once the game is over.
    wash: f32,
}

impl Drawable<GridDrawArgs<'_>> for Grid {
    fn draw(&self, args: GridDrawArgs<'_>) {
        for row_id in FIRST_VISIBLE_ROW_ID..GRID_COUNT_ROWS {
            for col_id in 0..GRID_COUNT_COLS {
                let Some(block) = self.get_cell(row_id, col_id) else {
                    continue;
                };

                let center = cell_center(row_id - FIRST_VISIBLE_ROW_ID, col_id);
                match args.style {
                    BlockStyle::Solid => block.draw(BlockArgs {
                        center,
                        textures: args.textures,
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

                        draw_ghost_cell(center, block.color, exterior, args.time, args.textures);
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
pub struct PiecePreviewArgs<'a> {
    /// World-space point that the piece's bounding box is centred on.
    center: Vec3,
    /// Edge length of one block cube, in world units.
    scale: f32,
    textures: &'a SceneTextures,
    lights: SceneLights,
}

impl Drawable<PiecePreviewArgs<'_>> for Piece {
    fn draw(&self, args: PiecePreviewArgs<'_>) {
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

                    draw_block_cube_scaled(position, block.color, scale, textures, &lights);
                }
            }
        }
    }
}

/// Placement and appearance of a single solid block inside the well.
pub struct BlockArgs<'a> {
    center: Vec3,
    textures: &'a SceneTextures,
    lights: SceneLights,
    wash: f32,
}

/// Dead, unlit steel that game-over blocks fade toward.
const COLOR_DEAD_BLOCK: Color = color_u8!(92, 90, 86, 255);

impl Drawable<BlockArgs<'_>> for Block {
    fn draw(&self, args: BlockArgs<'_>) {
        let BlockArgs {
            center,
            textures,
            lights,
            wash,
        } = args;
        let color = mix_color(self.color, COLOR_DEAD_BLOCK, wash);

        draw_block_cube(center, color, textures, &lights);
    }
}

/// Edge length of the tumbling cube used as the menu cursor, in world units.
const MENU_CURSOR_SIZE: f32 = 0.55;

/// Depth of the menu cursor cube. The HUD layer is not depth tested, so this
/// only has to lie inside the camera's clip range, ahead of the cabinet.
const MENU_CURSOR_Z: f32 = 1.7;

/// Draw a slowly tumbling block cube at a framebuffer position, on top of the
/// menu. Drawn in the HUD layer, where depth testing is off; a convex cube
/// shows only its camera-facing faces, which never overlap, so it needs none.
fn draw_menu_cursor(frame: &Frame, screen_center: Vec2) {
    let time = frame.time as f32;
    let center = frame.point_at(screen_center, MENU_CURSOR_Z);
    let rotation = Vec3::new(time * 0.9, time * 1.4, 0.35);

    draw_tumbling_cube(center, rotation, MENU_CURSOR_SIZE, COLOR_AMBER, frame.textures);
}

impl Drawable<&Frame<'_>> for Menu<'_> {
    fn draw(&self, frame: &Frame<'_>) {
        if !self.is_visible {
            return;
        }

        let hud = frame.hud();
        let scale = hud_scale();
        let well = well_screen_rect();
        let lights = SceneLights::idle(frame.time);
        let item_height = MENU_ITEM_HEIGHT * scale;
        let is_main = self.title.eq_ignore_ascii_case("bloxide");
        let title = self.title.to_ascii_uppercase();

        hud.fill(
            Rect::new(0.0, 0.0, frame_width(), frame_height()),
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

        draw_instrument_panel(&hud, panel, &lights);

        let title_baseline = panel_y + ((MENU_PADDING_Y + MENU_TITLE_TEXT_SIZE) * scale);
        let title_size = if is_main { MENU_TITLE_TEXT_SIZE } else { 34.0 };

        // Embossed: a deep shadow down-right and a one-pixel lit rim up-left,
        // so the title reads as letters cast into the plate.
        draw_text_centered_at(
            &hud,
            &title,
            center_x + (3.0 * scale),
            title_baseline + (3.0 * scale),
            title_size,
            color_u8!(3, 3, 2, 220),
        );
        draw_text_centered_at(
            &hud,
            &title,
            center_x - 1.0,
            title_baseline - 1.0,
            title_size,
            color_u8!(255, 250, 232, 255),
        );
        draw_text_centered_at(&hud, &title, center_x, title_baseline, title_size, COLOR_TEXT);
        let section_y = (title_baseline + (18.0 * scale)).round();
        hud.hline(
            panel_x + (24.0 * scale),
            panel_x + panel_width - (24.0 * scale),
            section_y,
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
                &hud,
                item.label,
                row.x + (30.0 * scale),
                baseline,
                MENU_ITEM_TEXT_SIZE,
                if active { COLOR_TEXT } else { COLOR_TEXT_MUTED },
            );
        }

        draw_text_centered_at(
            &hud,
            "UP/DOWN MOVE  ENTER SELECT",
            center_x,
            panel_y + panel_height - (16.0 * scale),
            18.0,
            COLOR_TEXT_MUTED,
        );

        if let Some(center) = cursor_center {
            draw_menu_cursor(frame, center);
        }
    }
}

impl Drawable<&Frame<'_>> for HighScoreManager {
    fn draw(&self, frame: &Frame<'_>) {
        let hud = frame.hud();
        let label = "PERSONAL BEST";
        let score = self.get_high_score().to_formatted_string(&Locale::en);
        let lights = SceneLights::idle(frame.time);
        let rect = snap_rect(Rect::new(frame_width() - 112.0, 8.0, 102.0, 34.0));
        draw_instrument_panel(&hud, rect, &lights);
        draw_engraved_text(&hud, label, rect.x + 8.0, rect.y + 14.0, 16.0, COLOR_TEXT_MUTED);
        draw_text_right_at(
            &hud,
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
            layout.award,
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
    fn score_award_panel_fits_between_hold_and_controls_outside_the_well() {
        let layout = hud_layout();
        let well = well_screen_rect();

        assert_eq!(layout.award.x, layout.hold.x);
        assert_eq!(layout.award.w, layout.hold.w);
        assert!(layout.award.y > layout.hold.y + layout.hold.h);
        assert!(layout.award.y + layout.award.h < layout.controls.y);
        assert!(!layout.award.overlaps(&well));

        for index in 0..4 {
            let baseline = layout.award.y
                + (AWARD_FIRST_BASELINE + index as f32 * AWARD_LINE_PITCH) * hud_scale();
            let height = SMALL_GLYPH_HEIGHT * text_pixel(AWARD_TEXT_SIZE);

            assert!(baseline - height > layout.award.y + HUD_HEADER_DIVIDER_OFFSET * hud_scale());
            assert!(baseline < layout.award.y + layout.award.h - PANEL_FRAME);
        }
    }

    #[test]
    fn score_announcements_show_clear_type_and_separate_chain_badges() {
        use crate::scoring::{score_lock, LockEvent, ScoringState};

        let event = LockEvent { spin: SpinKind::None, lines: 4, level: 1 };
        let (state, _) = score_lock(event, ScoringState::default());
        let (_, award) = score_lock(event, state);
        let text = score_announcement_text(award);

        assert_eq!(text.headline, "LINE CLEAR");
        assert_eq!(text.clear, "TETRIS");
        assert_eq!(text.points, "+1,250");
        assert_eq!(text.back_to_back, "B2B");
        assert_eq!(text.combo, "COMBO 1");

        for (spin, headline, points) in [
            (SpinKind::Mini, "T-SPIN MINI", "+400"),
            (SpinKind::Full, "T-SPIN", "+1,200"),
        ] {
            let (_, award) = score_lock(
                LockEvent { spin, lines: 2, level: 1 },
                ScoringState::default(),
            );
            let text = score_announcement_text(award);

            assert_eq!(text.headline, headline);
            assert_eq!(text.clear, "DOUBLE");
            assert_eq!(text.points, points);
            assert!(text.back_to_back.is_empty());
            assert!(text.combo.is_empty());
        }
    }

    #[test]
    fn award_text_and_large_values_fit_the_panel_and_have_bitmap_glyphs() {
        use crate::{
            pixel_font::SMALL_GLYPH_CHARS,
            scoring::{score_lock, LockEvent, ScoringState},
        };

        let rect = hud_layout().award;
        let available_width = rect.w - 32.0 * hud_scale();
        let mut awards = Vec::new();

        for (spin, max_lines) in [(SpinKind::None, 4), (SpinKind::Mini, 2), (SpinKind::Full, 3)] {
            for lines in 0..=max_lines {
                let (_, award) = score_lock(
                    LockEvent { spin, lines, level: 20 },
                    ScoringState::default(),
                );
                awards.push(award);
            }
        }

        awards.push(ScoreAward {
            event: LockEvent { spin: SpinKind::Mini, lines: 2, level: 20 },
            base: usize::MAX,
            back_to_back_bonus: 0,
            combo_bonus: 0,
            combo: Some(usize::MAX),
        });

        for award in awards {
            let text = score_announcement_text(award);

            for (line, size) in [
                (text.headline, LABEL_TEXT_SIZE),
                (text.clear, AWARD_TEXT_SIZE),
                (text.points.as_str(), AWARD_TEXT_SIZE),
                (text.back_to_back, AWARD_TEXT_SIZE),
                (text.combo.as_str(), AWARD_TEXT_SIZE),
            ] {
                assert!(pixel_text_metrics(line, size).0 <= available_width, "{line}");
                assert!(line.chars().all(|c| c == ' ' || SMALL_GLYPH_CHARS.contains(c)), "{line}");
            }
        }
    }

    #[test]
    fn hold_preview_is_vertically_centered_below_its_header() {
        let layout = hud_layout();
        let (content_top, content_bottom) = preview_content_vertical_bounds(layout.hold);
        let anchor_y = world_to_screen(hold_piece_anchor(&layout)).y;
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
        let layout = hud_layout();
        let (content_top, content_bottom) = preview_content_vertical_bounds(layout.next);
        let slot_height = (content_bottom - content_top) / 3.0;
        let minimum_slot_padding = 2.0;
        let anchors = [
            next_piece_anchor(&layout, 0),
            next_piece_anchor(&layout, 1),
            next_piece_anchor(&layout, 2),
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

    #[test]
    fn camera_shake_is_still_for_a_settled_board() {
        let high_scores = HighScoreManager::new();
        let game_state = GameState::new(&high_scores);

        assert_eq!(camera_shake(&game_state, 1.234), Vec2::ZERO);
    }
}
