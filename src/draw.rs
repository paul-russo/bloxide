use crate::block::Block;
use crate::game_state::GameState;
use crate::grid::{Grid, FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, GRID_COUNT_ROWS};
use crate::high_score_manager::HighScoreManager;
use crate::menu::Menu;
use crate::piece::Piece;
use crate::render3d::{
    cell_center, draw_block_cube, draw_block_cube_scaled, draw_block_outline, draw_shrapnel,
    draw_well, well_camera, world_to_screen, world_to_screen_with_shake, BlockTextures,
    BLOCK_INSET, RENDER_HEIGHT, RENDER_WIDTH, WELL_DEPTH, WELL_HEIGHT, WELL_WIDTH,
};
use macroquad::prelude::*;
use macroquad::texture::{render_target_ex, RenderTargetParams};
use num_format::{Locale, ToFormattedString};

/// The window is sized for the 3D scene rather than for a pixel-exact 2D grid:
/// it has to fit the well plus a side panel on each side, with the well framed
/// by [`crate::render3d::well_camera`].
pub const WINDOW_WIDTH: f32 = 1200.0;
pub const WINDOW_HEIGHT: f32 = 900.0;

/// Cleared behind the 3D scene. Slightly blue rather than pure black so the
/// well's dark interior still reads as recessed against it.
pub const BACKGROUND_COLOR: Color = color_u8!(10, 9, 7, 255);

#[derive(Clone)]
pub struct RenderSurface {
    pub target: RenderTarget,
    pub block_textures: BlockTextures,
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
            block_textures: BlockTextures::new(),
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

        draw_texture_ex(
            &self.target.texture,
            origin.x,
            origin.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(size),
                flip_y: true,
                ..Default::default()
            },
        );
    }
}

const COLOR_VOID: Color = color_u8!(13, 12, 10, 255);
const COLOR_BUNKER: Color = color_u8!(43, 38, 29, 255);
const COLOR_PANEL: Color = color_u8!(30, 29, 25, 255);
const COLOR_PANEL_HEADER: Color = color_u8!(24, 23, 20, 255);
const COLOR_PANEL_FRAME: Color = color_u8!(63, 60, 51, 255);
const COLOR_PANEL_LIGHT: Color = color_u8!(103, 98, 82, 255);
const COLOR_PANEL_BORDER: Color = color_u8!(78, 74, 62, 255);
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

const SCORE_TEXT_SIZE: f32 = 30.0;
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
const HUD_SCORE_WIDTH: f32 = 220.0;
const HUD_SCORE_HEIGHT: f32 = 84.0;
const HUD_SAFE_MARGIN: f32 = 20.0;
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

/// A deliberately tiny bitmap alphabet for the low-resolution HUD. Macroquad's
/// antialiased default font loses stems when it is first rasterized at 600x450
/// and then enlarged; these glyphs are composed only of whole framebuffer
/// pixels, so they stay crisp at every integer presentation scale.
fn pixel_glyph(character: char) -> [u8; 7] {
    match character.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '>' => [
            0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000,
        ],
        '!' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        _ => [0; 7],
    }
}

fn pixel_text_metrics(text: &str, size: f32) -> (f32, f32, f32) {
    let requested_height = size * hud_scale();
    let pixel = (requested_height / 7.0).round().max(1.0);
    let character_pitch = pixel * 6.0;
    let width = if text.is_empty() {
        0.0
    } else {
        text.chars().count() as f32 * character_pitch - pixel
    };
    (width, pixel * 7.0, pixel)
}

fn draw_pixel_text(text: &str, x: f32, baseline_y: f32, size: f32, color: Color) {
    let (_, height, pixel) = pixel_text_metrics(text, size);
    let origin_x = x.round();
    let origin_y = (baseline_y - height).round();

    for (character_index, character) in text.chars().enumerate() {
        let glyph = pixel_glyph(character);
        let glyph_x = origin_x + character_index as f32 * pixel * 6.0;

        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    draw_rectangle(
                        glyph_x + col as f32 * pixel,
                        origin_y + row as f32 * pixel,
                        pixel,
                        pixel,
                        color,
                    );
                }
            }
        }
    }
}

