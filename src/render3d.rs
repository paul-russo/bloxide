//! Low-level 3D drawing for the playfield: camera framing, grid-to-world
//! coordinate mapping, shaded cubes, and the well that contains them.
//!
//! macroquad's vertex format carries only position, UV and colour — there are no
//! normals, so a real lighting shader is not available. Instead every cube is
//! drawn as six independent quads, each tinted by a fixed per-face brightness.
//! That is enough to make a stack of same-coloured blocks read as distinct solid
//! volumes, which is the whole point of moving to 3D.

use macroquad::prelude::*;

use crate::grid::{GRID_COUNT_COLS, VISIBLE_GRID_COUNT_ROWS};

/// The entire game is rendered to this deliberately tiny framebuffer before
/// nearest-neighbour integer upscaling. At the default 1200x900 window this is a
/// clean 2x scale (4x on a Retina framebuffer), giving every edge the same
/// software-rendered pixel density.
pub const RENDER_WIDTH: u32 = 600;
pub const RENDER_HEIGHT: u32 = 450;

/// Width of the playable well in world units. One grid cell is one unit, so a
/// grid coordinate becomes a world coordinate by a simple offset.
pub const WELL_WIDTH: f32 = GRID_COUNT_COLS as f32;

/// Height of the playable well in world units, covering only the visible rows.
/// The two hidden spawn rows above the well are intentionally outside the frame.
pub const WELL_HEIGHT: f32 = VISIBLE_GRID_COUNT_ROWS as f32;

/// Depth of the well cavity. Blocks are one unit deep and centred on `z == 0`,
/// so they exactly fill this span and the back wall sits flush behind them.
pub const WELL_DEPTH: f32 = 1.0;

/// One world-space grid cell maps to exactly this many pixels on the internal
/// render target. Keeping this integral is what makes every column and row land
/// on the same pixel phase before nearest-neighbour upscaling.
const CELL_PIXEL_PITCH: f32 = 17.0;

/// Each front face is exactly 16x16 internal pixels, leaving a consistent
/// one-pixel seam between neighbouring cells.
pub const BLOCK_INSET: f32 = (CELL_PIXEL_PITCH - 1.0) / CELL_PIXEL_PITCH;

const BLOCK_FACE_Z_BIAS: f32 = 0.008;

/// Cosine of the pitch formed by the original camera position and target. The
/// orthographic view height compensates for that pitch, making one vertical
/// world unit cover exactly [`CELL_PIXEL_PITCH`] framebuffer pixels.
const CAMERA_PITCH_COSINE: f32 = 0.996_546;
const CAMERA_VIEW_HEIGHT: f32 = RENDER_HEIGHT as f32 * CAMERA_PITCH_COSINE / CELL_PIXEL_PITCH;

/// A tiny explicit aspect correction independently locks the horizontal pitch
/// to the same 17 pixels without changing the camera's position or direction.
const CAMERA_ASPECT: f32 = RENDER_WIDTH as f32 / (CELL_PIXEL_PITCH * CAMERA_VIEW_HEIGHT);

/// The camera sits in front of the well and a little above its centre, tilted
/// down just enough to reveal the top faces of stacked blocks and the well
/// floor.
///
/// There is deliberately no yaw. Keeping the view left/right symmetric means a
/// column looks the same wherever it sits on the board, so the player can still
/// judge horizontal alignment of a falling piece at a glance.
const CAMERA_POSITION: Vec3 = Vec3::new(0.0, 2.6, 33.0);

/// Look slightly below the well's centre so the downward tilt is shared across
/// the board rather than concentrated at the bottom.
///
/// The position and target are offset from the well's centre by the same amount,
/// which slides the well down the frame without steepening the tilt. That is
/// what balances the margin above the well against the one below it.
const CAMERA_TARGET: Vec3 = Vec3::new(0.0, -0.15, 0.0);

/// Ghost outlines are drawn brighter than the piece they preview, so the thin
/// wireframe stays readable against both the dark well and the stack behind it.
const GHOST_OUTLINE_SHADE: f32 = 1.28;

/// Per-face brightness multipliers, simulating a light source above, in front of
/// and slightly to the left of the well. Faces facing away from it are dimmed
/// rather than blackened so that a block's hue stays recognisable on every side.
const SHADE_TOP: f32 = 1.32;
const SHADE_FRONT: f32 = 1.0;
const SHADE_LEFT: f32 = 0.82;
const SHADE_RIGHT: f32 = 0.6;
const SHADE_BACK: f32 = 0.5;
const SHADE_BOTTOM: f32 = 0.42;

/// Recessed back wall of the well. Kept near-black so lit blocks pop against it.
const WELL_BACK_COLOR_TOP: Color = color_u8!(25, 25, 21, 255);
const WELL_BACK_COLOR_BOTTOM: Color = color_u8!(8, 9, 8, 255);

/// The cabinet face and sloped throat around the recessed playfield. These stay
/// neutral so the coloured blocks and semantic amber HUD accents remain the
/// strongest colours on screen.
const BEZEL_FRAME_COLOR: Color = color_u8!(54, 52, 45, 255);
const BEZEL_TEXTURE_COLOR: Color = color_u8!(91, 87, 75, 255);
const BEZEL_SIDE_COLOR: Color = color_u8!(42, 40, 34, 255);
const BEZEL_BOTTOM_COLOR: Color = color_u8!(63, 59, 49, 255);
const BEZEL_EDGE_COLOR: Color = color_u8!(137, 126, 99, 185);
const BEZEL_SEAM_COLOR: Color = color_u8!(13, 12, 10, 230);

