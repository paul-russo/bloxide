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

/// Horizontal/depth reference for HOLD. Its world-space `y` is derived from
/// the card's padded screen-space content area.
const HOLD_PREVIEW_REFERENCE: Vec3 = Vec3::new(-10.2, 0.0, 0.0);

/// Edge length of a preview block. Pieces outside the well are drawn smaller
/// than playfield blocks so a 4-wide I piece fits the side panel.
const PREVIEW_SCALE: f32 = 0.74;

/// How far the stack's colours drain toward dead steel once the game is over.
const GAME_OVER_WASH: f32 = 0.7;

const LABEL_TEXT_SIZE: f32 = 20.0;
const LINE_COUNT_TEXT_SIZE: f32 = 28.0;
const LINE_COUNT_COMPACT_TEXT_SIZE: f32 = 16.0;
const CONTROL_TEXT_SIZE: f32 = 16.0;
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
const NEXT_SLOT_COUNT: usize = 3;
const HUD_HEADER_DIVIDER_OFFSET: f32 = 36.0;
const HUD_PREVIEW_TOP_PADDING: f32 = 12.0;
const HUD_PREVIEW_BOTTOM_PADDING: f32 = 12.0;
const HUD_AWARD_GAP: f32 = 16.0;
const AWARD_TEXT_SIZE: f32 = 16.0;
const AWARD_POINTS_TEXT_SIZE: f32 = 28.0;
const AWARD_CLEAR_BASELINE: f32 = 58.0;
const AWARD_POINTS_BASELINE: f32 = 94.0;
const AWARD_B2B_BASELINE: f32 = 112.0;
const AWARD_COMBO_BASELINE: f32 = 130.0;
const AWARD_BADGE_PIXELS: f32 = 10.0;
const AWARD_METER_SEGMENTS: usize = 8;
const AWARD_METER_HEIGHT: f32 = 2.0;
const CONTROL_ROW_TOP: f32 = 48.0;
const CONTROL_ROW_PITCH: f32 = 28.0;
const CONTROL_KEY_HEIGHT: f32 = 22.0;
const CONTROL_BOTTOM_PADDING: f32 = 8.0;
const PROGRESS_PIP_SIZE: f32 = 5.0;
const PROGRESS_PIP_GAP: f32 = 2.0;
const CONTROL_ROWS: [(&str, &str, f32); 5] = [
    ("L/R", "MOVE", 42.0),
    ("Z/X", "ROTATE", 42.0),
    ("SPACE", "DROP", 62.0),
    ("C", "HOLD", 28.0),
    ("ESC", "PAUSE", 42.0),
];

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

fn controls_panel_height() -> f32 {
    let scale = hud_scale();
    let last_row_top =
        ((CONTROL_ROW_TOP + (CONTROL_ROWS.len() - 1) as f32 * CONTROL_ROW_PITCH) * scale).round();
    let key_height = (CONTROL_KEY_HEIGHT * scale).round();

    // Include the keycap's one-pixel drop shadow, the frame, and inner padding.
    (last_row_top + key_height + 1.0 + PANEL_FRAME + CONTROL_BOTTOM_PADDING * scale).ceil()
}

/// Round the desired height up to fit equal, whole-pixel clear slots.
fn next_panel_height() -> f32 {
    // Reserve the header, one pixel per header/separator rule, and the bottom
    // inner bevel and frame. The remaining height belongs entirely to slots.
    let fixed_height =
        HUD_HEADER_DIVIDER_OFFSET * hud_scale() + NEXT_SLOT_COUNT as f32 + PANEL_FRAME + 1.0;
    let slot_height = (((HUD_NEXT_HEIGHT * hud_scale()).round() - fixed_height)
        / NEXT_SLOT_COUNT as f32)
        .ceil();

    fixed_height + slot_height * NEXT_SLOT_COUNT as f32
}

/// Clear face rectangles, excluding the header/separator rules and inner bevel.
/// Both preview centres and separator positions come from these slots.
fn next_slot_rects(rect: Rect) -> [Rect; NEXT_SLOT_COUNT] {
    let inset = PANEL_FRAME + 1.0;
    let top = rect.y + HUD_HEADER_DIVIDER_OFFSET * hud_scale() + 1.0;
    let bottom = rect.y + rect.h - inset;
    let slot_height = (bottom - top - (NEXT_SLOT_COUNT - 1) as f32) / NEXT_SLOT_COUNT as f32;

    std::array::from_fn(|index| {
        Rect::new(
            rect.x + inset,
            top + index as f32 * (slot_height + 1.0),
            rect.w - inset * 2.0,
            slot_height,
        )
    })
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
    let lower_height = controls_panel_height();

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
            next_panel_height(),
        ),
        controls: Rect::new(left_x, lower_y, panel_width, lower_height),
        stats: Rect::new(right_x, lower_y, panel_width, lower_height),
        score: lintel_screen_rect(),
    }
}

/// HOLD's padded preview range; NEXT uses the actual clear-slot rectangles.
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