fn draw_rounded_rectangle(rect: Rect, radius: f32, color: Color) {
    let radius = radius.min(rect.w * 0.5).min(rect.h * 0.5);

    // These regions do not overlap. That matters for translucent cards: simply
    // layering circles over rectangles doubles the alpha at every corner.
    draw_rectangle(
        rect.x + radius,
        rect.y,
        rect.w - radius * 2.0,
        rect.h,
        color,
    );
    draw_rectangle(
        rect.x,
        rect.y + radius,
        radius,
        rect.h - radius * 2.0,
        color,
    );
    draw_rectangle(
        rect.x + rect.w - radius,
        rect.y + radius,
        radius,
        rect.h - radius * 2.0,
        color,
    );

    let corners = [
        (vec2(rect.x + radius, rect.y + radius), 180.0),
        (vec2(rect.x + rect.w - radius, rect.y + radius), 270.0),
        (
            vec2(rect.x + rect.w - radius, rect.y + rect.h - radius),
            0.0,
        ),
        (vec2(rect.x + radius, rect.y + rect.h - radius), 90.0),
    ];

    for (center, start_degrees) in corners {
        for segment in 0..8 {
            let a = (start_degrees + segment as f32 * 11.25).to_radians();
            let b = (start_degrees + (segment + 1) as f32 * 11.25).to_radians();
            draw_triangle(
                center,
                center + vec2(a.cos(), a.sin()) * radius,
                center + vec2(b.cos(), b.sin()) * radius,
                color,
            );
        }
    }
}

fn draw_rounded_rectangle_lines(rect: Rect, radius: f32, thickness: f32, color: Color) {
    let radius = radius.min(rect.w * 0.5).min(rect.h * 0.5);
    draw_line(
        rect.x + radius,
        rect.y,
        rect.x + rect.w - radius,
        rect.y,
        thickness,
        color,
    );
    draw_line(
        rect.x + radius,
        rect.y + rect.h,
        rect.x + rect.w - radius,
        rect.y + rect.h,
        thickness,
        color,
    );
    draw_line(
        rect.x,
        rect.y + radius,
        rect.x,
        rect.y + rect.h - radius,
        thickness,
        color,
    );
    draw_line(
        rect.x + rect.w,
        rect.y + radius,
        rect.x + rect.w,
        rect.y + rect.h - radius,
        thickness,
        color,
    );

    draw_arc(
        rect.x + radius,
        rect.y + radius,
        32,
        radius,
        180.0,
        thickness,
        90.0,
        color,
    );
    draw_arc(
        rect.x + rect.w - radius,
        rect.y + radius,
        32,
        radius,
        270.0,
        thickness,
        90.0,
        color,
    );
    draw_arc(
        rect.x + rect.w - radius,
        rect.y + rect.h - radius,
        32,
        radius,
        0.0,
        thickness,
        90.0,
        color,
    );
    draw_arc(
        rect.x + radius,
        rect.y + rect.h - radius,
        32,
        radius,
        90.0,
        thickness,
        90.0,
        color,
    );
}