/// Faint cell guides on the back wall. These are a gameplay aid as much as
/// decoration: with the 3D camera applied, they give the eye a reference for
/// which column a falling piece is over.
const WELL_GUIDE_COLOR: Color = color_u8!(166, 151, 112, 18);
const WELL_GUIDE_MAJOR_COLOR: Color = color_u8!(210, 172, 96, 38);

/// The bezel is built on simple fractional world units, while the visible
/// fascia is exactly one unit wide. Its 64x64 material can therefore repeat at
/// one tile per world unit without stretching.
const BEZEL_FASCIA_WIDTH: f32 = 1.0;
const BEZEL_BEVEL_WIDTH: f32 = 0.5;
const BEZEL_BOX_DEPTH: f32 = 0.18;
const BEZEL_TILE_SIZE: f32 = 1.0;
const BEZEL_FRONT_Z: f32 = 1.05;
const BEZEL_THROAT_Z: f32 = (WELL_DEPTH * 0.5) + 0.02;

/// Nudge used to lift the back-wall guides off the wall itself. Without it the
/// two coplanar surfaces z-fight and the guides flicker as the scene redraws.
const GUIDE_Z_BIAS: f32 = 0.02;

/// `z` of the well's back wall, and of the guides drawn just in front of it.
const BACK_WALL_Z: f32 = -WELL_DEPTH / 2.0;

/// Neutral grayscale block materials. The original generated artwork is kept in
/// `assets/source`; these embedded 64x64 copies were reduced with point sampling
/// and are sampled with nearest filtering at runtime.
#[derive(Clone)]
pub struct BlockTextures {
    armor: Texture2D,
    vent: Texture2D,
    bezel: Texture2D,
}

impl BlockTextures {
    pub fn new() -> Self {
        let armor = Texture2D::from_file_with_format(
            include_bytes!("../assets/textures/block-armor-64.png"),
            None,
        );
        let vent = Texture2D::from_file_with_format(
            include_bytes!("../assets/textures/block-vent-64.png"),
            None,
        );
        let bezel = Texture2D::from_file_with_format(
            include_bytes!("../assets/textures/rail-gunmetal-64.png"),
            None,
        );
        armor.set_filter(FilterMode::Nearest);
        vent.set_filter(FilterMode::Nearest);
        bezel.set_filter(FilterMode::Nearest);

        Self { armor, vent, bezel }
    }

    /// Choose a stable material from the piece color. Position-based variation
    /// would make an active piece visibly swap textures whenever it moves.
    fn for_color(&self, color: Color) -> &Texture2D {
        let r = (color.r * 255.0).round() as u32;
        let g = (color.g * 255.0).round() as u32;
        let b = (color.b * 255.0).round() as u32;
        let signature = r * 3 + g * 5 + b * 7;

        if signature % 5 >= 3 {
            &self.vent
        } else {
            &self.armor
        }
    }
}

/// Camera used for every 3D element of the game.
///
/// Rebuilt per call rather than cached because it is cheap and may target either
/// the low-resolution framebuffer or the screen-space projection helper.
pub fn well_camera(render_target: Option<RenderTarget>, shake: Vec2) -> Camera3D {
    let camera_offset = Vec3::new(shake.x, shake.y, 0.0);
    Camera3D {
        position: CAMERA_POSITION + camera_offset,
        target: CAMERA_TARGET + camera_offset,
        up: Vec3::Y,
        fovy: CAMERA_VIEW_HEIGHT,
        projection: Projection::Orthographics,
        aspect: Some(CAMERA_ASPECT),
        render_target,
        viewport: None,
        // The whole scene sits roughly 20-45 units from the camera. Keeping the
        // clip range tight gives the low-resolution depth buffer substantially
        // more precision than Macroquad's general-purpose 0.01-10000 default.
        z_near: 1.0,
        z_far: 100.0,
    }
}

/// Context-free copy of the well camera matrix used for HUD projection and
/// projection invariants in tests. Macroquad's `Camera3D::matrix` queries the
/// active window size even when an explicit aspect ratio is supplied, which is
/// unavailable in unit tests.
fn well_view_projection_matrix(shake: Vec2) -> Mat4 {
    let top = CAMERA_VIEW_HEIGHT * 0.5;
    let right = top * CAMERA_ASPECT;
    let camera_offset = Vec3::new(shake.x, shake.y, 0.0);

    Mat4::orthographic_rh_gl(-right, right, -top, top, 1.0, 100.0)
        * Mat4::look_at_rh(
            CAMERA_POSITION + camera_offset,
            CAMERA_TARGET + camera_offset,
            Vec3::Y,
        )
}

/// Project a world-space point into screen pixels using [`well_camera`].
///
/// This lets 2D HUD labels be anchored to features of the 3D scene (the well
/// rim, the side panels) instead of duplicating the camera's framing maths as
/// hand-tuned screen offsets that silently drift whenever the camera moves.
pub fn world_to_screen(point: Vec3) -> Vec2 {
    world_to_screen_with_shake(point, Vec2::ZERO)
}

/// Project a world-space point through the same shaken camera used to draw the
/// 3D well. Screen-space effects anchored to blocks must use this variant or
/// they visibly separate from the stack during an impact pulse.
pub fn world_to_screen_with_shake(point: Vec3, shake: Vec2) -> Vec2 {
    let clip = well_view_projection_matrix(shake) * point.extend(1.0);

    // Perspective divide to normalised device coordinates, then map to pixels.
    // NDC y points up while screen y points down, hence the flip.
    let ndc = clip.truncate() / clip.w;

    Vec2::new(
        (ndc.x + 1.0) * 0.5 * RENDER_WIDTH as f32,
        (1.0 - ndc.y) * 0.5 * RENDER_HEIGHT as f32,
    )
}