fn next_piece_anchor(layout: &HudLayout, index: usize, shake: Vec2) -> Vec3 {
    debug_assert!(index < NEXT_SLOT_COUNT);
    let slot = next_slot_rects(layout.next)[index];
    ScreenPlane::new(0.0, shake).world(vec2(slot.x + slot.w * 0.5, slot.y + slot.h * 0.5))
}

fn header_band_height() -> f32 {
    HUD_HEADER_DIVIDER_OFFSET * hud_scale() - PANEL_FRAME - 1.0
}

/// Both outer titles and inner section headers use identical typography,
/// colour, horizontal inset, and positioning within their header strip.
fn panel_header_text_origin(rect: Rect, top: f32) -> Vec2 {
    let text_height = SMALL_GLYPH_HEIGHT * text_pixel(LABEL_TEXT_SIZE);

    vec2(
        rect.x + 16.0 * hud_scale(),
        (top + (header_band_height() - text_height - 1.0) * 0.5 + text_height).floor(),
    )
}

fn draw_panel_header(hud: &Surface, rect: Rect, label: &str) {
    draw_panel_header_at(hud, rect, label, rect.y + PANEL_FRAME + 1.0);
}

fn draw_panel_header_at(hud: &Surface, rect: Rect, label: &str, top: f32) {
    let inset = PANEL_FRAME + 1.0;
    let origin = panel_header_text_origin(rect, top);
    let right = rect.x + rect.w - 16.0 * hud_scale();
    let divider_y = top + header_band_height();

    hud.fill(
        Rect::new(
            rect.x + inset,
            top,
            rect.w - inset * 2.0,
            header_band_height(),
        ),
        COLOR_PANEL_HEADER,
    );
    draw_engraved_text(hud, label, origin.x, origin.y, LABEL_TEXT_SIZE, COLOR_TEXT_MUTED);
    hud.hline(origin.x, right, divider_y, COLOR_PANEL_BORDER);
}

fn next_slot_guides(rect: Rect) -> [Rect; NEXT_SLOT_COUNT - 1] {
    let slots = next_slot_rects(rect);
    let inset = 16.0 * hud_scale();

    std::array::from_fn(|index| {
        Rect::new(
            rect.x + inset,
            slots[index].y + slots[index].h,
            rect.w - inset * 2.0,
            1.0,
        )
    })
}