/// A compact recessed instrument housing inspired by the hardware-like HUDs of
/// mid-90s shooters. The frame is neutral; amber is reserved for live state and
/// selection elsewhere in the interface.
fn draw_instrument_panel(rect: Rect) {
    let scale = hud_scale();
    let pixel = scale.max(1.0);
    let shadow = (3.0 * scale).round().max(1.0);
    let frame = (4.0 * scale).round().max(2.0);

    draw_rectangle(
        rect.x + shadow,
        rect.y + shadow,
        rect.w,
        rect.h,
        color_u8!(3, 3, 2, 210),
    );
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, COLOR_PANEL_FRAME);
    draw_rectangle(
        rect.x + frame,
        rect.y + frame,
        rect.w - frame * 2.0,
        rect.h - frame * 2.0,
        COLOR_PANEL,
    );
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        pixel,
        color_u8!(10, 9, 7, 255),
    );

    draw_line(
        rect.x + pixel,
        rect.y + pixel,
        rect.x + rect.w - pixel,
        rect.y + pixel,
        pixel,
        COLOR_PANEL_LIGHT,
    );
    draw_line(
        rect.x + pixel,
        rect.y + pixel,
        rect.x + pixel,
        rect.y + rect.h - pixel,
        pixel,
        COLOR_PANEL_BORDER,
    );
    draw_line(
        rect.x + pixel,
        rect.y + rect.h - pixel,
        rect.x + rect.w - pixel,
        rect.y + rect.h - pixel,
        pixel,
        color_u8!(4, 4, 3, 255),
    );
    draw_line(
        rect.x + rect.w - pixel,
        rect.y + pixel,
        rect.x + rect.w - pixel,
        rect.y + rect.h - pixel,
        pixel,
        color_u8!(4, 4, 3, 255),
    );
}

fn hash01(seed: f32) -> f32 {
    ((seed * 12.9898).sin() * 43_758.547).fract().abs()
}