/// World-space centre of a visible grid cell.
///
/// `visible_row` is relative to the top of the *visible* playfield, so callers
/// must already have subtracted `FIRST_VISIBLE_ROW_ID`. Grid rows count downward
/// while world `y` counts upward, so the row term is negated here.
pub fn cell_center(visible_row: usize, col: usize) -> Vec3 {
    Vec3::new(
        col as f32 - (WELL_WIDTH / 2.0) + 0.5,
        (WELL_HEIGHT / 2.0) - visible_row as f32 - 0.5,
        0.0,
    )
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + ((b.r - a.r) * t),
        a.g + ((b.g - a.g) * t),
        a.b + ((b.b - a.b) * t),
        a.a + ((b.a - a.a) * t),
    )
}

/// Scale a colour's RGB by `shade` and override its alpha, clamping to the valid
/// range so that highlight multipliers above 1.0 saturate instead of wrapping.
fn shaded(color: Color, shade: f32, alpha: f32) -> Color {
    Color {
        r: (color.r * shade).clamp(0.0, 1.0),
        g: (color.g * shade).clamp(0.0, 1.0),
        b: (color.b * shade).clamp(0.0, 1.0),
        a: alpha,
    }
}

/// Draw an axis-aligned box as six per-face shaded quads.
///
/// Faces are not back-face culled, so all six are submitted and the depth buffer
/// decides which are visible. That keeps this correct from any camera angle at
/// the cost of a few hidden quads per block, which is irrelevant at the couple
/// of hundred blocks a playfield can hold.
pub fn draw_shaded_box(center: Vec3, size: Vec3, color: Color, alpha: f32) {
    // Each face is defined by its lowest corner plus two edge vectors.
    let min = center - (size * 0.5);
    let edge_x = Vec3::new(size.x, 0.0, 0.0);
    let edge_y = Vec3::new(0.0, size.y, 0.0);
    let edge_z = Vec3::new(0.0, 0.0, size.z);

    let face = |offset: Vec3, e1: Vec3, e2: Vec3, shade: f32| {
        draw_affine_parallelogram(offset, e1, e2, None, shaded(color, shade, alpha));
    };

    face(min + edge_z, edge_x, edge_y, SHADE_FRONT);
    face(min, edge_x, edge_y, SHADE_BACK);
    face(min + edge_y, edge_x, edge_z, SHADE_TOP);
    face(min, edge_x, edge_z, SHADE_BOTTOM);
    face(min + edge_x, edge_y, edge_z, SHADE_RIGHT);
    face(min, edge_y, edge_z, SHADE_LEFT);
}

/// Cover the front of a box with square material tiles at a fixed world scale.
/// Alternating UV direction breaks up obvious repetition without changing tile
/// density or introducing geometry seams between neighbouring samples.
fn draw_tiled_front_face(center: Vec3, size: Vec3, texture: &Texture2D, tint: Color, phase: usize) {
    let columns = (size.x / BEZEL_TILE_SIZE).round() as usize;
    let rows = (size.y / BEZEL_TILE_SIZE).round() as usize;
    debug_assert!((columns as f32 * BEZEL_TILE_SIZE - size.x).abs() < 0.001);
    debug_assert!((rows as f32 * BEZEL_TILE_SIZE - size.y).abs() < 0.001);

    let min = center - (size * 0.5);
    let z = center.z + size.z * 0.5 + 0.003;

    for row in 0..rows {
        for column in 0..columns {
            let index = row * columns + column + phase;
            let x = min.x + column as f32 * BEZEL_TILE_SIZE;
            let y = min.y + row as f32 * BEZEL_TILE_SIZE;
            let flip_x = index % 2 == 1;
            let flip_y = (index / 2) % 2 == 1;
            let origin = Vec3::new(
                x + if flip_x { BEZEL_TILE_SIZE } else { 0.0 },
                y + if flip_y { BEZEL_TILE_SIZE } else { 0.0 },
                z,
            );
            let edge_x = Vec3::new(
                if flip_x {
                    -BEZEL_TILE_SIZE
                } else {
                    BEZEL_TILE_SIZE
                },
                0.0,
                0.0,
            );
            let edge_y = Vec3::new(
                0.0,
                if flip_y {
                    -BEZEL_TILE_SIZE
                } else {
                    BEZEL_TILE_SIZE
                },
                0.0,
            );

            draw_affine_parallelogram(origin, edge_x, edge_y, Some(texture), tint);
        }
    }
}

fn draw_front_face_outline(center: Vec3, size: Vec3, color: Color) {
    let half = size * 0.5;
    let z = center.z + half.z + 0.006;
    let corners = [
        Vec3::new(center.x - half.x, center.y - half.y, z),
        Vec3::new(center.x + half.x, center.y - half.y, z),
        Vec3::new(center.x + half.x, center.y + half.y, z),
        Vec3::new(center.x - half.x, center.y + half.y, z),
    ];

    for index in 0..4 {
        draw_line_3d(corners[index], corners[(index + 1) % 4], color);
    }
}