fn draw_next_slot_guides(hud: &Surface, layout: &HudLayout) {
    for guide in next_slot_guides(layout.next) {
        hud.fill(guide, color_u8!(69, 65, 54, 120));
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
    draw_panel_header(hud, layout.stats, "LEVEL");
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

struct StatsReadoutLayout {
    level_area: Rect,
    lines_header_top: f32,
    lines: Rect,
    lines_size: f32,
    progress: Rect,
}

fn line_count_text(rows_cleared: usize) -> String {
    if rows_cleared > 99_999 {
        return String::from("99,999+");
    }

    rows_cleared.to_formatted_string(&Locale::en)
}

fn stats_readout_layout(rect: Rect, line_text: &str) -> StatsReadoutLayout {
    let scale = hud_scale();
    let lines_size = if pixel_text_metrics(line_text, LINE_COUNT_TEXT_SIZE).0 <= rect.w - 32.0 * scale {
        LINE_COUNT_TEXT_SIZE
    } else {
        LINE_COUNT_COMPACT_TEXT_SIZE
    };
    let (line_width, line_height, _) = pixel_text_metrics(line_text, lines_size);
    let window_width = digit_text_width("00", READOUT_PIXEL) + READOUT_PADDING * 2.0;
    let window_height = readout_height();
    let window_top = (rect.y + (HUD_HEADER_DIVIDER_OFFSET + 8.0) * scale).round();
    let bar_width = PROGRESS_PIP_SIZE * 10.0 + PROGRESS_PIP_GAP * 9.0;
    let progress = Rect::new(
        (rect.x + (rect.w - bar_width) * 0.5).round(),
        (rect.y + rect.h - PANEL_FRAME - 12.0 * scale - PROGRESS_PIP_SIZE).round(),
        bar_width,
        PROGRESS_PIP_SIZE,
    );

    let lines_header_top = (window_top + window_height + 4.0 * scale).round();

    StatsReadoutLayout {
        level_area: Rect::new(
            (rect.x + (rect.w - window_width) * 0.5).round(),
            window_top,
            window_width,
            window_height,
        ),
        lines_header_top,
        lines: Rect::new(
            (rect.x + (rect.w - line_width) * 0.5).round(),
            (lines_header_top + header_band_height() + 4.0 * scale).round(),
            line_width,
            line_height,
        ),
        lines_size,
        progress,
    }
}

fn level_digit_origin(area: Rect, text: &str) -> Vec2 {
    vec2(
        (area.x + (area.w - digit_text_width(text, READOUT_PIXEL)) * 0.5).round(),
        (area.y + (area.h - DIGIT_HEIGHT as f32 * READOUT_PIXEL) * 0.5).round(),
    )
}

/// Centred values beneath matching LEVEL and LINES section headers.
fn draw_level_and_rows_cleared(hud: &Surface, layout: &HudLayout, level: usize, rows_cleared: usize) {
    let line_text = line_count_text(rows_cleared);
    let positions = stats_readout_layout(layout.stats, &line_text);
    let level_text = format!("{level:02}");
    let origin = level_digit_origin(positions.level_area, &level_text);

    // Keep the numerals and their subtle bloom, without a separate glass inset.
    hud.digit_text(
        &level_text,
        origin.x + 1.0,
        origin.y + 1.0,
        READOUT_PIXEL,
        shaded(COLOR_TEXT, 0.25),
    );
    hud.digit_text(
        &level_text,
        origin.x,
        origin.y,
        READOUT_PIXEL,
        COLOR_TEXT,
    );
    draw_panel_header_at(hud, layout.stats, "LINES", positions.lines_header_top);
    draw_engraved_text(
        hud,
        &line_text,
        positions.lines.x,
        positions.lines.y + positions.lines.h,
        positions.lines_size,
        COLOR_TEXT,
    );

    let completed = rows_cleared % 10;

    for segment in 0..10 {
        draw_indicator_pip(
            hud,
            positions.progress.x + segment as f32 * (PROGRESS_PIP_SIZE + PROGRESS_PIP_GAP),
            positions.progress.y,
            PROGRESS_PIP_SIZE,
            segment < completed,
        );
    }
}

fn control_keycap_rect(panel: Rect, index: usize, width: f32) -> Rect {
    let scale = hud_scale();

    Rect::new(
        (panel.x + 16.0 * scale).round(),
        (panel.y + (CONTROL_ROW_TOP + index as f32 * CONTROL_ROW_PITCH) * scale).round(),
        (width * scale).round(),
        (CONTROL_KEY_HEIGHT * scale).round(),
    )
}

fn control_label_baseline(rect: Rect) -> f32 {
    let text_height = SMALL_GLYPH_HEIGHT * text_pixel(CONTROL_TEXT_SIZE);

    (rect.y + (rect.h + text_height - 1.0) * 0.5).floor()
}

/// A raised keycap: lit along its top and left, shadowed bottom and right.
fn draw_keycap(hud: &Surface, label: &str, rect: Rect) {
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
        control_label_baseline(rect),
        CONTROL_TEXT_SIZE,
        COLOR_TEXT,
    );
}

fn draw_controls(hud: &Surface, layout: &HudLayout) {
    let rect = layout.controls;
    let right = rect.x + rect.w - 16.0 * hud_scale();

    for (index, (key, action, key_width)) in CONTROL_ROWS.iter().enumerate() {
        let keycap = control_keycap_rect(rect, index, *key_width);
        draw_keycap(hud, key, keycap);
        draw_text_right_at(
            hud,
            action,
            right,
            control_label_baseline(keycap),
            CONTROL_TEXT_SIZE,
            COLOR_TEXT_MUTED,
        );
    }
}

struct ScoreAnnouncementText {
    headline: &'static str,
    clear: &'static str,
    medal: &'static str,
    points: String,
    back_to_back: &'static str,
    combo: String,
}

fn score_announcement_text(award: ScoreAward) -> ScoreAnnouncementText {
    let headline = match (award.event.spin, award.event.lines) {
        (SpinKind::None, 4) => "CARNAGE",
        (SpinKind::None, _) => "LINE CLEAR",
        (SpinKind::Mini, _) => "T-SPIN MINI",
        (SpinKind::Full, _) => "T-SPIN",
    };
    let clear = match award.event.lines {
        0 => "NO LINES",
        1 => "SINGLE",
        2 => "DOUBLE",
        3 => "TRIPLE",
        4 => "TETRIS",
        _ => "CLEAR",
    };
    let medal = match (award.event.spin, award.event.lines) {
        (SpinKind::None, 1) => "1",
        (SpinKind::None, 2) => "2",
        (SpinKind::None, 3) => "3",
        (SpinKind::None, 4) => "4",
        (SpinKind::None, _) => "!",
        _ => "T",
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
        medal,
        points,
        back_to_back: if award.back_to_back_bonus > 0 { "B2B" } else { "" },
        combo,
    }
}

struct ScoreAnnouncementLayout {
    medal: Rect,
    clear: Rect,
    points: Rect,
    points_size: f32,
    back_to_back: Rect,
    combo: Rect,
    meter: Rect,
    impact: f32,
}

fn centered_label_bounds(rect: Rect, text: &str, baseline: f32, size: f32) -> Rect {
    let (width, height, _) = pixel_text_metrics(text, size);

    Rect::new(
        (rect.x + (rect.w - width) * 0.5).round(),
        (baseline - height).round(),
        width,
        height,
    )
}

fn award_impact(remaining: f32) -> f32 {
    // A quarter-second impact within the award's two-second lifetime.
    ((remaining.clamp(0.0, 1.0) - 0.875) / 0.125).clamp(0.0, 1.0)
}

fn score_announcement_layout(
    rect: Rect,
    text: &ScoreAnnouncementText,
    remaining: f32,
) -> ScoreAnnouncementLayout {
    let scale = hud_scale();
    let pixel = text_pixel(AWARD_TEXT_SIZE);
    let impact = award_impact(remaining);
    let medal_size = AWARD_BADGE_PIXELS * pixel;
    let (clear_width, clear_height, _) = pixel_text_metrics(text.clear, AWARD_TEXT_SIZE);
    let gap = 4.0 * pixel;
    let group_x = (rect.x + (rect.w - medal_size - gap - clear_width) * 0.5).round();
    let clear_baseline = rect.y + AWARD_CLEAR_BASELINE * scale;
    let available_width = rect.w - 32.0 * scale;
    let points_size = if pixel_text_metrics(&text.points, AWARD_POINTS_TEXT_SIZE).0 <= available_width {
        AWARD_POINTS_TEXT_SIZE
    } else {
        AWARD_TEXT_SIZE
    };

    ScoreAnnouncementLayout {
        medal: Rect::new(
            group_x,
            (clear_baseline - (8.0 + impact.round()) * pixel).round(),
            medal_size,
            medal_size,
        ),
        clear: Rect::new(
            group_x + medal_size + gap,
            clear_baseline - clear_height,
            clear_width,
            clear_height,
        ),
        points: centered_label_bounds(
            rect, &text.points, rect.y + AWARD_POINTS_BASELINE * scale, points_size,
        ),
        points_size,
        back_to_back: centered_label_bounds(
            rect, text.back_to_back, rect.y + AWARD_B2B_BASELINE * scale, AWARD_TEXT_SIZE,
        ),
        combo: centered_label_bounds(
            rect, &text.combo, rect.y + AWARD_COMBO_BASELINE * scale, AWARD_TEXT_SIZE,
        ),
        meter: Rect::new(
            rect.x + 16.0 * scale,
            rect.y + rect.h - PANEL_FRAME - AWARD_METER_HEIGHT,
            available_width,
            AWARD_METER_HEIGHT,
        ),
        impact,
    }
}

fn award_meter_segments(rect: Rect, remaining: f32) -> impl Iterator<Item = (Rect, f32)> {
    let pitch = (rect.w / AWARD_METER_SEGMENTS as f32).floor();
    let width = (pitch - 2.0).max(1.0);
    let total_width = pitch * (AWARD_METER_SEGMENTS - 1) as f32 + width;
    let left = (rect.x + (rect.w - total_width) * 0.5).round();
    let units = remaining.clamp(0.0, 1.0) * AWARD_METER_SEGMENTS as f32;

    (0..AWARD_METER_SEGMENTS).map(move |index| {
        let segment = Rect::new(left + index as f32 * pitch, rect.y, width, rect.h);
        let filled_width = (width * (units - index as f32).clamp(0.0, 1.0)).floor();

        (segment, filled_width)
    })
}

fn draw_score_announcement(
    hud: &Surface,
    layout: &HudLayout,
    award: ScoreAward,
    remaining: f32,
) {
    let text = score_announcement_text(award);
    let rect = layout.award;
    let positions = score_announcement_layout(rect, &text, remaining);

    if positions.impact > 0.0 {
        let top = rect.y + HUD_HEADER_DIVIDER_OFFSET * hud_scale() + 1.0;
        hud.fill(
            Rect::new(
                rect.x + PANEL_FRAME, top, rect.w - PANEL_FRAME * 2.0,
                rect.y + rect.h - PANEL_FRAME - top,
            ),
            Color::new(COLOR_AMBER.r, COLOR_AMBER.g, COLOR_AMBER.b, positions.impact * 0.16),
        );
    }

    draw_panel_header(hud, rect, text.headline);
    hud.fill(
        positions.medal,
        mix_color(color_u8!(74, 44, 18, 255), COLOR_AMBER, 0.2 + positions.impact * 0.35),
    );
    draw_bevel(
        hud,
        positions.medal,
        mix_color(COLOR_AMBER, COLOR_TEXT, positions.impact),
        COLOR_PANEL_DARK,
        false,
    );
    let pixel = text_pixel(AWARD_TEXT_SIZE);
    draw_text_centered_at(
        hud,
        text.medal,
        positions.medal.x + positions.medal.w * 0.5,
        (positions.medal.y + (positions.medal.h + SMALL_GLYPH_HEIGHT * pixel - pixel) * 0.5).floor(),
        AWARD_TEXT_SIZE,
        COLOR_TEXT,
    );

    for (line, bounds, size, color) in [
        (text.clear, positions.clear, AWARD_TEXT_SIZE, COLOR_TEXT),
        (text.points.as_str(), positions.points, positions.points_size, COLOR_AMBER),
        (text.back_to_back, positions.back_to_back, AWARD_TEXT_SIZE, COLOR_AMBER),
        (text.combo.as_str(), positions.combo, AWARD_TEXT_SIZE, COLOR_TEXT_MUTED),
    ] {
        if line.is_empty() {
            continue;
        }

        draw_pixel_text(hud, line, bounds.x, bounds.y + bounds.h, size, color);
    }

    for (segment, filled_width) in award_meter_segments(positions.meter, remaining) {
        hud.fill(segment, color_u8!(44, 39, 25, 255));
        if filled_width > 0.0 {
            hud.fill(
                Rect::new(segment.x, segment.y, filled_width, segment.h),
                mix_color(COLOR_AMBER, COLOR_TEXT, positions.impact * 0.6),
            );
        }
    }
}

/// Draw the next-piece queue into the right side panel.
fn draw_piece_previews(
    layout: &HudLayout,
    piece_previews: [Piece; NEXT_SLOT_COUNT],
    textures: &SceneTextures,
    lights: SceneLights,
    shake: Vec2,
) {
    for (offset, piece) in piece_previews.iter().enumerate() {
        piece.draw(PiecePreviewArgs {
            center: next_piece_anchor(layout, offset, shake),
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

fn draw_game_effects(hud: &Surface, game_state: &GameState<'_>, shake: Vec2) {
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
        draw_piece_previews(&layout, self.get_piece_previews(), textures, lights, frame.shake);
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
            draw_score_announcement(&hud, &layout, award, self.get_score_announcement_remaining());
        }

        draw_game_effects(&hud, self, frame.shake);
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

    fn preview_screen_bounds(piece: Piece, center: Vec3, shake: Vec2) -> Rect {
        let (blocks, _, _) = piece.get_blocks(0);
        let (min_row, max_row, min_col, max_col) = piece.get_trimmed_bounds(0);
        let span_cols = (max_col - min_col) as f32;
        let span_rows = (max_row - min_row) as f32;
        let origin_x = center.x - (span_cols * PREVIEW_SCALE / 2.0) + (PREVIEW_SCALE / 2.0);
        let origin_y = center.y + (span_rows * PREVIEW_SCALE / 2.0) - (PREVIEW_SCALE / 2.0);
        let half_cube = BLOCK_INSET * PREVIEW_SCALE * 0.5;
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);

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

                // Include the depth of every rendered cube, not just cell centres.
                for x_sign in [-1.0, 1.0] {
                    for y_sign in [-1.0, 1.0] {
                        for z_sign in [-1.0, 1.0] {
                            let corner = position + vec3(x_sign, y_sign, z_sign) * half_cube;
                            let screen = world_to_screen_with_shake(corner, shake);
                            min = min.min(screen);
                            max = max.max(screen);
                        }
                    }
                }
            }
        }

        Rect::new(min.x, min.y, max.x - min.x, max.y - min.y)
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
    fn every_panel_title_fits_between_the_frame_and_header_divider() {
        let layout = hud_layout();

        for (rect, label) in [
            (layout.hold, "HOLD"),
            (layout.next, "NEXT"),
            (layout.controls, "CONTROLS"),
            (layout.stats, "LEVEL"),
            (layout.award, "LINE CLEAR"),
            (layout.award, "CARNAGE"),
            (layout.award, "T-SPIN"),
            (layout.award, "T-SPIN MINI"),
        ] {
            let baseline = panel_header_text_origin(rect, rect.y + PANEL_FRAME + 1.0).y;
            let (width, height, _) = pixel_text_metrics(label, LABEL_TEXT_SIZE);
            let divider = rect.y + HUD_HEADER_DIVIDER_OFFSET * hud_scale();

            assert_eq!(baseline.fract(), 0.0);
            assert!(baseline - height >= rect.y + PANEL_FRAME + 1.0, "{label}");
            assert!(baseline + 1.0 < divider, "{label} engraving shadow");
            assert!(
                rect.x + 16.0 * hud_scale() + width + 1.0 <= rect.x + rect.w - PANEL_FRAME,
                "{label} width"
            );
        }
    }

    #[test]
    fn all_five_controls_and_their_shadows_fit_inside_the_frame() {
        let rect = hud_layout().controls;
        let divider = rect.y + HUD_HEADER_DIVIDER_OFFSET * hud_scale();
        let right = rect.x + rect.w - 16.0 * hud_scale();

        assert_eq!(CONTROL_ROWS.last().map(|row| (row.0, row.1)), Some(("ESC", "PAUSE")));
        assert_eq!(CONTROL_ROWS.iter().filter(|row| row.0 == "ESC").count(), 1);

        for (index, (key, action, width)) in CONTROL_ROWS.iter().enumerate() {
            let keycap = control_keycap_rect(rect, index, *width);
            let baseline = control_label_baseline(keycap);
            let (key_width, key_height, _) = pixel_text_metrics(key, CONTROL_TEXT_SIZE);
            let (action_width, action_height, _) = pixel_text_metrics(action, CONTROL_TEXT_SIZE);

            assert!(keycap.y > divider, "{key}");
            assert!(keycap.x >= rect.x + PANEL_FRAME + 1.0, "{key}");
            assert!(keycap.x + keycap.w + 1.0 < right - action_width, "{key}/{action}");
            assert!(
                keycap.y + keycap.h + 1.0 + CONTROL_BOTTOM_PADDING * hud_scale()
                    <= rect.y + rect.h - PANEL_FRAME,
                "{key} bottom shadow"
            );
            assert!(key_width <= keycap.w - 2.0, "{key} glyph width");
            assert!(baseline - key_height >= keycap.y + 1.0, "{key} glyph top");
            assert!(baseline <= keycap.y + keycap.h - 1.0, "{key} glyph bottom");
            assert!(baseline - action_height > divider, "{action}");
        }
    }

    #[test]
    fn five_digit_line_counts_fit_below_the_centered_level_without_overlapping() {
        let rect = hud_layout().stats;
        let original = stats_readout_layout(rect, &line_count_text(0));

        for lines in [0, 99, 100, 999, 1000, 12_345, 99_999, 100_000, usize::MAX] {
            let text = line_count_text(lines);
            let positions = stats_readout_layout(rect, &text);
            let value_center = positions.level_area.x + positions.level_area.w * 0.5;

            assert_eq!(
                positions.lines_size,
                if lines <= 99_999 { LINE_COUNT_TEXT_SIZE } else { LINE_COUNT_COMPACT_TEXT_SIZE }
            );
            assert!((value_center - (rect.x + rect.w * 0.5)).abs() <= 0.5);
            assert!(positions.level_area.x >= rect.x + PANEL_FRAME + 1.0);
            assert!(
                positions.level_area.y
                    > rect.y + HUD_HEADER_DIVIDER_OFFSET * hud_scale()
            );
            assert!(positions.lines_header_top > positions.level_area.y + positions.level_area.h);
            assert!(positions.lines.x >= rect.x + PANEL_FRAME + 1.0, "{text}");
            assert!(
                positions.lines.x + positions.lines.w + 1.0 <= rect.x + rect.w - PANEL_FRAME,
                "{text}"
            );
            assert!(positions.lines.y > positions.lines_header_top + header_band_height());
            assert!(positions.lines.y + positions.lines.h + 1.0 < positions.progress.y);
            assert!(positions.progress.y + positions.progress.h < rect.y + rect.h - PANEL_FRAME);
            assert_eq!(
                (positions.level_area.x, positions.level_area.y),
                (original.level_area.x, original.level_area.y)
            );

            for level in [1, 9, 10, 20] {
                assert!(
                    digit_text_width(&format!("{level:02}"), READOUT_PIXEL)
                        <= positions.level_area.w - READOUT_PADDING * 2.0
                );
            }
        }
    }

    #[test]
    fn plain_level_digits_keep_their_size_and_centering() {
        let panel = hud_layout().stats;
        let area = stats_readout_layout(panel, &line_count_text(99_999)).level_area;
        let height = DIGIT_HEIGHT as f32 * READOUT_PIXEL;

        for level in [1, 9, 10, 20] {
            let text = format!("{level:02}");
            let width = digit_text_width(&text, READOUT_PIXEL);
            let origin = level_digit_origin(area, &text);

            assert_eq!(origin.x + width * 0.5, panel.x + panel.w * 0.5);
            assert_eq!(origin.y + height * 0.5, area.y + area.h * 0.5);
            assert!(origin.x >= area.x && origin.x + width + 1.0 <= area.x + area.w);
            assert!(origin.y >= area.y && origin.y + height + 1.0 <= area.y + area.h);
        }
    }

    #[test]
    fn line_count_formatting_keeps_five_digits_and_bounds_larger_values() {
        assert_eq!(line_count_text(0), "0");
        assert_eq!(line_count_text(100), "100");
        assert_eq!(line_count_text(12_345), "12,345");
        assert_eq!(line_count_text(99_999), "99,999");
        assert_eq!(line_count_text(100_000), "99,999+");
        assert_eq!(line_count_text(usize::MAX), "99,999+");
    }

    #[test]
    fn level_and_lines_headers_share_insets_and_vertical_text_metrics() {
        let rect = hud_layout().stats;
        let layout = stats_readout_layout(rect, &line_count_text(99_999));
        let level_top = rect.y + PANEL_FRAME + 1.0;
        let level = panel_header_text_origin(rect, level_top);
        let lines = panel_header_text_origin(rect, layout.lines_header_top);

        assert_eq!(level.x, lines.x);
        assert_eq!(level.y - level_top, lines.y - layout.lines_header_top);
        assert!(lines.y < layout.lines.y);
        assert_eq!(level.x, rect.x + 16.0 * hud_scale());
        assert_eq!(layout.lines_header_top.fract(), 0.0);
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
    }

    #[test]
    fn score_announcements_show_clear_type_and_separate_chain_badges() {
        use crate::scoring::{score_lock, LockEvent, ScoringState};

        let event = LockEvent { spin: SpinKind::None, lines: 4, level: 1 };
        let (state, _) = score_lock(event, ScoringState::default());
        let (_, award) = score_lock(event, state);
        let text = score_announcement_text(award);

        assert_eq!(text.headline, "CARNAGE");
        assert_eq!(text.clear, "TETRIS");
        assert_eq!(text.medal, "4");
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
            assert_eq!(text.medal, "T");
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
        let (chain, _) = score_lock(
            LockEvent { spin: SpinKind::None, lines: 4, level: 1 },
            ScoringState::default(),
        );

        for (spin, max_lines) in [(SpinKind::None, 4), (SpinKind::Mini, 2), (SpinKind::Full, 3)] {
            for lines in 0..=max_lines {
                let (_, award) = score_lock(
                    LockEvent { spin, lines, level: 20 },
                    chain,
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
            let positions = score_announcement_layout(rect, &text, 1.0);

            for (line, size) in [
                (text.headline, LABEL_TEXT_SIZE),
                (text.clear, AWARD_TEXT_SIZE),
                (text.medal, AWARD_TEXT_SIZE),
                (text.points.as_str(), positions.points_size),
                (text.back_to_back, AWARD_TEXT_SIZE),
                (text.combo.as_str(), AWARD_TEXT_SIZE),
            ] {
                assert!(pixel_text_metrics(line, size).0 <= available_width, "{line}");
                assert!(line.chars().all(|c| c == ' ' || SMALL_GLYPH_CHARS.contains(c)), "{line}");
            }

            for remaining in [1.0, 0.95, 0.875, 0.5, 0.0] {
                let positions = score_announcement_layout(rect, &text, remaining);

                for bounds in [
                    positions.medal, positions.clear, positions.points,
                    positions.back_to_back, positions.combo, positions.meter,
                ] {
                    assert!(bounds.x >= rect.x + PANEL_FRAME + 1.0);
                    assert!(bounds.x + bounds.w <= rect.x + rect.w - PANEL_FRAME - 1.0);
                    assert!(bounds.y > rect.y + HUD_HEADER_DIVIDER_OFFSET * hud_scale());
                    assert!(bounds.y + bounds.h <= rect.y + rect.h - PANEL_FRAME);

                    for value in [bounds.x, bounds.y, bounds.w, bounds.h] {
                        assert_eq!(value.fract(), 0.0);
                    }
                }

                assert!(positions.clear.x > positions.medal.x + positions.medal.w);
                assert!(positions.points.y > positions.medal.y + positions.medal.h);
                assert!(positions.points.y > positions.clear.y + positions.clear.h);
                assert!(positions.back_to_back.y > positions.points.y + positions.points.h);
                assert!(positions.combo.y > positions.back_to_back.y + positions.back_to_back.h);
                assert!(positions.meter.y > positions.combo.y + positions.combo.h);
            }
        }
    }

    #[test]
    fn common_awards_use_larger_points_and_long_awards_fall_back_without_clipping() {
        use crate::scoring::{score_lock, LockEvent, ScoringState};

        let rect = hud_layout().award;

        for (event, expected_size) in [
            (LockEvent { spin: SpinKind::None, lines: 3, level: 1 }, AWARD_POINTS_TEXT_SIZE),
            (LockEvent { spin: SpinKind::Full, lines: 2, level: 1 }, AWARD_POINTS_TEXT_SIZE),
            (LockEvent { spin: SpinKind::Full, lines: 3, level: 20 }, AWARD_TEXT_SIZE),
        ] {
            let (_, award) = score_lock(event, ScoringState::default());
            let text = score_announcement_text(award);
            let positions = score_announcement_layout(rect, &text, 1.0);

            assert_eq!(positions.points_size, expected_size);
            assert_eq!(text.points.replace(',', ""), format!("+{}", award.total()));
        }
    }

    #[test]
    fn award_flash_and_segmented_meter_follow_the_remaining_lifetime() {
        let rect = Rect::new(8.0, 67.0, 72.0, AWARD_METER_HEIGHT);
        let full: f32 = award_meter_segments(rect, 1.0).map(|(_, filled)| filled).sum();
        let half: f32 = award_meter_segments(rect, 0.5).map(|(_, filled)| filled).sum();

        assert_eq!(award_impact(1.0), 1.0);
        assert!(award_impact(0.95) > 0.0 && award_impact(0.95) < 1.0);
        assert_eq!(award_impact(0.875), 0.0);
        assert_eq!(award_impact(0.0), 0.0);
        assert_eq!(half, full * 0.5);

        let mut previous = f32::INFINITY;

        for remaining in [1.0, 0.875, 0.5, 0.125, 0.0] {
            let mut total = 0.0;

            for (segment, filled) in award_meter_segments(rect, remaining) {
                assert!(segment.x >= rect.x && segment.x + segment.w <= rect.x + rect.w);
                assert!(filled >= 0.0 && filled <= segment.w);
                assert_eq!(filled.fract(), 0.0);
                total += filled;
            }

            assert!(total <= previous);
            previous = total;
        }

        assert_eq!(previous, 0.0);
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
    fn next_slots_share_equal_clear_space_between_pixel_aligned_rules() {
        let layout = hud_layout();
        let panel = layout.next;
        let slots = next_slot_rects(panel);
        let guides = next_slot_guides(panel);
        let header_divider = panel.y + PANEL_FRAME + 1.0 + header_band_height();
        let bottom_bevel = panel.y + panel.h - PANEL_FRAME - 1.0;
        let desired_height = (HUD_NEXT_HEIGHT * hud_scale()).round();
        let centers = slots.map(|slot| vec2(slot.x + slot.w * 0.5, slot.y + slot.h * 0.5));

        assert_eq!(panel.h, 142.0);
        assert!(panel.h >= desired_height && panel.h < desired_height + NEXT_SLOT_COUNT as f32);
        assert!(panel.y + panel.h < layout.stats.y);
        assert_eq!(slots.map(|slot| slot.y - panel.y), [19.0, 58.0, 97.0]);
        assert_eq!(slots.map(|slot| slot.y + slot.h - panel.y), [57.0, 96.0, 135.0]);
        assert_eq!(slots.map(|slot| slot.h), [38.0; NEXT_SLOT_COUNT]);
        assert_eq!(slots[0].y, header_divider + 1.0);
        assert_eq!(slots[NEXT_SLOT_COUNT - 1].y + slots[NEXT_SLOT_COUNT - 1].h, bottom_bevel);
        assert_eq!(centers[1] - centers[0], vec2(0.0, 39.0));
        assert_eq!(centers[2] - centers[1], centers[1] - centers[0]);

        for slot in slots {
            assert_eq!(slot.x, panel.x + PANEL_FRAME + 1.0);
            assert_eq!(slot.x + slot.w, panel.x + panel.w - PANEL_FRAME - 1.0);
        }

        for (index, guide) in guides.into_iter().enumerate() {
            assert_eq!(guide.y, slots[index].y + slots[index].h);
            assert_eq!(guide.y + guide.h, slots[index + 1].y);
            assert_eq!(guide.h, 1.0);
            assert_eq!(guide.x, panel.x + 16.0 * hud_scale());
            assert_eq!(guide.x + guide.w, panel.x + panel.w - 16.0 * hud_scale());
        }

        for bounds in slots.into_iter().chain(guides) {
            for value in [bounds.x, bounds.y, bounds.w, bounds.h] {
                assert_eq!(value.fract(), 0.0);
            }
        }
    }

    #[test]
    fn every_next_piece_stays_centered_inside_each_clear_slot_during_shake() {
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
        let slots = next_slot_rects(layout.next);
        let minimum_slot_padding = 5.0;
        let tolerance = 0.001;

        for piece in pieces {
            let resting_bounds = preview_screen_bounds(
                piece,
                next_piece_anchor(&layout, 0, Vec2::ZERO),
                Vec2::ZERO,
            );
            let expected_clearance = vec2(
                (slots[0].w - resting_bounds.w) * 0.5,
                (slots[0].h - resting_bounds.h) * 0.5,
            );

            // Rest, signed axis maxima, and all corners of camera_shake's
            // component envelope; the two oscillations need not peak together.
            for shake_x in [-0.25, 0.0, 0.25] {
                for shake_y in [-0.25, 0.0, 0.25] {
                    let shake = vec2(shake_x, shake_y);

                    for (slot_index, slot) in slots.into_iter().enumerate() {
                        let anchor = next_piece_anchor(&layout, slot_index, shake);
                        assert!(anchor.z.abs() < tolerance);

                        let expected_center = vec2(slot.x + slot.w * 0.5, slot.y + slot.h * 0.5);
                        let anchor_screen = world_to_screen_with_shake(anchor, shake);
                        assert!((anchor_screen - expected_center).length() < tolerance);

                        let bounds = preview_screen_bounds(piece, anchor, shake);
                        let center = vec2(bounds.x + bounds.w * 0.5, bounds.y + bounds.h * 0.5);
                        assert!(
                            (center.x - expected_center.x).abs() < tolerance,
                            "{} x centre in slot {slot_index} under {shake:?}",
                            piece.name
                        );
                        assert!(
                            (center.y - expected_center.y).abs() < tolerance,
                            "{} y centre in slot {slot_index} under {shake:?}",
                            piece.name
                        );

                        for (edge, clearance, expected) in [
                            ("left", bounds.x - slot.x, expected_clearance.x),
                            ("right", slot.x + slot.w - bounds.x - bounds.w, expected_clearance.x),
                            ("top", bounds.y - slot.y, expected_clearance.y),
                            ("bottom", slot.y + slot.h - bounds.y - bounds.h, expected_clearance.y),
                        ] {
                            assert!(
                                clearance >= minimum_slot_padding,
                                "{} {edge} clearance in slot {slot_index} under {shake:?}: {clearance}",
                                piece.name
                            );
                            assert!(
                                (clearance - expected).abs() < tolerance,
                                "{} {edge} clearance changes in slot {slot_index} under {shake:?}",
                                piece.name
                            );
                        }
                    }
                }
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