/// Dithered bunker wall generated from primitives. At 600x450 the individual
/// noise texels and hard lighting bands upscale like a late-90s framebuffer.
pub fn draw_background() {
    let width = frame_width();
    let height = frame_height();
    let flicker = (get_time() as f32 * 8.0).floor();

    for band in 0..24 {
        let t = band as f32 / 23.0;
        let y = height * t;
        draw_rectangle(
            0.0,
            y,
            width,
            (height / 23.0) + 1.0,
            mix_color(COLOR_BUNKER, COLOR_VOID, t),
        );
    }

    // Concrete courses and offset seams establish scale behind the cabinet.
    let row_count = (height / 20.0).ceil() as usize;
    for row in 0..row_count {
        let y = row as f32 * 20.0;
        draw_line(0.0, y, width, y, 1.0, color_u8!(8, 7, 6, 125));
        let offset = if row % 2 == 0 { 0.0 } else { 31.0 };
        let mut x = offset;
        while x < width {
            draw_line(x, y, x, y + 20.0, 1.0, color_u8!(10, 9, 7, 95));
            x += 62.0;
        }
    }

    // A stepped, dirty amber light pool sits behind the playfield.
    for layer in (0..7).rev() {
        let spread = layer as f32 * 8.0;
        draw_rectangle_lines(
            width * 0.5 - 92.0 - spread,
            34.0 - spread * 0.3,
            184.0 + spread * 2.0,
            292.0 + spread * 0.6,
            2.0,
            Color::new(0.55, 0.29, 0.08, 0.018 + layer as f32 * 0.006),
        );
    }

    for index in 0..310 {
        let seed = index as f32 + 1.0;
        let x = (hash01(seed) * width).floor();
        let y = (hash01(seed + 9.0) * height).floor();
        let hot = hash01(seed + flicker) > 0.82;
        let shade = if hot { 50 } else { 25 };
        draw_rectangle(
            x,
            y,
            if index % 7 == 0 { 2.0 } else { 1.0 },
            1.0,
            Color::from_rgba(shade, shade - 5, shade - 12, 150),
        );
    }

    for band in 0..6 {
        let alpha = ((6 - band) * 14) as u8;
        let inset = band as f32 * 5.0;
        draw_rectangle_lines(
            inset,
            inset,
            width - inset * 2.0,
            height - inset * 2.0,
            6.0,
            Color::from_rgba(2, 2, 1, alpha),
        );
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
    let score_width = (HUD_SCORE_WIDTH * scale).round();
    let score_height = (HUD_SCORE_HEIGHT * scale).round();
    let score_x = (well.x + well.w * 0.5 - score_width * 0.5).round();
    let safe_margin = (HUD_SAFE_MARGIN * scale).round();

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
        score: Rect::new(score_x, safe_margin - 2.0, score_width, score_height),
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

fn draw_panel_header(rect: Rect, label: &str) {
    let scale = hud_scale();
    let inset = (4.0 * scale).round().max(2.0);
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
    draw_pixel_text(label, left, baseline, LABEL_TEXT_SIZE, COLOR_TEXT_MUTED);
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

fn draw_well_aura() {
    let scale = hud_scale();
    let well = well_screen_rect();

    for layer in (1..=5).rev() {
        let spread = layer as f32 * 5.0 * scale;
        draw_rectangle_lines(
            well.x - spread,
            well.y - spread,
            well.w + spread * 2.0,
            well.h + spread * 2.0,
            2.0 * scale,
            Color::new(
                COLOR_AMBER.r,
                COLOR_AMBER.g,
                COLOR_AMBER.b,
                0.012 * layer as f32,
            ),
        );
    }
}

fn draw_game_chrome() {
    draw_well_aura();
    draw_instrument_panel(hold_card_rect());
    draw_instrument_panel(next_card_rect());
    draw_instrument_panel(stats_card_rect());
    draw_instrument_panel(controls_card_rect());
}

/// Render the empty well on its own, as a backdrop for the main menu screen.
///
/// Leaves the default 2D camera active so the caller can draw menu text
/// immediately afterwards.
pub fn draw_backdrop(surface: &RenderSurface) {
    draw_well_aura();
    set_camera(&surface.camera_3d(Vec2::ZERO));
    draw_well(&surface.block_textures);
    surface.restore_2d();
}

/// Draw the score as the visual crown of the playfield.
fn draw_score(score: usize) {
    let text = &score.to_formatted_string(&Locale::en);
    let rect = hud_layout().score;
    let center_x = rect.x + rect.w * 0.5;
    let scale = hud_scale();
    let label_baseline = rect.y + (28.0 * scale);
    let score_baseline = rect.y + (68.0 * scale);
    let shadow_offset = 2.0 * scale;

    draw_text_centered_at("SCORE", center_x, label_baseline, 18.0, COLOR_TEXT_MUTED);

    draw_text_centered_at(
        text,
        center_x + shadow_offset,
        score_baseline + shadow_offset,
        SCORE_TEXT_SIZE,
        color_u8!(7, 6, 4, 235),
    );

    draw_text_centered_at(text, center_x, score_baseline, SCORE_TEXT_SIZE, COLOR_AMBER);
}

/// Draw the captions for the hold slot and the next queue.
///
/// These live in the 2D pass rather than beside the cubes they label: `draw_text`
/// emits screen-space geometry, so calling it while the 3D camera is active
/// would push the glyphs through the perspective matrix and off the frame.
fn draw_panel_labels(has_held_piece: bool) {
    let scale = hud_scale();
    draw_panel_header(hold_card_rect(), "HOLD");
    draw_panel_header(next_card_rect(), "NEXT");
    draw_panel_header(controls_card_rect(), "CONTROLS");
    draw_panel_header(stats_card_rect(), "STATUS");
    draw_next_slot_guides();

    if !has_held_piece {
        let center = world_to_screen(hold_piece_anchor());
        draw_text_centered_at(
            "EMPTY",
            center.x,
            center.y + (3.0 * scale),
            16.0,
            color_u8!(118, 105, 76, 150),
        );
    }
}

/// Draw the level and cleared-line counts beneath the next queue.
fn draw_level_and_rows_cleared(level: usize, rows_cleared: usize) {
    let scale = hud_scale();
    let rect = stats_card_rect();
    let mid_x = rect.x + rect.w * 0.5;

    for (label, value, center_x) in [
        ("LEVEL", level, rect.x + rect.w * 0.25),
        ("LINES", rows_cleared, rect.x + rect.w * 0.75),
    ] {
        draw_text_centered_at(
            label,
            center_x,
            rect.y + (62.0 * scale),
            STATS_TEXT_SIZE,
            COLOR_TEXT_MUTED,
        );
        draw_text_centered_at(
            &format!("{:02}", value),
            center_x,
            rect.y + (112.0 * scale),
            26.0,
            COLOR_TEXT,
        );
    }

    draw_line(
        mid_x,
        rect.y + (48.0 * scale),
        mid_x,
        rect.y + (128.0 * scale),
        1.0,
        color_u8!(72, 68, 57, 180),
    );

    let bar = Rect::new(
        rect.x + (16.0 * scale),
        rect.y + rect.h - (18.0 * scale),
        rect.w - (32.0 * scale),
        4.0 * scale,
    );
    let completed = rows_cleared % 10;
    let gap = 1.0;
    let segment_width = ((bar.w - gap * 9.0) / 10.0).floor();
    for segment in 0..10 {
        let x = bar.x + segment as f32 * (segment_width + gap);
        draw_rectangle(
            x,
            bar.y,
            segment_width,
            bar.h,
            if segment < completed {
                COLOR_AMBER
            } else {
                color_u8!(58, 55, 47, 255)
            },
        );
    }
}

fn draw_keycap(label: &str, x: f32, y: f32, width: f32) {
    let scale = hud_scale();
    let rect = Rect::new(x, y, width * scale, 22.0 * scale);
    draw_rectangle(
        rect.x + (2.0 * scale),
        rect.y + (2.0 * scale),
        rect.w,
        rect.h,
        color_u8!(6, 6, 5, 255),
    );
    draw_rounded_rectangle(rect, 0.0, color_u8!(47, 45, 38, 255));
    draw_rounded_rectangle_lines(rect, 0.0, 1.0, color_u8!(92, 86, 70, 255));
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
fn draw_piece_previews(piece_previews: [Piece; 3], textures: &BlockTextures) {
    for (offset, piece) in piece_previews.iter().enumerate() {
        piece.draw(PiecePreviewArgs {
            center: next_piece_anchor(offset),
            scale: PREVIEW_SCALE,
            textures: textures.clone(),
        });
    }
}

/// Draw the held piece into the left side panel, if one is held.
fn draw_held_piece(held_piece: Option<Piece>, textures: &BlockTextures) {
    if let Some(piece) = held_piece {
        piece.draw(PiecePreviewArgs {
            center: hold_piece_anchor(),
            scale: PREVIEW_SCALE,
            textures: textures.clone(),
        });
    }
}

fn line_clear_banner_layout(
    well: Rect,
    center: Vec2,
    text: &str,
    text_size: f32,
    lift: f32,
) -> (Rect, f32) {
    let (_, text_height, _) = pixel_text_metrics(text, text_size);
    let vertical_padding = (6.0 * hud_scale()).round().max(3.0);
    let banner_height = text_height + vertical_padding * 2.0;
    let banner = Rect::new(
        well.x + 12.0,
        (center.y - lift - banner_height * 0.5).round(),
        well.w - 24.0,
        banner_height,
    );
    let text_baseline = banner.y + vertical_padding + text_height;

    (banner, text_baseline)
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

    let (clear_count, clear_remaining) = game_state.get_clear_effect();
    if clear_remaining > 0.0 && clear_count > 0 {
        let elapsed = 1.0 - clear_remaining;
        let fade = (clear_remaining * 2.2).min(1.0);
        let sweep_y = (well.y + (well.h * elapsed)).floor();
        let center = vec2(well.x + well.w * 0.5, well.y + well.h * 0.52);

        draw_rectangle(
            well.x,
            well.y,
            well.w,
            well.h,
            Color::new(0.72, 0.39, 0.10, clear_remaining * 0.11),
        );
        draw_rectangle(
            well.x,
            sweep_y,
            well.w,
            3.0,
            Color::new(COLOR_AMBER.r, COLOR_AMBER.g, COLOR_AMBER.b, fade),
        );

        for index in 0..42 {
            let seed = index as f32 + clear_count as f32 * 31.0;
            let direction = vec2(hash01(seed) - 0.5, hash01(seed + 4.0) - 0.62);
            let speed = 55.0 + hash01(seed + 11.0) * 115.0;
            let point = center + direction * speed * elapsed + vec2(0.0, elapsed * elapsed * 64.0);
            let size = if index % 5 == 0 {
                3.0
            } else {
                1.0 + (index % 2) as f32
            };
            let color = if index % 3 == 0 {
                COLOR_TEXT
            } else {
                COLOR_AMBER
            };
            draw_rectangle(
                point.x.floor(),
                point.y.floor(),
                size,
                size,
                Color::new(color.r, color.g, color.b, fade),
            );
        }

        let callout = match clear_count {
            1 => "LINE CLEAR",
            2 => "DOUBLE CLEAR",
            3 => "TRIPLE CLEAR",
            _ => "BLOXIDE!",
        };
        let lift = (elapsed * 18.0 * scale).floor();
        let callout_size = 24.0 + (clear_count as f32 * 2.0);
        let (banner, callout_baseline) =
            line_clear_banner_layout(well, center, callout, callout_size, lift);
        draw_rectangle(
            banner.x,
            banner.y,
            banner.w,
            banner.h,
            Color::new(0.04, 0.035, 0.025, fade * 0.92),
        );
        draw_rectangle_lines(
            banner.x,
            banner.y,
            banner.w,
            banner.h,
            1.0,
            Color::new(COLOR_AMBER.r, COLOR_AMBER.g, COLOR_AMBER.b, fade),
        );
        draw_text_centered_at(
            callout,
            center.x,
            callout_baseline,
            callout_size,
            Color::new(COLOR_TEXT.r, COLOR_TEXT.g, COLOR_TEXT.b, fade),
        );
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
        draw_game_chrome();
        let impact_shake = self.get_impact_effect().powi(2) * 0.08;
        let (clear_count, clear_remaining) = self.get_clear_effect();
        let clear_shake = if clear_remaining > 0.0 {
            clear_remaining.powi(2) * 0.05 * (clear_count as f32)
        } else {
            0.0
        };
        let shake_amount = (impact_shake + clear_shake).min(0.25);
        let shake = vec2(
            (get_time() as f32 * 91.0).sin() * shake_amount,
            (get_time() as f32 * 73.0).cos() * shake_amount,
        );
        set_camera(&args.camera_3d(shake));

        draw_well(&args.block_textures);
        self.get_grid_locked().draw(GridDrawArgs {
            style: BlockStyle::Solid,
            textures: args.block_textures.clone(),
        });
        self.get_grid_active().draw(GridDrawArgs {
            style: BlockStyle::Solid,
            textures: args.block_textures.clone(),
        });
        self.get_grid_ghost().draw(GridDrawArgs {
            style: BlockStyle::Ghost,
            textures: args.block_textures.clone(),
        });
        draw_piece_previews(self.get_piece_previews(), &args.block_textures);
        draw_held_piece(self.get_held_piece(), &args.block_textures);
        draw_shrapnel(self.get_shrapnel(), &args.block_textures);

        args.restore_2d();

        draw_score(self.get_score());
        draw_panel_labels(self.get_held_piece().is_some());
        draw_level_and_rows_cleared(self.get_level(), self.get_rows_cleared());
        draw_controls();
        draw_game_effects(self, shake);
    }
}

pub struct GridDrawArgs {
    style: BlockStyle,
    textures: BlockTextures,
}

impl Drawable for Grid {
    type Args = GridDrawArgs;

    fn draw(&self, args: GridDrawArgs) {
        for row_id in FIRST_VISIBLE_ROW_ID..GRID_COUNT_ROWS {
            for col_id in 0..GRID_COUNT_COLS {
                if let Some(block) = self.get_cell(row_id, col_id) {
                    block.draw(BlockArgs {
                        center: cell_center(row_id - FIRST_VISIBLE_ROW_ID, col_id),
                        style: args.style,
                        textures: args.textures.clone(),
                    });
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
    textures: BlockTextures,
}

impl Drawable for Piece {
    type Args = PiecePreviewArgs;

    fn draw(&self, args: PiecePreviewArgs) {
        let PiecePreviewArgs {
            center,
            scale,
            textures,
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

                    draw_block_cube_scaled(position, block.color, scale, &textures);
                }
            }
        }
    }
}

/// Placement and appearance of a single block inside the well.
pub struct BlockArgs {
    center: Vec3,
    style: BlockStyle,
    textures: BlockTextures,
}

impl Drawable for Block {
    type Args = BlockArgs;

    fn draw(&self, args: BlockArgs) {
        let BlockArgs {
            center,
            style,
            textures,
        } = args;

        match style {
            BlockStyle::Solid => draw_block_cube(center, self.color, &textures),
            BlockStyle::Ghost => draw_block_outline(center, self.color),
        }
    }
}

impl<'a> Drawable for Menu<'a> {
    type Args = ();

    fn draw(&self, _args: ()) {
        if !self.is_visible {
            return;
        }

        let scale = hud_scale();
        let well = well_screen_rect();
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

        draw_instrument_panel(panel);

        let title_baseline = panel_y + ((MENU_PADDING_Y + MENU_TITLE_TEXT_SIZE) * scale);

        draw_text_centered_at(
            &title,
            center_x + (3.0 * scale),
            title_baseline + (3.0 * scale),
            if is_main { MENU_TITLE_TEXT_SIZE } else { 34.0 },
            color_u8!(3, 3, 2, 220),
        );
        draw_text_centered_at(
            &title,
            center_x,
            title_baseline,
            if is_main { MENU_TITLE_TEXT_SIZE } else { 34.0 },
            COLOR_TEXT,
        );
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
        for (index, item) in self.items.iter().enumerate() {
            let row = snap_rect(Rect::new(
                panel_x + (24.0 * scale),
                items_top + (index as f32 * item_height),
                panel_width - (48.0 * scale),
                item_height - (7.0 * scale),
            ));
            let active = self.active_index == index;

            if active {
                draw_pixel_text(
                    ">",
                    row.x + (3.0 * scale),
                    row.y + (28.0 * scale),
                    MENU_ITEM_TEXT_SIZE,
                    COLOR_AMBER,
                );
            }

            draw_pixel_text(
                item.label,
                row.x + (22.0 * scale),
                row.y + (28.0 * scale),
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
    }
}

impl Drawable for HighScoreManager {
    type Args = ();

    fn draw(&self, _args: ()) {
        let label = "PERSONAL BEST";
        let score = self.get_high_score().to_formatted_string(&Locale::en);
        let rect = snap_rect(Rect::new(frame_width() - 112.0, 8.0, 102.0, 34.0));
        draw_instrument_panel(rect);
        draw_pixel_text(label, rect.x + 6.0, rect.y + 12.0, 16.0, COLOR_TEXT_MUTED);
        draw_text_right_at(
            &score,
            rect.x + rect.w - 6.0,
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

    #[test]
    fn line_clear_banner_contains_the_full_bitmap_text_height() {
        let well = well_screen_rect();
        let center = vec2(well.x + well.w * 0.5, well.y + well.h * 0.52);

        for (text, size) in [
            ("LINE CLEAR", 26.0),
            ("DOUBLE CLEAR", 28.0),
            ("TRIPLE CLEAR", 30.0),
            ("BLOXIDE!", 32.0),
        ] {
            let (_, text_height, _) = pixel_text_metrics(text, size);
            let (banner, baseline) = line_clear_banner_layout(well, center, text, size, 0.0);
            let text_top = baseline - text_height;
            let top_padding = text_top - banner.y;
            let bottom_padding = banner.y + banner.h - baseline;

            assert!(top_padding >= 3.0);
            assert!(bottom_padding >= 3.0);
        }
    }
}