const FACE_BACK: usize = 0;
const FACE_FRONT: usize = 1;
const FACE_BOTTOM: usize = 2;
const FACE_TOP: usize = 3;
const FACE_LEFT: usize = 4;
const FACE_RIGHT: usize = 5;

/// Corner indices plus the two faces that meet at each of the cube's 12 edges.
const BLOCK_EDGES: [(usize, usize, usize, usize); 12] = [
    (0, 1, FACE_BACK, FACE_BOTTOM),
    (1, 2, FACE_BACK, FACE_RIGHT),
    (2, 3, FACE_BACK, FACE_TOP),
    (3, 0, FACE_BACK, FACE_LEFT),
    (4, 5, FACE_FRONT, FACE_BOTTOM),
    (5, 6, FACE_FRONT, FACE_RIGHT),
    (6, 7, FACE_FRONT, FACE_TOP),
    (7, 4, FACE_FRONT, FACE_LEFT),
    (0, 4, FACE_BOTTOM, FACE_LEFT),
    (1, 5, FACE_BOTTOM, FACE_RIGHT),
    (2, 6, FACE_TOP, FACE_RIGHT),
    (3, 7, FACE_TOP, FACE_LEFT),
];

/// Determine which of an axis-aligned cube's six faces point toward the camera.
/// The order is back, front, bottom, top, left, right.
fn visible_block_faces(center: Vec3, size: f32) -> [bool; 6] {
    let half = size * 0.5;
    let face_centers = [
        center - Vec3::Z * half,
        center + Vec3::Z * half,
        center - Vec3::Y * half,
        center + Vec3::Y * half,
        center - Vec3::X * half,
        center + Vec3::X * half,
    ];
    let normals = [-Vec3::Z, Vec3::Z, -Vec3::Y, Vec3::Y, -Vec3::X, Vec3::X];

    std::array::from_fn(|index| normals[index].dot(CAMERA_POSITION - face_centers[index]) > 0.0)
}

/// An edge is visible exactly when at least one of its two owning faces faces
/// the camera. Edges whose two owners are rear-facing are never submitted.
fn visible_block_edges(visible_faces: [bool; 6]) -> [bool; 12] {
    std::array::from_fn(|index| {
        let (_, _, adjacent_a, adjacent_b) = BLOCK_EDGES[index];
        visible_faces[adjacent_a] || visible_faces[adjacent_b]
    })
}

/// Draw one block from first principles: submit only camera-facing quads, then
/// outline only edges adjacent to at least one submitted face. Rear-facing
/// geometry never enters the draw list, so it cannot leak through at low depth
/// precision or during integer upscaling.
pub fn draw_block_cube_scaled(center: Vec3, color: Color, scale: f32, textures: &BlockTextures) {
    let size = BLOCK_INSET * scale;
    let min = center - Vec3::splat(size * 0.5);
    let edge_x = Vec3::new(size, 0.0, 0.0);
    let edge_y = Vec3::new(0.0, size, 0.0);
    let edge_z = Vec3::new(0.0, 0.0, size);
    let visible = visible_block_faces(center, size);
    let texture = textures.for_color(color);

    let face = |offset: Vec3, e1: Vec3, e2: Vec3, shade: f32| {
        draw_affine_parallelogram(offset, e1, e2, Some(texture), shaded(color, shade, 1.0));
    };

    if visible[FACE_BACK] {
        face(min, edge_x, edge_y, SHADE_BACK);
    }
    if visible[FACE_FRONT] {
        face(min + edge_z, edge_x, edge_y, SHADE_FRONT);
    }
    if visible[FACE_BOTTOM] {
        face(min, edge_x, edge_z, SHADE_BOTTOM);
    }
    if visible[FACE_TOP] {
        face(min + edge_y, edge_x, edge_z, SHADE_TOP);
    }
    if visible[FACE_LEFT] {
        face(min, edge_y, edge_z, SHADE_LEFT);
    }
    if visible[FACE_RIGHT] {
        face(min + edge_x, edge_y, edge_z, SHADE_RIGHT);
    }

    let corners = [
        min,
        min + edge_x,
        min + edge_x + edge_y,
        min + edge_y,
        min + edge_z,
        min + edge_x + edge_z,
        min + edge_x + edge_y + edge_z,
        min + edge_y + edge_z,
    ];
    let visible_edges = visible_block_edges(visible);
    for (index, &(start, end, adjacent_a, adjacent_b)) in BLOCK_EDGES.iter().enumerate() {
        let visible_count = visible[adjacent_a] as u8 + visible[adjacent_b] as u8;
        if visible_edges[index] {
            // A crease joins two visible faces; a silhouette borders one visible
            // and one culled face. Both are real camera-facing edges, but the
            // crease is kept slightly lighter so it does not cut the cube into a
            // wire cage. Lines remain on their true geometry so the depth buffer
            // can occlude them behind neighbouring blocks.
            let shade = if visible_count == 2 { 0.38 } else { 0.27 };
            draw_line_3d(corners[start], corners[end], shaded(color, shade, 1.0));
        }
    }
}

/// Maximum simultaneous active shrapnel voxels across all clearing rows.
pub const MAX_SHRAPNEL_VOXELS: usize = 320;

/// A 3D tumbling sub-voxel spawned when rows are cleared.
#[derive(Copy, Clone, Debug, Default)]
pub struct ShrapnelVoxel {
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Vec3,
    pub angular_velocity: Vec3,
    pub color: Color,
    pub size: f32,
    pub age: f32,
    pub max_life: f32,
    pub bounce_count: u8,
    pub is_carnage: bool,
    pub active: bool,
}

/// Draw a single tumbling shrapnel voxel with dynamic directional shading,
/// point-sampled texture, and optional hot-metal carnages.
pub fn draw_shrapnel_voxel(voxel: &ShrapnelVoxel, textures: &BlockTextures) {
    if !voxel.active {
        return;
    }

    let progress = (voxel.age / voxel.max_life).clamp(0.0, 1.0);
    let fade = ((1.0 - progress) / 0.15).clamp(0.0, 1.0);
    let scale = voxel.size * (0.25 + 0.75 * fade);
    if scale <= 0.001 {
        return;
    }

    // Four-line clears superheat shrapnel into molten incandescence before
    // slowly cooling back to the block's base material.
    let (is_hot, heat) = if voxel.is_carnage && progress < 0.72 {
        let t = (progress / 0.72).clamp(0.0, 1.0);
        let heat_curve = (1.0 - t).powf(1.35);
        (true, heat_curve)
    } else {
        (false, 0.0)
    };

    let color = if is_hot {
        if heat > 0.60 {
            let t = (heat - 0.60) / 0.40;
            mix_color(
                Color::new(1.0, 0.70, 0.12, 1.0),
                Color::new(1.0, 0.98, 0.90, 1.0),
                t,
            )
        } else if heat > 0.22 {
            let t = (heat - 0.22) / 0.38;
            mix_color(
                Color::new(0.98, 0.18, 0.02, 1.0),
                Color::new(1.0, 0.70, 0.12, 1.0),
                t,
            )
        } else {
            let t = heat / 0.22;
            mix_color(
                voxel.color,
                Color::new(0.98, 0.18, 0.02, 1.0),
                t,
            )
        }
    } else {
        voxel.color
    };

    if is_hot && heat > 0.15 {
        let halo_size = scale * (2.2 + 1.4 * heat);
        let halo_alpha = (heat * 0.26 * fade).clamp(0.0, 0.35);
        let halo_color = Color::new(1.0, 0.58, 0.12, halo_alpha);
        let halo_origin = voxel.position - Vec3::new(halo_size * 0.5, halo_size * 0.5, 0.0);

        draw_affine_parallelogram(
            halo_origin,
            Vec3::new(halo_size, 0.0, 0.0),
            Vec3::new(0.0, halo_size, 0.0),
            None,
            halo_color,
        );
    }

    let rot = Quat::from_euler(
        EulerRot::XYZ,
        voxel.rotation.x,
        voxel.rotation.y,
        voxel.rotation.z,
    );
    let edge_x = rot * Vec3::new(scale, 0.0, 0.0);
    let edge_y = rot * Vec3::new(0.0, scale, 0.0);
    let edge_z = rot * Vec3::new(0.0, 0.0, scale);
    let min = voxel.position - 0.5 * (edge_x + edge_y + edge_z);

    let light_dir = Vec3::new(-0.35, 0.75, 0.55).normalize();
    let texture = textures.for_color(color);

    let face_normals = [
        -rot * Vec3::Z,
        rot * Vec3::Z,
        -rot * Vec3::Y,
        rot * Vec3::Y,
        -rot * Vec3::X,
        rot * Vec3::X,
    ];

    let face_origins = [
        min,
        min + edge_z,
        min,
        min + edge_y,
        min,
        min + edge_x,
    ];

    let face_e1_e2 = [
        (edge_x, edge_y),
        (edge_x, edge_y),
        (edge_x, edge_z),
        (edge_x, edge_z),
        (edge_y, edge_z),
        (edge_y, edge_z),
    ];

    let corners = [
        min,
        min + edge_x,
        min + edge_x + edge_y,
        min + edge_y,
        min + edge_z,
        min + edge_x + edge_z,
        min + edge_x + edge_y + edge_z,
        min + edge_y + edge_z,
    ];

    let mut visible_faces = [false; 6];
    for index in 0..6 {
        let normal = face_normals[index];
        let face_center = face_origins[index] + 0.5 * (face_e1_e2[index].0 + face_e1_e2[index].1);
        let cam_dir = CAMERA_POSITION - face_center;

        if normal.dot(cam_dir) > 0.0 {
            visible_faces[index] = true;
            let ndotl = normal.dot(light_dir).max(0.0);
            let shade = (0.42 + 0.88 * ndotl) * (1.0 - heat * 0.85)
                + (1.08 + 0.22 * ndotl) * (heat * 0.85);
            let texture_opt = if is_hot && heat > 0.42 {
                None
            } else {
                Some(texture)
            };

            draw_affine_parallelogram(
                face_origins[index],
                face_e1_e2[index].0,
                face_e1_e2[index].1,
                texture_opt,
                shaded(color, shade, 1.0),
            );
        }
    }

    let visible_edges = visible_block_edges(visible_faces);
    for (index, &(start, end, adjacent_a, adjacent_b)) in BLOCK_EDGES.iter().enumerate() {
        let visible_count = visible_faces[adjacent_a] as u8 + visible_faces[adjacent_b] as u8;
        if visible_edges[index] {
            let base_shade = if visible_count == 2 { 0.45 } else { 0.28 };
            let edge_color = if is_hot && heat > 0.08 {
                let hot_color = mix_color(
                    Color::new(1.0, 0.65, 0.15, 1.0),
                    Color::new(1.0, 1.0, 0.92, 1.0),
                    heat,
                );
                mix_color(shaded(color, base_shade, 1.0), hot_color, heat)
            } else {
                shaded(color, base_shade, 1.0)
            };

            draw_line_3d(corners[start], corners[end], edge_color);
        }
    }
}

/// Draw all active 3D shrapnel voxels.
pub fn draw_shrapnel(voxels: &[ShrapnelVoxel], textures: &BlockTextures) {
    for voxel in voxels {
        if voxel.active {
            draw_shrapnel_voxel(voxel, textures);
        }
    }
}

/// Draw a single playfield block as an inset shaded cube centred on `center`.
pub fn draw_block_cube(center: Vec3, color: Color, textures: &BlockTextures) {
    draw_block_cube_scaled(center, color, 1.0, textures);
}

/// Draw a block as a wireframe outline, used for the hard-drop landing preview.
///
/// A wireframe is used rather than a translucent cube on purpose: alpha-blended
/// cubes need to be sorted back-to-front against the stack to composite
/// correctly, whereas lines read cleanly at any depth and never hide the blocks
/// the player is aiming at.
pub fn draw_block_outline(center: Vec3, color: Color) {
    let pulse = 0.82 + (((get_time() as f32 * 4.0).sin() + 1.0) * 0.09);
    let half = BLOCK_INSET * 0.5;
    let z = center.z + half + BLOCK_FACE_Z_BIAS;
    let outline = shaded(color, GHOST_OUTLINE_SHADE, pulse);
    let points = [
        Vec3::new(center.x - half, center.y - half, z),
        Vec3::new(center.x + half, center.y - half, z),
        Vec3::new(center.x + half, center.y + half, z),
        Vec3::new(center.x - half, center.y + half, z),
    ];

    for index in 0..4 {
        draw_line_3d(points[index], points[(index + 1) % 4], outline);
    }
}

/// Draw the well: its recessed back wall, the cell guides on that wall, and the
/// frame around its opening.
///
/// Must be drawn before the blocks. The back wall is opaque and would otherwise
/// need to sort behind them, and drawing it first also gives the depth buffer a
/// sane floor for everything that follows.
pub fn draw_well(textures: &BlockTextures) {
    let half_w = WELL_WIDTH / 2.0;
    let half_h = WELL_HEIGHT / 2.0;

    // Build the back wall in strips for a deep navy vertical gradient. This is
    // intentionally subtle: the board should feel illuminated, never striped.
    for row in 0..VISIBLE_GRID_COUNT_ROWS {
        let t = row as f32 / (VISIBLE_GRID_COUNT_ROWS - 1) as f32;
        let color = Color::new(
            WELL_BACK_COLOR_TOP.r + (WELL_BACK_COLOR_BOTTOM.r - WELL_BACK_COLOR_TOP.r) * t,
            WELL_BACK_COLOR_TOP.g + (WELL_BACK_COLOR_BOTTOM.g - WELL_BACK_COLOR_TOP.g) * t,
            WELL_BACK_COLOR_TOP.b + (WELL_BACK_COLOR_BOTTOM.b - WELL_BACK_COLOR_TOP.b) * t,
            1.0,
        );
        let y = half_h - row as f32 - 1.0;

        draw_affine_parallelogram(
            Vec3::new(-half_w, y, BACK_WALL_Z),
            Vec3::new(WELL_WIDTH, 0.0, 0.0),
            Vec3::new(0.0, 1.01, 0.0),
            None,
            color,
        );
    }

    draw_well_guides(half_w, half_h);
    draw_well_bezel(half_w, half_h, textures);
}

/// Draw the faint per-cell grid on the well's back wall.
fn draw_well_guides(half_w: f32, half_h: f32) {
    let z = BACK_WALL_Z + GUIDE_Z_BIAS;

    for col in 1..GRID_COUNT_COLS {
        let x = col as f32 - half_w;
        draw_line_3d(
            Vec3::new(x, -half_h, z),
            Vec3::new(x, half_h, z),
            if col == GRID_COUNT_COLS / 2 {
                WELL_GUIDE_MAJOR_COLOR
            } else {
                WELL_GUIDE_COLOR
            },
        );
    }

    for row in 1..VISIBLE_GRID_COUNT_ROWS {
        let y = half_h - row as f32;
        draw_line_3d(
            Vec3::new(-half_w, y, z),
            Vec3::new(half_w, y, z),
            if row % 5 == 0 {
                WELL_GUIDE_MAJOR_COLOR
            } else {
                WELL_GUIDE_COLOR
            },
        );
    }
}

/// Three continuous solids forming an open-topped cabinet fascia. The bottom
/// and sides meet only at shared edges, so there are no coplanar overlaps or
/// accumulated block-sized alignment errors.
fn bezel_fascia_boxes(half_w: f32, half_h: f32) -> [(Vec3, Vec3); 3] {
    let front_center_z = BEZEL_FRONT_Z - BEZEL_BOX_DEPTH * 0.5;
    let outer_width = WELL_WIDTH + 2.0 * (BEZEL_BEVEL_WIDTH + BEZEL_FASCIA_WIDTH);
    let side_height = WELL_HEIGHT + 2.0 * BEZEL_BEVEL_WIDTH;
    let side_x = half_w + BEZEL_BEVEL_WIDTH + BEZEL_FASCIA_WIDTH * 0.5;
    let cap_y = half_h + BEZEL_BEVEL_WIDTH + BEZEL_FASCIA_WIDTH * 0.5;

    [
        (
            Vec3::new(0.0, -cap_y, front_center_z),
            Vec3::new(outer_width, BEZEL_FASCIA_WIDTH, BEZEL_BOX_DEPTH),
        ),
        (
            Vec3::new(-side_x, 0.0, front_center_z),
            Vec3::new(BEZEL_FASCIA_WIDTH, side_height, BEZEL_BOX_DEPTH),
        ),
        (
            Vec3::new(side_x, 0.0, front_center_z),
            Vec3::new(BEZEL_FASCIA_WIDTH, side_height, BEZEL_BOX_DEPTH),
        ),
    ]
}

/// Draw the sloped sides and floor between the cabinet face and the playfield
/// opening. Leaving the top open is a gameplay cue: pieces visibly enter a
/// chute instead of appearing on a display enclosed behind glass.
fn draw_bezel_throat(half_w: f32, half_h: f32) {
    let depth = BEZEL_FRONT_Z - BEZEL_THROAT_Z;

    draw_affine_parallelogram(
        Vec3::new(-half_w, -half_h, BEZEL_THROAT_Z),
        Vec3::new(0.0, WELL_HEIGHT, 0.0),
        Vec3::new(-BEZEL_BEVEL_WIDTH, 0.0, depth),
        None,
        BEZEL_SIDE_COLOR,
    );
    draw_affine_parallelogram(
        Vec3::new(half_w, -half_h, BEZEL_THROAT_Z),
        Vec3::new(0.0, WELL_HEIGHT, 0.0),
        Vec3::new(BEZEL_BEVEL_WIDTH, 0.0, depth),
        None,
        shaded(BEZEL_SIDE_COLOR, 0.78, 1.0),
    );
    draw_affine_parallelogram(
        Vec3::new(-half_w, -half_h, BEZEL_THROAT_Z),
        Vec3::new(WELL_WIDTH, 0.0, 0.0),
        Vec3::new(0.0, -BEZEL_BEVEL_WIDTH, depth),
        None,
        BEZEL_BOTTOM_COLOR,
    );
    let mut corner_vertices = Vec::with_capacity(12);
    let mut corner_indices = Vec::with_capacity(12);
    let mut push_triangle = |a: Vec3, b: Vec3, c: Vec3, color: Color| {
        let first = corner_vertices.len() as u16;
        corner_vertices.push(Vertex::new2(a, Vec2::ZERO, color));
        corner_vertices.push(Vertex::new2(b, Vec2::ZERO, color));
        corner_vertices.push(Vertex::new2(c, Vec2::ZERO, color));
        corner_indices.extend_from_slice(&[first, first + 1, first + 2]);
    };

    for x_sign in [-1.0, 1.0] {
        let y_sign = -1.0;
        let inner = Vec3::new(x_sign * half_w, y_sign * half_h, BEZEL_THROAT_Z);
        let outer_side = Vec3::new(
            x_sign * (half_w + BEZEL_BEVEL_WIDTH),
            y_sign * half_h,
            BEZEL_FRONT_Z,
        );
        let outer_cap = Vec3::new(
            x_sign * half_w,
            y_sign * (half_h + BEZEL_BEVEL_WIDTH),
            BEZEL_FRONT_Z,
        );
        let outer_corner = Vec3::new(
            x_sign * (half_w + BEZEL_BEVEL_WIDTH),
            y_sign * (half_h + BEZEL_BEVEL_WIDTH),
            BEZEL_FRONT_Z,
        );
        let side_color = if x_sign < 0.0 {
            BEZEL_SIDE_COLOR
        } else {
            shaded(BEZEL_SIDE_COLOR, 0.78, 1.0)
        };

        push_triangle(inner, outer_cap, outer_corner, BEZEL_BOTTOM_COLOR);
        push_triangle(inner, outer_corner, outer_side, side_color);
    }

    draw_mesh(&Mesh {
        vertices: corner_vertices,
        indices: corner_indices,
        texture: None,
    });

    let edge_z = BEZEL_THROAT_Z + 0.004;
    let corners = [
        Vec3::new(-half_w, -half_h, edge_z),
        Vec3::new(half_w, -half_h, edge_z),
        Vec3::new(half_w, half_h, edge_z),
        Vec3::new(-half_w, half_h, edge_z),
    ];
    draw_line_3d(corners[0], corners[1], BEZEL_EDGE_COLOR);
    draw_line_3d(corners[0], corners[3], BEZEL_EDGE_COLOR);
    draw_line_3d(corners[1], corners[2], BEZEL_EDGE_COLOR);
}

/// Draw a textured cabinet face around a sloped, open-topped throat. The depth
/// step gives the well physical structure while the open silhouette preserves
/// the clear visual story of pieces falling down into it.
fn draw_well_bezel(half_w: f32, half_h: f32, textures: &BlockTextures) {
    draw_bezel_throat(half_w, half_h);

    for (index, (center, size)) in bezel_fascia_boxes(half_w, half_h).into_iter().enumerate() {
        draw_shaded_box(center, size, BEZEL_FRAME_COLOR, 1.0);
        draw_cube_wires(center, size, BEZEL_SEAM_COLOR);
        draw_tiled_front_face(center, size, &textures.bezel, BEZEL_TEXTURE_COLOR, index);
        draw_front_face_outline(center, size, BEZEL_SEAM_COLOR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_block_submits_front_and_top_faces() {
        let visible = visible_block_faces(Vec3::ZERO, BLOCK_INSET);

        assert!(visible[FACE_FRONT]);
        assert!(visible[FACE_TOP]);
        assert!(!visible[FACE_BACK]);
        assert!(!visible[FACE_BOTTOM]);
        assert!(!visible[FACE_LEFT]);
        assert!(!visible[FACE_RIGHT]);
    }

    #[test]
    fn centered_block_omits_edges_owned_only_by_hidden_faces() {
        let faces = visible_block_faces(Vec3::ZERO, BLOCK_INSET);
        let edges = visible_block_edges(faces);

        assert_eq!(edges.iter().filter(|&&visible| visible).count(), 7);
        for hidden_edge in [0, 1, 3, 8, 9] {
            assert!(!edges[hidden_edge]);
        }
    }

    #[test]
    fn horizontal_face_visibility_tracks_camera_side() {
        let left_block = visible_block_faces(Vec3::new(-4.0, 0.0, 0.0), BLOCK_INSET);
        let right_block = visible_block_faces(Vec3::new(4.0, 0.0, 0.0), BLOCK_INSET);

        assert!(left_block[FACE_RIGHT]);
        assert!(!left_block[FACE_LEFT]);
        assert!(right_block[FACE_LEFT]);
        assert!(!right_block[FACE_RIGHT]);
    }

    #[test]
    fn vertical_face_visibility_tracks_camera_height() {
        let low_block = visible_block_faces(Vec3::new(0.0, -8.0, 0.0), BLOCK_INSET);
        let high_block = visible_block_faces(Vec3::new(0.0, 8.0, 0.0), BLOCK_INSET);

        assert!(low_block[FACE_TOP]);
        assert!(!low_block[FACE_BOTTOM]);
        assert!(high_block[FACE_BOTTOM]);
        assert!(!high_block[FACE_TOP]);
    }

    #[test]
    fn grid_cells_project_to_exact_integer_pixel_pitch() {
        let origin = world_to_screen(Vec3::ZERO);
        let one_column = world_to_screen(Vec3::X);
        let one_row = world_to_screen(-Vec3::Y);

        assert!(((one_column.x - origin.x) - CELL_PIXEL_PITCH).abs() < 0.001);
        assert!(((one_row.y - origin.y) - CELL_PIXEL_PITCH).abs() < 0.001);
    }

    #[test]
    fn block_front_faces_leave_one_pixel_between_cells() {
        let origin = world_to_screen(Vec3::ZERO);
        let block_width = world_to_screen(Vec3::X * BLOCK_INSET);
        let block_height = world_to_screen(-Vec3::Y * BLOCK_INSET);
        let expected_face_size = CELL_PIXEL_PITCH - 1.0;

        assert!(((block_width.x - origin.x) - expected_face_size).abs() < 0.001);
        assert!(((block_height.y - origin.y) - expected_face_size).abs() < 0.001);
    }

    #[test]
    fn bezel_fascia_forms_a_gapless_open_topped_chute() {
        let [bottom, left, right] = bezel_fascia_boxes(WELL_WIDTH / 2.0, WELL_HEIGHT / 2.0);
        let (bottom_min, bottom_max) = (bottom.0 - bottom.1 * 0.5, bottom.0 + bottom.1 * 0.5);
        let (left_min, left_max) = (left.0 - left.1 * 0.5, left.0 + left.1 * 0.5);
        let (right_min, right_max) = (right.0 - right.1 * 0.5, right.0 + right.1 * 0.5);

        const EPSILON: f32 = 0.000_001;
        assert!((bottom_max.y - left_min.y).abs() < EPSILON);
        assert!((bottom_max.y - right_min.y).abs() < EPSILON);
        assert!((bottom_min.x - left_min.x).abs() < EPSILON);
        assert!((bottom_max.x - right_max.x).abs() < EPSILON);
        assert!((left_max.y - right_max.y).abs() < EPSILON);
        assert!((left_max.y - (WELL_HEIGHT * 0.5 + BEZEL_BEVEL_WIDTH)).abs() < EPSILON);
    }

    #[test]
    fn bezel_fascia_uses_only_unstretched_square_tiles() {
        for (_, size) in bezel_fascia_boxes(WELL_WIDTH / 2.0, WELL_HEIGHT / 2.0) {
            assert!((size.x / BEZEL_TILE_SIZE).fract().abs() < 0.000_001);
            assert!((size.y / BEZEL_TILE_SIZE).fract().abs() < 0.000_001);
        }

        assert!(BEZEL_THROAT_Z > BLOCK_INSET * 0.5);
        assert!(BEZEL_FRONT_Z > BEZEL_THROAT_Z);
    }

    #[test]
    fn shrapnel_voxel_defaults_to_inactive() {
        let voxel = ShrapnelVoxel::default();
        assert!(!voxel.active);
        assert_eq!(voxel.age, 0.0);
    }

    #[test]
    fn carnage_voxel_heat_curve_spans_majority_of_lifetime() {
        let mut voxel = ShrapnelVoxel {
            active: true,
            is_carnage: true,
            size: 0.4,
            age: 0.1,
            max_life: 2.0,
            color: Color::new(0.0, 0.5, 1.0, 1.0),
            ..Default::default()
        };

        // At 5% progress, voxel should be in white-hot / incandescent phase
        let progress_early = voxel.age / voxel.max_life;
        assert!(progress_early < 0.1);

        // Advance to 50% progress (1.0s in): should still be actively hot
        voxel.age = 1.0;
        let progress_mid = voxel.age / voxel.max_life;
        assert!(progress_mid < 0.72);

        // Advance past 75% progress (1.5s in): should be fully cooled to base color
        voxel.age = 1.6;
        let progress_late = voxel.age / voxel.max_life;
        assert!(progress_late >= 0.72);
    }
}
