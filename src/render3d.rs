//! Low-level 3D drawing for the playfield: camera framing, grid-to-world
//! coordinate mapping, shaded cubes, and the cabinet that contains them.
//!
//! macroquad's vertex format carries only position, UV and colour — there are no
//! normals, so a real lighting shader is not available. Instead every cube is
//! drawn as six independent quads, each tinted by a fixed per-face brightness,
//! and every large surface is split into unit tiles whose corners are tinted by
//! the CPU-evaluated scene lights in [`crate::lighting`]. That is enough to make
//! a stack of same-coloured blocks read as distinct solid volumes and the
//! cabinet read as lit steel.

use macroquad::prelude::*;

use crate::grid::{GRID_COUNT_COLS, VISIBLE_GRID_COUNT_ROWS};
use crate::lighting::{lit, SceneLights};
use crate::textures::SceneTextures;

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

/// The camera sits in front of the well and above its centre, tilted down about
/// nine degrees: enough that the top face of every stacked block reads as a
/// real surface a couple of pixels tall, and the shaft's floor and inner walls
/// show inside the rim, while rows stay evenly spaced and columns vertical.
///
/// There is deliberately no yaw. Keeping the view left/right symmetric means a
/// column looks the same wherever it sits on the board, so the player can still
/// judge horizontal alignment of a falling piece at a glance.
const CAMERA_POSITION: Vec3 = Vec3::new(0.0, 4.83, 33.0);

/// Look a little below the well's centre so the downward tilt is shared across
/// the board rather than concentrated at the bottom.
///
/// The position and target are offset from the well's centre by the same amount,
/// which slides the well down the frame without steepening the tilt. That is
/// what balances the lintel above the well against the pit below it.
const CAMERA_TARGET: Vec3 = Vec3::new(0.0, -0.40, 0.0);

/// Cosine of the camera's downward pitch. The orthographic view height is
/// scaled by this so that one vertical world unit still covers exactly
/// [`CELL_PIXEL_PITCH`] framebuffer pixels however far the camera tilts.
fn camera_pitch_cosine() -> f32 {
    let forward = CAMERA_TARGET - CAMERA_POSITION;
    forward.z.abs() / forward.length()
}

fn camera_view_height() -> f32 {
    RENDER_HEIGHT as f32 * camera_pitch_cosine() / CELL_PIXEL_PITCH
}

/// An explicit aspect ratio independently locks the horizontal pitch to the
/// same 17 pixels without changing the camera's position or direction.
fn camera_aspect() -> f32 {
    RENDER_WIDTH as f32 / (CELL_PIXEL_PITCH * camera_view_height())
}

/// Ghost outlines are drawn brighter than the piece they preview, so the thin
/// wireframe stays readable against both the dark well and the stack behind it.
const GHOST_OUTLINE_SHADE: f32 = 1.28;

/// Per-face brightness multipliers, simulating a light source above, in front of
/// and slightly to the left of the well. Faces facing away from it are dimmed
/// rather than blackened so that a block's hue stays recognisable on every side.
///
/// The front shade is above 1.0 to compensate for the material textures' mean
/// brightness, so a lit front face lands on the palette colour; only the
/// textures' bevel highlights saturate past it.
const SHADE_TOP: f32 = 1.32;
const SHADE_FRONT: f32 = 1.12;
const SHADE_LEFT: f32 = 0.82;
const SHADE_RIGHT: f32 = 0.6;
const SHADE_BACK: f32 = 0.5;
const SHADE_BOTTOM: f32 = 0.42;

/// Base tint of the well's back wall before lighting. Dark enough that under
/// ambient light alone it reads near-black, so lit blocks pop against it.
const WELL_BACK_COLOR: Color = color_u8!(40, 41, 40, 255);

/// The cabinet face and sloped throat around the recessed playfield. These stay
/// neutral steel so the coloured blocks, the warm lamps and the semantic amber
/// HUD accents remain the strongest colours on screen.
const BEZEL_FRAME_COLOR: Color = color_u8!(52, 53, 52, 255);
const BEZEL_TEXTURE_COLOR: Color = color_u8!(88, 90, 90, 255);
const BEZEL_SIDE_COLOR: Color = color_u8!(44, 45, 46, 255);
const BEZEL_EDGE_COLOR: Color = color_u8!(137, 126, 99, 185);
const BEZEL_SEAM_COLOR: Color = color_u8!(13, 12, 10, 230);

/// Faint cell guides on the back wall. These are a gameplay aid as much as
/// decoration: with the 3D camera applied, they give the eye a reference for
/// which column a falling piece is over.
const WELL_GUIDE_COLOR: Color = color_u8!(166, 151, 112, 18);
const WELL_GUIDE_MAJOR_COLOR: Color = color_u8!(210, 172, 96, 38);

/// The visible fascia is exactly one unit wide, so its material repeats at one
/// tile per world unit; tiles along a non-integral edge are clipped, never
/// stretched.
const BEZEL_FASCIA_WIDTH: f32 = 1.0;
const BEZEL_BOX_DEPTH: f32 = 0.18;
const BEZEL_TILE_SIZE: f32 = 1.0;

/// The rim around the opening is a narrow chamfer from the fascia front down
/// to the block plane, so the blocks sit only a little deeper than the frame
/// and the frame reads as a lip rather than a ledge the stack floats above.
const BEZEL_BEVEL_WIDTH: f32 = 0.12;
const BEZEL_THROAT_Z: f32 = (WELL_DEPTH * 0.5) + 0.02;
pub const BEZEL_FRONT_Z: f32 = BEZEL_THROAT_Z + 0.10;

/// Centre line of each pillar, where the bolts run and the lamps mount.
pub const BEZEL_PILLAR_CENTER_X: f32 =
    WELL_WIDTH * 0.5 + BEZEL_BEVEL_WIDTH + BEZEL_FASCIA_WIDTH * 0.5;

/// The lintel is a heavier beam spanning the pillar tops. It is deeper than the
/// pillars so it stands proud of them, and it is where the score readout is
/// mounted (see [`lintel_front_bounds`]). Pieces spawn in the hidden rows
/// behind it and visibly drop out from under it into the well, through the
/// slot left by [`BEZEL_LINTEL_CLEARANCE`] above the visible rows.
const BEZEL_LINTEL_HEIGHT: f32 = 2.0;
const BEZEL_LINTEL_DEPTH: f32 = 0.30;
const BEZEL_LINTEL_CLEARANCE: f32 = 0.5;

/// The shaft narrows slightly toward its back wall. With no yaw and an
/// orthographic camera the cavity's side walls would otherwise be edge-on and
/// invisible; this taper lets a thin strip of each wall show inside the rim
/// wherever the well is empty, so the blocks read as sitting inside a volume.
const WELL_SPLAY: f32 = 0.15;

/// The shaft's back and side walls continue up behind the lintel, covering the
/// hidden spawn rows, so the slot under the lintel looks into the shaft rather
/// than out at the wall behind the cabinet.
const BACK_WALL_ROWS_ABOVE: usize = 2;

/// Bolt heads studding the pillars and lintel corners.
const BOLT_SIZE: f32 = 0.28;
const BOLT_DEPTH: f32 = 0.10;
const BOLT_SPACING: f32 = 3.0;
const BOLT_COLOR: Color = color_u8!(132, 134, 132, 255);

/// Caged bulkhead lamps mounted on the pillar fronts.
const LAMP_HOUSING_SIZE: Vec3 = Vec3::new(0.62, 0.52, 0.26);
const LAMP_HOUSING_COLOR: Color = color_u8!(70, 72, 72, 255);
const LAMP_CAGE_COLOR: Color = color_u8!(20, 20, 18, 255);

/// Steel of the furnace grille on the fascia and the floor grate in the well.
const GRATE_COLOR: Color = color_u8!(74, 76, 76, 255);

/// The well has no solid floor. Its bottom is an open grate at the block
/// plane, and below it the shaft continues down into a pit: the back and side
/// walls extend to a pool of molten metal this far under the grate, seen
/// through the floor bars from above and through the fascia's grille from
/// the front. Debris from cleared rows falls through the grate into it.
pub const PIT_DEPTH: f32 = 1.7;

/// `y` of the well's floor grate and of the lava surface beneath it.
pub const FLOOR_Y: f32 = -WELL_HEIGHT * 0.5;
pub const LAVA_Y: f32 = FLOOR_Y - PIT_DEPTH;

/// Floor grate bars run front to back on a four-pixel pitch, like the grille.
const FLOOR_BAR_PITCH_PIXELS: f32 = 4.0;
const FLOOR_BAR_WIDTH_PIXELS: f32 = 2.0;
const FLOOR_BAR_HEIGHT: f32 = 0.12;

/// Floor bar positions, as the `x` of each bar's left edge, measured out from
/// the well's centre line so they land on whole framebuffer pixels.
fn floor_bar_left_edges() -> impl Iterator<Item = f32> {
    let pixel = 1.0 / CELL_PIXEL_PITCH;
    let bar_pitch = FLOOR_BAR_PITCH_PIXELS * pixel;
    let bar_count = (WELL_WIDTH / bar_pitch).floor() as usize;
    let bars_left = -bar_pitch * bar_count as f32 * 0.5 + pixel;

    (0..bar_count).map(move |bar| bars_left + bar as f32 * bar_pitch)
}

/// Whether `x` lies in a gap between floor bars rather than over a bar, so
/// the debris physics and the drawn grate agree on what falls through.
pub fn floor_gap_contains(x: f32) -> bool {
    let bar_width = FLOOR_BAR_WIDTH_PIXELS / CELL_PIXEL_PITCH;
    !floor_bar_left_edges().any(|left| x >= left && x < left + bar_width)
}

/// Molten metal palette: the cooler crust between the bright channels, and
/// the hottest glow.
const LAVA_CRUST_COLOR: Color = color_u8!(170, 50, 10, 255);
const LAVA_GLOW_COLOR: Color = color_u8!(255, 160, 45, 255);
const LAVA_HOT_COLOR: Color = color_u8!(255, 240, 180, 255);

/// The pit walls are close enough to the melt to be lit by it directly: this
/// is the light on the wall at the lava line, fading up to the floor grate.
const PIT_WALL_GLOW: Vec3 = Vec3::new(3.2, 1.3, 0.3);

/// Indices into [`bezel_fascia_boxes`].
const FASCIA_BOTTOM: usize = 0;
const FASCIA_LINTEL: usize = 3;

/// Nudge used to lift the back-wall guides off the wall itself. Without it the
/// two coplanar surfaces z-fight and the guides flicker as the scene redraws.
const GUIDE_Z_BIAS: f32 = 0.02;

/// `z` of the well's back wall, and of the guides drawn just in front of it.
const BACK_WALL_Z: f32 = -WELL_DEPTH / 2.0;

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
        fovy: camera_view_height(),
        projection: Projection::Orthographics,
        aspect: Some(camera_aspect()),
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
    let top = camera_view_height() * 0.5;
    let right = top * camera_aspect();
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

/// Inverse of [`world_to_screen`] onto the plane `z == plane_z`: the world point
/// under a framebuffer pixel on, say, the wall behind the cabinet. Lets 2D
/// backdrop drawing sample the same scene lights as the 3D geometry.
pub fn screen_to_world_on_plane(point: Vec2, plane_z: f32) -> Vec3 {
    let inverse = well_view_projection_matrix(Vec2::ZERO).inverse();
    let ndc = Vec2::new(
        point.x / RENDER_WIDTH as f32 * 2.0 - 1.0,
        1.0 - point.y / RENDER_HEIGHT as f32 * 2.0,
    );
    let near = inverse.project_point3(Vec3::new(ndc.x, ndc.y, -1.0));
    let far = inverse.project_point3(Vec3::new(ndc.x, ndc.y, 1.0));
    let t = (plane_z - near.z) / (far.z - near.z);

    near + (far - near) * t
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

/// Draw one quad with explicit texture orientation and per-corner colours.
///
/// `origin` carries the material's top-left texel; `e1` runs along the
/// material's rows (u) and `e2` down its columns (v). Callers therefore decide
/// which world edge a material's lit top edge lands on, instead of inheriting
/// `draw_affine_parallelogram`'s transposed mapping. Colours follow the same
/// corner order and are interpolated across the face, which is how every
/// gradient and light falloff in the scene is drawn without a lighting shader.
pub fn draw_quad(origin: Vec3, e1: Vec3, e2: Vec3, texture: Option<&Texture2D>, colors: [Color; 4]) {
    draw_quad_uv(origin, e1, e2, Vec2::ZERO, Vec2::ONE, texture, colors);
}

/// [`draw_quad`] over a sub-rectangle of the texture, from `uv_min` at
/// `origin` to `uv_max` at the opposite corner. Used to clip the last partial
/// tile when a small material is tiled across a surface of arbitrary size.
pub fn draw_quad_uv(
    origin: Vec3,
    e1: Vec3,
    e2: Vec3,
    uv_min: Vec2,
    uv_max: Vec2,
    texture: Option<&Texture2D>,
    colors: [Color; 4],
) {
    let vertices = vec![
        Vertex::new2(origin, vec2(uv_min.x, uv_min.y), colors[0]),
        Vertex::new2(origin + e1, vec2(uv_max.x, uv_min.y), colors[1]),
        Vertex::new2(origin + e1 + e2, vec2(uv_max.x, uv_max.y), colors[2]),
        Vertex::new2(origin + e2, vec2(uv_min.x, uv_max.y), colors[3]),
    ];

    draw_mesh(&Mesh {
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: texture.cloned(),
    });
}

/// Draw an untextured quad through four arbitrary corners, given in the same
/// order as [`draw_quad`] (material top-left, then around). Used for surfaces
/// that are not parallelograms, such as the tapered walls of the shaft.
fn draw_quad_corners(corners: [Vec3; 4], colors: [Color; 4]) {
    let vertices = vec![
        Vertex::new2(corners[0], vec2(0.0, 0.0), colors[0]),
        Vertex::new2(corners[1], vec2(1.0, 0.0), colors[1]),
        Vertex::new2(corners[2], vec2(1.0, 1.0), colors[2]),
        Vertex::new2(corners[3], vec2(0.0, 1.0), colors[3]),
    ];

    draw_mesh(&Mesh {
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: None,
    });
}

/// Draw a quad whose corners are each tinted by the scene light falling on
/// them. Large surfaces should be split into roughly unit-sized quads before
/// calling this, since the light is only evaluated at the corners.
pub fn draw_lit_quad(
    origin: Vec3,
    e1: Vec3,
    e2: Vec3,
    texture: Option<&Texture2D>,
    base: Color,
    lights: &SceneLights,
) {
    let corners = [origin, origin + e1, origin + e1 + e2, origin + e2];
    let colors = corners.map(|corner| lit(base, lights.at(corner)));
    draw_quad(origin, e1, e2, texture, colors);
}

/// Mirror a quad's texture along either axis without moving its geometry, by
/// shifting the origin to the opposite corner and negating the edge. Used to
/// break up visible repetition when one small material tiles a surface.
fn flipped_quad(origin: Vec3, e1: Vec3, e2: Vec3, flip_x: bool, flip_y: bool) -> (Vec3, Vec3, Vec3) {
    let mut origin = origin;
    let mut e1 = e1;
    let mut e2 = e2;

    if flip_x {
        origin += e1;
        e1 = -e1;
    }
    if flip_y {
        origin += e2;
        e2 = -e2;
    }

    (origin, e1, e2)
}

/// A face of a cube described in the cube's own edge basis. `origin` selects
/// the corner holding the material's top-left texel (each component is 0 or 1
/// along the cube's x, y and z edges), and `e1`/`e2` are signed edge axes for
/// the material's rows and columns. Orientations are chosen so that every face
/// is upright and unmirrored when seen from outside the cube.
struct CubeFace {
    origin: Vec3,
    e1: Vec3,
    e2: Vec3,
}

/// Faces in the same order as the `FACE_*` indices: back, front, bottom, top,
/// left, right.
const CUBE_FACES: [CubeFace; 6] = [
    CubeFace {
        origin: Vec3::new(1.0, 1.0, 0.0),
        e1: Vec3::NEG_X,
        e2: Vec3::NEG_Y,
    },
    CubeFace {
        origin: Vec3::new(0.0, 1.0, 1.0),
        e1: Vec3::X,
        e2: Vec3::NEG_Y,
    },
    CubeFace {
        origin: Vec3::new(0.0, 0.0, 1.0),
        e1: Vec3::X,
        e2: Vec3::NEG_Z,
    },
    CubeFace {
        origin: Vec3::new(0.0, 1.0, 0.0),
        e1: Vec3::X,
        e2: Vec3::Z,
    },
    CubeFace {
        origin: Vec3::new(0.0, 1.0, 0.0),
        e1: Vec3::Z,
        e2: Vec3::NEG_Y,
    },
    CubeFace {
        origin: Vec3::new(1.0, 1.0, 1.0),
        e1: Vec3::NEG_Z,
        e2: Vec3::NEG_Y,
    },
];

/// Resolve a [`CubeFace`] against a cube's minimum corner and (possibly
/// rotated) edge vectors into a world-space origin and edge pair for
/// [`draw_quad`].
fn cube_face_geometry(
    face: &CubeFace,
    min: Vec3,
    edge_x: Vec3,
    edge_y: Vec3,
    edge_z: Vec3,
) -> (Vec3, Vec3, Vec3) {
    let in_basis = |v: Vec3| edge_x * v.x + edge_y * v.y + edge_z * v.z;
    (min + in_basis(face.origin), in_basis(face.e1), in_basis(face.e2))
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

/// Cover the front of a box with square, individually lit material tiles at a
/// fixed world scale. Alternating UV direction breaks up obvious repetition
/// without changing tile density or introducing geometry seams. Tiles along a
/// non-integral edge are clipped through their UVs rather than stretched.
fn draw_tiled_front_face(
    center: Vec3,
    size: Vec3,
    texture: &Texture2D,
    tint: Color,
    phase: usize,
    lights: &SceneLights,
) {
    let columns = (size.x / BEZEL_TILE_SIZE).ceil() as usize;
    let rows = (size.y / BEZEL_TILE_SIZE).ceil() as usize;
    let min = center - (size * 0.5);
    let top = center.y + size.y * 0.5;
    let z = center.z + size.z * 0.5 + 0.003;

    for row in 0..rows {
        for column in 0..columns {
            let index = row * columns + column + phase;
            let x = min.x + column as f32 * BEZEL_TILE_SIZE;
            let y = top - row as f32 * BEZEL_TILE_SIZE;
            let width = BEZEL_TILE_SIZE.min(min.x + size.x - x);
            let height = BEZEL_TILE_SIZE.min(y - (top - size.y));
            let origin = Vec3::new(x, y, z);
            let e1 = Vec3::X * width;
            let e2 = Vec3::NEG_Y * height;
            let corners = [origin, origin + e1, origin + e1 + e2, origin + e2];
            let colors = corners.map(|corner| lit(tint, lights.at(corner)));
            let uv_max = Vec2::new(width / BEZEL_TILE_SIZE, height / BEZEL_TILE_SIZE);
            let (uv_min, uv_max) = flipped_uvs(uv_max, index % 2 == 1, (index / 2) % 2 == 1);

            draw_quad_uv(origin, e1, e2, uv_min, uv_max, Some(texture), colors);
        }
    }
}

/// Mirror a tile's UV range along either axis. Where a clipped tile is
/// mirrored, the clipped edge moves to the far side of the texture so the tile
/// still shows a full texel at the surface's edge.
fn flipped_uvs(uv_max: Vec2, flip_x: bool, flip_y: bool) -> (Vec2, Vec2) {
    let (u0, u1) = if flip_x {
        (1.0, 1.0 - uv_max.x)
    } else {
        (0.0, uv_max.x)
    };
    let (v0, v1) = if flip_y {
        (1.0, 1.0 - uv_max.y)
    } else {
        (0.0, uv_max.y)
    };

    (Vec2::new(u0, v0), Vec2::new(u1, v1))
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

/// Fixed per-face brightness indexed by the `FACE_*` constants.
const FACE_SHADES: [f32; 6] = [
    SHADE_BACK,
    SHADE_FRONT,
    SHADE_BOTTOM,
    SHADE_TOP,
    SHADE_LEFT,
    SHADE_RIGHT,
];

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

/// Direction from the scene toward the camera. The camera is orthographic, so
/// this is the same for every point in the scene and is what a rotated face's
/// visibility must be tested against: testing against the camera's position
/// instead treats the view as perspective, and misjudges faces near edge-on
/// for anything drawn away from the screen centre.
fn view_direction() -> Vec3 {
    (CAMERA_POSITION - CAMERA_TARGET).normalize()
}

/// Outward normals of a cube's six faces after `rotation`, in `FACE_*` order
/// (back, front, bottom, top, left, right).
///
/// Each negative axis is rotated directly rather than written as `-rotation *
/// axis`: unary minus binds tighter than `*`, and a negated quaternion is the
/// same rotation, so that spelling would give opposite faces identical normals.
fn rotated_face_normals(rotation: Quat) -> [Vec3; 6] {
    [
        rotation * Vec3::NEG_Z,
        rotation * Vec3::Z,
        rotation * Vec3::NEG_Y,
        rotation * Vec3::Y,
        rotation * Vec3::NEG_X,
        rotation * Vec3::X,
    ]
}

/// Determine which of an axis-aligned cube's six faces point toward the camera.
/// The order is back, front, bottom, top, left, right.
///
/// The camera is orthographic, so the answer is the same for every block on
/// the board: the front and, because the camera looks down, the top. The side
/// faces are exactly edge-on and never drawn, and no block ever shows its
/// underside, which keeps the whole stack reading as one consistently lit
/// volume from the bottom row to the top.
fn visible_block_faces() -> [bool; 6] {
    let view = view_direction();
    rotated_face_normals(Quat::IDENTITY).map(|normal| normal.dot(view) > 0.0)
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
pub fn draw_block_cube_scaled(
    center: Vec3,
    color: Color,
    scale: f32,
    textures: &SceneTextures,
    lights: &SceneLights,
) {
    let size = BLOCK_INSET * scale;
    let min = center - Vec3::splat(size * 0.5);
    let edge_x = Vec3::new(size, 0.0, 0.0);
    let edge_y = Vec3::new(0.0, size, 0.0);
    let edge_z = Vec3::new(0.0, 0.0, size);
    let visible = visible_block_faces();
    let texture = textures.for_color(color);

    // Material selection keys off the palette colour; only the drawn colour
    // picks up the room's light.
    let tint = lights.block_tint(center);
    let color = Color::new(color.r * tint.r, color.g * tint.g, color.b * tint.b, color.a);

    for (index, face) in CUBE_FACES.iter().enumerate() {
        if !visible[index] {
            continue;
        }

        let (origin, e1, e2) = cube_face_geometry(face, min, edge_x, edge_y, edge_z);
        let shade = FACE_SHADES[index];

        // Shades at the quad's origin edge and at its far (`e2`) edge. The front
        // face runs top to bottom and carries a slight falloff so a column of
        // identical blocks still reads as separate lit volumes. The top face
        // runs from the back wall forward and is only a few pixels deep on
        // screen, so it is flat-shaded rather than textured, darkest at the
        // back and brightest along its front lip: a lit surface receding into
        // the shaft.
        let (texture, origin_shade, far_shade) = match index {
            FACE_FRONT => (Some(texture), shade * 1.04, shade * 0.94),
            FACE_TOP => (None, shade * 0.72, shade * 1.0),
            _ => (Some(texture), shade, shade),
        };
        let origin_color = shaded(color, origin_shade, 1.0);
        let far_color = shaded(color, far_shade, 1.0);

        draw_quad(
            origin,
            e1,
            e2,
            texture,
            [origin_color, origin_color, far_color, far_color],
        );
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

        // Only silhouette edges are inked: the crease where the lit top meets
        // the front is left to the faces' own shading, so a dark line does not
        // cut a groove between them. Lines remain on their true geometry so the
        // depth buffer can occlude them behind neighbouring blocks.
        if visible_edges[index] && visible_count == 1 {
            draw_line_3d(corners[start], corners[end], shaded(color, 0.27, 1.0));
        }
    }
}

/// Maximum simultaneous active shrapnel voxels across all clearing rows.
pub const MAX_SHRAPNEL_VOXELS: usize = 320;

/// How long a carnage voxel's in-flight incandescence lasts before it cools
/// back to its base material.
pub const CARNAGE_GLOW_SECONDS: f32 = 1.4;

/// A 3D tumbling sub-voxel spawned when rows are cleared. It flies, drops
/// through the floor grate, and ends its life sinking into the melt.
#[derive(Copy, Clone, Debug, Default)]
pub struct ShrapnelVoxel {
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Vec3,
    pub angular_velocity: Vec3,
    pub color: Color,
    pub size: f32,
    /// Seconds since the voxel was spawned.
    pub age: f32,
    /// How far the voxel has sunk below the lava surface, as a fraction of
    /// its own size: 0.0 while airborne, 1.0 when it has gone under and is
    /// retired.
    pub submersion: f32,
    pub bounce_count: u8,
    pub is_carnage: bool,
    pub active: bool,
}

impl ShrapnelVoxel {
    /// Whether the voxel has reached the melt and is sinking.
    pub fn is_sinking(&self) -> bool {
        self.submersion > 0.0
    }
}

/// Maximum simultaneous lava splashes.
pub const MAX_LAVA_SPLASHES: usize = 48;

/// How long a splash ring takes to spread and fade.
pub const SPLASH_SECONDS: f32 = 0.6;

/// A ring of light spreading across the melt where debris went in.
#[derive(Copy, Clone, Debug, Default)]
pub struct LavaSplash {
    pub position: Vec3,
    pub age: f32,
    /// Size of the debris that made it, which sets the ring's reach.
    pub size: f32,
    pub active: bool,
}

/// Draw the active splash rings: each is a bright disc that spreads out and
/// thins, drawn flat on the lava surface. Must be drawn after the lava and
/// before the floor grate, so the bars still occlude it.
pub fn draw_lava_splashes(splashes: &[LavaSplash]) {
    for splash in splashes.iter().filter(|splash| splash.active) {
        let t = (splash.age / SPLASH_SECONDS).clamp(0.0, 1.0);
        let radius = splash.size * (1.0 + 3.4 * t);
        let alpha = (1.0 - t).powi(2);
        let color = mix_color(LAVA_HOT_COLOR, LAVA_GLOW_COLOR, t);
        let center = Vec3::new(splash.position.x, LAVA_Y + 0.01, splash.position.z);

        // The ring on the surface is nearly edge-on to the camera, so a short
        // vertical flare of the same light stands up from the point of entry
        // to make the splash legible.
        draw_glow_disc_on_plane(center, radius, color, alpha);
        draw_glow_disc(
            center + Vec3::Y * splash.size * 0.6,
            splash.size * (1.2 + 1.6 * t),
            color,
            alpha * 0.8,
        );
    }
}

/// Draw a single tumbling shrapnel voxel with dynamic directional shading,
/// point-sampled texture, and hot-metal incandescence: briefly in flight for
/// four-line clears, and always once it hits the melt.
pub fn draw_shrapnel_voxel(voxel: &ShrapnelVoxel, textures: &SceneTextures) {
    if !voxel.active {
        return;
    }

    // A voxel that has reached the melt floats on the surface while the heat
    // takes it, then is clipped by the surface: the cube shrinks toward its
    // top and the visible remainder rides the lava, which at this pixel scale
    // reads as the debris going under.
    const HEAT_SOAK: f32 = 0.55;
    let remaining = if voxel.submersion <= HEAT_SOAK {
        1.0
    } else {
        1.0 - (voxel.submersion - HEAT_SOAK) / (1.0 - HEAT_SOAK)
    };
    let scale = voxel.size * remaining;
    if scale <= 0.001 {
        return;
    }

    // Four-line clears superheat shrapnel into molten incandescence in flight,
    // slowly cooling back to the block's base material; anything that reaches
    // the melt heats up again as it soaks and goes under white-hot.
    let flight_heat = if voxel.is_carnage {
        (1.0 - voxel.age / CARNAGE_GLOW_SECONDS).clamp(0.0, 1.0).powf(1.35)
    } else {
        0.0
    };
    let sink_heat = (voxel.submersion / HEAT_SOAK).clamp(0.0, 1.0);
    let heat = flight_heat.max(sink_heat);
    let is_hot = heat > 0.0;
    let fade = remaining;

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

    // A sinking voxel's visible remainder rides on the surface, lifted a
    // touch so it never z-fights with the melt.
    let position = if voxel.is_sinking() {
        Vec3::new(voxel.position.x, LAVA_Y + scale * 0.5 + 0.02, voxel.position.z)
    } else {
        voxel.position
    };

    if is_hot && heat > 0.15 {
        // Debris on the melt throws a bigger, brighter halo than debris
        // glowing in flight: it is the thing that is burning up.
        let (reach, strength) = if voxel.is_sinking() {
            (2.2, 0.65)
        } else {
            (1.3, 0.4)
        };
        let halo_radius = voxel.size * (reach + 0.9 * heat);
        let halo_alpha = (heat * strength * fade.max(0.3)).clamp(0.0, 0.7);

        draw_glow_disc(
            position,
            halo_radius,
            Color::new(1.0, 0.62, 0.14, 1.0),
            halo_alpha,
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
    let min = position - 0.5 * (edge_x + edge_y + edge_z);

    let light_dir = Vec3::new(-0.35, 0.75, 0.55).normalize();
    let view = view_direction();
    let texture = textures.for_color(color);
    let face_normals = rotated_face_normals(rot);

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
    for (index, face) in CUBE_FACES.iter().enumerate() {
        let normal = face_normals[index];
        let (origin, e1, e2) = cube_face_geometry(face, min, edge_x, edge_y, edge_z);

        if normal.dot(view) > 0.0 {
            visible_faces[index] = true;
            let ndotl = normal.dot(light_dir).max(0.0);
            let shade = (0.42 + 0.88 * ndotl) * (1.0 - heat * 0.85)
                + (1.08 + 0.22 * ndotl) * (heat * 0.85);
            let texture_opt = if is_hot && heat > 0.42 {
                None
            } else {
                Some(texture)
            };

            draw_quad(origin, e1, e2, texture_opt, [shaded(color, shade, 1.0); 4]);
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

/// Draw a single textured cube at an arbitrary rotation with the shrapnel
/// shading model, for decorative uses such as the menu cursor. It is simply an
/// ageless, cool shrapnel voxel.
pub fn draw_tumbling_cube(
    position: Vec3,
    rotation: Vec3,
    size: f32,
    color: Color,
    textures: &SceneTextures,
) {
    let voxel = ShrapnelVoxel {
        position,
        rotation,
        color,
        size,
        active: true,
        ..Default::default()
    };

    draw_shrapnel_voxel(&voxel, textures);
}

/// Draw all active 3D shrapnel voxels.
pub fn draw_shrapnel(voxels: &[ShrapnelVoxel], textures: &SceneTextures) {
    for voxel in voxels {
        if voxel.active {
            draw_shrapnel_voxel(voxel, textures);
        }
    }
}

/// Draw a single playfield block as an inset shaded cube centred on `center`.
pub fn draw_block_cube(center: Vec3, color: Color, textures: &SceneTextures, lights: &SceneLights) {
    draw_block_cube_scaled(center, color, 1.0, textures, lights);
}

/// Which edges of a ghost cell lie on the piece's silhouette, in the order
/// top, right, bottom, left.
pub type GhostEdges = [bool; 4];

/// Length of each leg of a targeting bracket, in world units.
const GHOST_BRACKET_LEG: f32 = 0.32;

/// Draw one cell of the hard-drop landing preview as part of a targeting
/// designator: the piece's silhouette as a faint line, with bright brackets
/// hooked around every convex corner.
///
/// Lines are used rather than a translucent cube on purpose: alpha-blended
/// cubes need to be sorted back-to-front against the stack to composite
/// correctly, whereas lines read cleanly at any depth and never hide the blocks
/// the player is aiming at.
pub fn draw_ghost_cell(center: Vec3, color: Color, exterior: GhostEdges) {
    let pulse = 0.82 + (((get_time() as f32 * 4.0).sin() + 1.0) * 0.09);
    let half = BLOCK_INSET * 0.5;
    let z = center.z + half + BLOCK_FACE_Z_BIAS;
    let silhouette = shaded(color, GHOST_OUTLINE_SHADE, pulse * 0.45);
    let bracket = shaded(color, GHOST_OUTLINE_SHADE * 1.1, pulse);

    // Corners clockwise from top-left; edge `i` runs from corner `i` to
    // corner `i + 1`, so corner `i` joins edges `i - 1` and `i`.
    let corners = [
        Vec3::new(center.x - half, center.y + half, z),
        Vec3::new(center.x + half, center.y + half, z),
        Vec3::new(center.x + half, center.y - half, z),
        Vec3::new(center.x - half, center.y - half, z),
    ];

    for edge in 0..4 {
        if exterior[edge] {
            draw_line_3d(corners[edge], corners[(edge + 1) % 4], silhouette);
        }
    }

    for corner in 0..4 {
        let incoming = exterior[(corner + 3) % 4];
        let outgoing = exterior[corner];
        if !(incoming && outgoing) {
            continue;
        }

        let point = corners[corner];
        let toward_next = (corners[(corner + 1) % 4] - point).normalize();
        let toward_previous = (corners[(corner + 3) % 4] - point).normalize();
        draw_line_3d(point, point + toward_next * GHOST_BRACKET_LEG, bracket);
        draw_line_3d(point, point + toward_previous * GHOST_BRACKET_LEG, bracket);
    }
}

/// Draw the well: the shaft's back wall, tapered side walls and floor, the cell
/// guides on the back wall, and the frame around the opening.
///
/// Must be drawn before the blocks. The shaft is opaque and would otherwise
/// need to sort behind them, and drawing it first also gives the depth buffer a
/// sane floor for everything that follows.
pub fn draw_well(
    textures: &SceneTextures,
    lights: &SceneLights,
    time: f64,
    splashes: &[LavaSplash],
) {
    let half_w = WELL_WIDTH / 2.0;
    let half_h = WELL_HEIGHT / 2.0;

    draw_back_wall(half_w, half_h, textures, lights);
    draw_shaft_walls(half_w, half_h, lights);
    draw_lava(half_w, lights, time);
    draw_lava_splashes(splashes);
    draw_floor_grate(half_w, lights);
    draw_well_guides(half_w, half_h);
    draw_well_bezel(half_w, half_h, textures, lights);
}

/// Half-width of the shaft at its back wall, after the taper.
fn back_wall_half_width(half_w: f32) -> f32 {
    half_w - WELL_SPLAY
}

/// Top of the shaft walls, above the visible rows and hidden behind the lintel.
fn shaft_top_y(half_h: f32) -> f32 {
    half_h + BACK_WALL_ROWS_ABOVE as f32
}

/// Bottom of the shaft walls: the lava surface at the foot of the pit.
fn shaft_bottom_y() -> f32 {
    LAVA_Y
}

/// Light falling on a shaft surface at `point`: the scene lights, plus the
/// melt's own glow on anything down in the pit with it, strongest at the lava
/// line and gone by the floor grate. The steel down there is heated as much
/// as lit, so the glow is not bounded by the surface's dark base tint.
fn shaft_light_at(point: Vec3, lights: &SceneLights) -> Vec3 {
    let scene = lights.at(point);
    if point.y >= FLOOR_Y {
        return scene;
    }

    let depth = ((FLOOR_Y - point.y) / PIT_DEPTH).clamp(0.0, 1.0);
    scene + PIT_WALL_GLOW * depth.powf(1.3) * lights.furnace_level()
}

/// Base tint of a shaft surface at `point`: its usual steel above the floor,
/// warming toward dull red heat down in the pit.
fn shaft_base_color(point: Vec3, base: Color) -> Color {
    if point.y >= FLOOR_Y {
        return base;
    }

    let depth = ((FLOOR_Y - point.y) / PIT_DEPTH).clamp(0.0, 1.0);
    mix_color(base, color_u8!(120, 44, 14, 255), depth * 0.7)
}

/// [`draw_quad`] with each corner tinted by [`shaft_base_color`] and lit by
/// [`shaft_light_at`].
fn draw_shaft_quad(
    origin: Vec3,
    e1: Vec3,
    e2: Vec3,
    texture: Option<&Texture2D>,
    base: Color,
    lights: &SceneLights,
) {
    let corners = [origin, origin + e1, origin + e1 + e2, origin + e2];
    let colors = corners
        .map(|corner| lit(shaft_base_color(corner, base), shaft_light_at(corner, lights)));
    draw_quad(origin, e1, e2, texture, colors);
}

/// [`draw_quad_corners`] with each corner tinted by [`shaft_base_color`],
/// scaled by `shade`, and lit by [`shaft_light_at`].
fn draw_shaft_quad_corners(corners: [Vec3; 4], base: Color, shade: f32, lights: &SceneLights) {
    let colors = corners.map(|corner| {
        lit(
            shaded(shaft_base_color(corner, base), shade, 1.0),
            shaft_light_at(corner, lights),
        )
    });
    draw_quad_corners(corners, colors);
}

/// The shaft's back wall: dark gunmetal plate, one lit tile per cell, so the
/// lava glow climbs the bottom rows and the lamps catch the top corners. Kept
/// dark overall so the coloured blocks stay the brightest thing inside the
/// well. Tiles are squeezed to the tapered width, continue up over the hidden
/// spawn rows, and run down past the floor grate to the melt.
fn draw_back_wall(half_w: f32, half_h: f32, textures: &SceneTextures, lights: &SceneLights) {
    let back_half_w = back_wall_half_width(half_w);
    let tile_width = back_half_w * 2.0 / GRID_COUNT_COLS as f32;
    let top = shaft_top_y(half_h);
    let bottom = shaft_bottom_y();
    let rows = ((top - bottom) / 1.0).ceil() as usize;

    for row in 0..rows {
        let y_top = top - row as f32;
        let height = (y_top - bottom).min(1.0);
        for col in 0..GRID_COUNT_COLS {
            let top_left = Vec3::new(col as f32 * tile_width - back_half_w, y_top, BACK_WALL_Z);
            let (origin, e1, e2) = flipped_quad(
                top_left,
                Vec3::X * tile_width,
                Vec3::NEG_Y * height,
                (row + col) % 2 == 1,
                (row / 2 + col) % 2 == 1,
            );

            draw_shaft_quad(
                origin,
                e1,
                e2,
                Some(textures.gunmetal()),
                WELL_BACK_COLOR,
                lights,
            );
        }
    }
}

/// The shaft's side walls, running from the opening's edge at the block plane
/// back to the narrower back wall, from above the lintel down to the melt.
/// They are lit one row at a time so the lava glow climbs their feet. The
/// right wall is shaded like the blocks' right faces, keeping the light's
/// direction consistent across the scene.
fn draw_shaft_walls(half_w: f32, half_h: f32, lights: &SceneLights) {
    let front_z = WELL_DEPTH * 0.5;
    let back_half_w = back_wall_half_width(half_w);
    let top = shaft_top_y(half_h);
    let bottom = shaft_bottom_y();
    let rows = ((top - bottom) / 1.0).ceil() as usize;

    for row in 0..rows {
        let y_top = top - row as f32;
        let y_bottom = (y_top - 1.0).max(bottom);
        for (x_sign, shade) in [(-1.0, 1.0), (1.0, 0.78)] {
            let front_x = x_sign * half_w;
            let back_x = x_sign * back_half_w;
            draw_shaft_quad_corners(
                [
                    Vec3::new(front_x, y_top, front_z),
                    Vec3::new(back_x, y_top, BACK_WALL_Z),
                    Vec3::new(back_x, y_bottom, BACK_WALL_Z),
                    Vec3::new(front_x, y_bottom, front_z),
                ],
                BEZEL_SIDE_COLOR,
                shade,
                lights,
            );
        }
    }
}

/// The pool of molten metal at the bottom of the pit, filling the tapered
/// shaft's footprint. It is drawn as a grid of small patches so the glow can
/// vary across it: a slowly crawling pattern of bright channels through a
/// darker crust, breathing with the furnace lightstyle.
fn draw_lava(half_w: f32, lights: &SceneLights, time: f64) {
    const PATCHES_X: usize = 20;
    const PATCHES_Z: usize = 3;
    let front_z = WELL_DEPTH * 0.5;
    let back_half_w = back_wall_half_width(half_w);
    let taper = back_half_w / half_w;
    let level = lights.furnace_level();
    let t = time as f32;

    for iz in 0..PATCHES_Z {
        let z0 = front_z - (iz as f32 / PATCHES_Z as f32) * WELL_DEPTH;
        let z1 = front_z - ((iz + 1) as f32 / PATCHES_Z as f32) * WELL_DEPTH;
        let taper0 = 1.0 + (taper - 1.0) * (front_z - z0) / WELL_DEPTH;
        let taper1 = 1.0 + (taper - 1.0) * (front_z - z1) / WELL_DEPTH;

        for ix in 0..PATCHES_X {
            let x0 = -half_w + ix as f32 * WELL_WIDTH / PATCHES_X as f32;
            let x1 = x0 + WELL_WIDTH / PATCHES_X as f32;
            let corners = [
                Vec3::new(x0 * taper1, LAVA_Y, z1),
                Vec3::new(x1 * taper1, LAVA_Y, z1),
                Vec3::new(x1 * taper0, LAVA_Y, z0),
                Vec3::new(x0 * taper0, LAVA_Y, z0),
            ];
            let colors = corners.map(|corner| lava_color_at(corner, t, level));

            draw_quad_corners(corners, colors);
        }
    }

    // Seen this near edge-on, the surface alone is a sliver; a haze of heat
    // rising off it gives the melt a visible glowing band.
    let haze_height = 0.55;
    let hot = lava_color_at(Vec3::new(0.0, LAVA_Y, front_z), t, level);
    let clear = Color::new(hot.r, hot.g, hot.b, 0.0);
    let glow = Color::new(hot.r, hot.g, hot.b, 0.6);
    draw_quad(
        Vec3::new(-half_w, LAVA_Y + haze_height, front_z - 0.02),
        Vec3::X * WELL_WIDTH,
        Vec3::NEG_Y * haze_height,
        None,
        [clear, clear, glow, glow],
    );
}

/// Colour of the lava surface at a point: two drifting sine ridges pick out
/// bright channels, and the whole pool breathes with the furnace.
fn lava_color_at(point: Vec3, time: f32, level: f32) -> Color {
    let ridge_a = ((point.x * 1.7 + time * 0.6).sin() * 0.5 + 0.5).powi(3);
    let ridge_b = ((point.x * 0.9 - point.z * 2.5 - time * 0.35).sin() * 0.5 + 0.5).powi(2);
    let channel = (ridge_a * 0.7 + ridge_b * 0.5).clamp(0.0, 1.0);
    let glow = mix_color(LAVA_CRUST_COLOR, LAVA_GLOW_COLOR, channel);
    let hot = mix_color(glow, LAVA_HOT_COLOR, (channel - 0.75).max(0.0) * 2.0);

    shaded(hot, 0.75 + 0.35 * level, 1.0)
}

/// The open grate that is the well's floor: steel bars running front to back
/// across the shaft at the block plane, with the melt visible between them.
/// Each bar is two framebuffer pixels wide on a four-pixel pitch, measured out
/// from the well's centre line, so they land on whole pixels and line up with
/// the fascia grille below. Drawn after the lava so the bars occlude it.
fn draw_floor_grate(half_w: f32, lights: &SceneLights) {
    let front_z = WELL_DEPTH * 0.5;
    let back_half_w = back_wall_half_width(half_w);
    let bar_width = FLOOR_BAR_WIDTH_PIXELS / CELL_PIXEL_PITCH;
    let grate = lit(GRATE_COLOR, lights.at(Vec3::new(0.0, FLOOR_Y, 0.0)));
    let top_lit = shaded(grate, 1.25, 1.0);
    let top_far = shaded(grate, 0.85, 1.0);
    let front_face = shaded(grate, 0.6, 1.0);

    for x in floor_bar_left_edges() {
        let taper_x = |x: f32| x * back_half_w / half_w;

        // Top face, lit along its front lip like the blocks' tops.
        draw_quad_corners(
            [
                Vec3::new(taper_x(x), FLOOR_Y, BACK_WALL_Z),
                Vec3::new(taper_x(x + bar_width), FLOOR_Y, BACK_WALL_Z),
                Vec3::new(x + bar_width, FLOOR_Y, front_z),
                Vec3::new(x, FLOOR_Y, front_z),
            ],
            [top_far, top_far, top_lit, top_lit],
        );

        // Front face, the bar's thickness seen from the camera.
        draw_quad(
            Vec3::new(x, FLOOR_Y, front_z),
            Vec3::X * bar_width,
            Vec3::NEG_Y * FLOOR_BAR_HEIGHT,
            None,
            [front_face; 4],
        );
    }

    // Front and back rails the bars are welded to.
    for (z, half) in [(front_z, half_w), (BACK_WALL_Z, back_half_w)] {
        draw_quad(
            Vec3::new(-half, FLOOR_Y + 0.002, z),
            Vec3::X * half * 2.0,
            Vec3::NEG_Y * FLOOR_BAR_HEIGHT,
            None,
            [shaded(grate, 0.9, 1.0); 4],
        );
    }
}

/// Draw the faint per-cell grid on the well's back wall. Columns follow the
/// wall's tapered width so the lines stay painted on the wall rather than
/// floating in front of its edges.
fn draw_well_guides(half_w: f32, half_h: f32) {
    let z = BACK_WALL_Z + GUIDE_Z_BIAS;
    let back_half_w = back_wall_half_width(half_w);
    let column_width = back_half_w * 2.0 / GRID_COUNT_COLS as f32;

    for col in 1..GRID_COUNT_COLS {
        let x = col as f32 * column_width - back_half_w;
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
            Vec3::new(-back_half_w, y, z),
            Vec3::new(back_half_w, y, z),
            if row % 5 == 0 {
                WELL_GUIDE_MAJOR_COLOR
            } else {
                WELL_GUIDE_COLOR
            },
        );
    }
}

/// Four continuous solids forming the cabinet fascia: a bottom bar, two side
/// pillars and the lintel across their tops. Neighbours meet only at shared
/// edges, so there are no coplanar overlaps or accumulated alignment errors.
/// All backs are flush; the lintel's extra depth projects forward.
fn bezel_fascia_boxes(half_w: f32, half_h: f32) -> [(Vec3, Vec3); 4] {
    let back_z = BEZEL_FRONT_Z - BEZEL_BOX_DEPTH;
    let front_center_z = back_z + BEZEL_BOX_DEPTH * 0.5;
    let lintel_center_z = back_z + BEZEL_LINTEL_DEPTH * 0.5;
    let outer_width = WELL_WIDTH + 2.0 * (BEZEL_BEVEL_WIDTH + BEZEL_FASCIA_WIDTH);
    let side_bottom = -half_h;
    let side_top = half_h + BEZEL_LINTEL_CLEARANCE;
    let side_x = half_w + BEZEL_BEVEL_WIDTH + BEZEL_FASCIA_WIDTH * 0.5;
    let lintel_y = side_top + BEZEL_LINTEL_HEIGHT * 0.5;

    // The bottom bar fronts the whole pit, from the floor grate down to just
    // below the lava surface, so its grille looks straight in at the melt.
    let pit_front_height = PIT_DEPTH + BEZEL_FASCIA_WIDTH * 0.5;
    let pit_front_y = FLOOR_Y - pit_front_height * 0.5;

    [
        (
            Vec3::new(0.0, pit_front_y, front_center_z),
            Vec3::new(outer_width, pit_front_height, BEZEL_BOX_DEPTH),
        ),
        (
            Vec3::new(-side_x, (side_bottom + side_top) * 0.5, front_center_z),
            Vec3::new(BEZEL_FASCIA_WIDTH, side_top - side_bottom, BEZEL_BOX_DEPTH),
        ),
        (
            Vec3::new(side_x, (side_bottom + side_top) * 0.5, front_center_z),
            Vec3::new(BEZEL_FASCIA_WIDTH, side_top - side_bottom, BEZEL_BOX_DEPTH),
        ),
        (
            Vec3::new(0.0, lintel_y, lintel_center_z),
            Vec3::new(outer_width, BEZEL_LINTEL_HEIGHT, BEZEL_LINTEL_DEPTH),
        ),
    ]
}

/// World-space top-left and bottom-right corners of the lintel's front face.
/// The HUD mounts the score readout here, projecting these through the camera
/// instead of hand-placing a card above the well.
pub fn lintel_front_bounds() -> (Vec3, Vec3) {
    let (center, size) = bezel_fascia_boxes(WELL_WIDTH / 2.0, WELL_HEIGHT / 2.0)[FASCIA_LINTEL];
    let half = size * 0.5;
    let front_z = center.z + half.z;

    (
        Vec3::new(center.x - half.x, center.y + half.y, front_z),
        Vec3::new(center.x + half.x, center.y - half.y, front_z),
    )
}

/// Bolt-head positions on the fascia front: a column up each pillar plus one
/// in each corner of the lintel.
fn bolt_positions(half_w: f32, half_h: f32) -> Vec<Vec3> {
    let boxes = bezel_fascia_boxes(half_w, half_h);
    let pillar_x = boxes[1].0.x.abs();
    let pillar_front_z = boxes[1].0.z + boxes[1].1.z * 0.5;
    let (lintel_center, lintel_size) = boxes[FASCIA_LINTEL];
    let lintel_front_z = lintel_center.z + lintel_size.z * 0.5;
    let mut positions = Vec::new();

    // The lamps take the place of whichever bolt would sit under them.
    let lamp_clearance = LAMP_HOUSING_SIZE.y * 0.5 + BOLT_SIZE;
    let mut y = -half_h + 0.5;
    while y < half_h {
        let under_lamp = SceneLights::lamp_positions()
            .iter()
            .any(|lamp| (lamp.y - y).abs() < lamp_clearance);
        if !under_lamp {
            for x_sign in [-1.0, 1.0] {
                positions.push(Vec3::new(x_sign * pillar_x, y, pillar_front_z));
            }
        }
        y += BOLT_SPACING;
    }

    let inset = 0.5;
    for x_sign in [-1.0, 1.0] {
        for y_sign in [-1.0, 1.0] {
            positions.push(Vec3::new(
                x_sign * (lintel_size.x * 0.5 - inset),
                lintel_center.y + y_sign * (lintel_size.y * 0.5 - inset),
                lintel_front_z,
            ));
        }
    }

    positions
}

/// A domed bolt head standing just off a fascia face: a soft drop shadow under
/// a small face that is lit across its crown and shadowed at its skirt.
fn draw_bolt(center: Vec3, lights: &SceneLights) {
    let half = BOLT_SIZE * 0.5;
    let shadow_offset = Vec3::new(0.06, -0.06, 0.0);
    let top_left = Vec3::new(center.x - half, center.y + half, center.z + BOLT_DEPTH);
    let base = lit(BOLT_COLOR, lights.at(center));

    draw_quad(
        top_left + shadow_offset + Vec3::Z * 0.002,
        Vec3::X * BOLT_SIZE,
        Vec3::NEG_Y * BOLT_SIZE,
        None,
        [color_u8!(8, 8, 7, 200); 4],
    );
    draw_quad(
        top_left + Vec3::Z * 0.004,
        Vec3::X * BOLT_SIZE,
        Vec3::NEG_Y * BOLT_SIZE,
        None,
        [
            shaded(base, 1.5, 1.0),
            shaded(base, 1.3, 1.0),
            shaded(base, 0.55, 1.0),
            shaded(base, 0.7, 1.0),
        ],
    );
}

/// Draw a caged bulkhead lamp housing with its emissive face. The translucent
/// glow around it is drawn separately by [`draw_lamp_glow`], after everything
/// opaque, so its depth writes cannot clip blocks in the well's top corners.
fn draw_lamp(position: Vec3, lights: &SceneLights) {
    let housing_center = Vec3::new(
        position.x,
        position.y,
        BEZEL_FRONT_Z + LAMP_HOUSING_SIZE.z * 0.5,
    );
    let housing = lit(LAMP_HOUSING_COLOR, lights.at(position));
    draw_shaded_box(housing_center, LAMP_HOUSING_SIZE, housing, 1.0);
    draw_cube_wires(housing_center, LAMP_HOUSING_SIZE, BEZEL_SEAM_COLOR);

    let face_size = Vec2::new(LAMP_HOUSING_SIZE.x - 0.18, LAMP_HOUSING_SIZE.y - 0.18);
    let face_z = housing_center.z + LAMP_HOUSING_SIZE.z * 0.5 + 0.004;
    let face_top_left = Vec3::new(
        position.x - face_size.x * 0.5,
        position.y + face_size.y * 0.5,
        face_z,
    );
    // The lens runs from a near-white filament at the top to deep amber glass
    // at the bottom, so it reads as a hot bulb rather than a flat swatch.
    let glow = lights.lamp_glow();
    let hot = mix_color(glow, WHITE, 0.4);
    let deep = mix_color(glow, Color::new(1.0, 0.5, 0.12, 1.0), 0.6);
    draw_quad(
        face_top_left,
        Vec3::X * face_size.x,
        Vec3::NEG_Y * face_size.y,
        None,
        [hot, hot, deep, deep],
    );

    // Two cage bars across the lens.
    for bar in 1..3 {
        let x = face_top_left.x + face_size.x * (bar as f32 / 3.0);
        draw_line_3d(
            Vec3::new(x, face_top_left.y, face_z + 0.004),
            Vec3::new(x, face_top_left.y - face_size.y, face_z + 0.004),
            LAMP_CAGE_COLOR,
        );
    }
}

/// A soft radial light pool facing the camera: a fan of triangles fading from
/// `alpha` at the centre to fully transparent at `radius`. Under the palette
/// quantisation this dithers into the stepped halos of a software lightmap.
pub fn draw_glow_disc(center: Vec3, radius: f32, color: Color, alpha: f32) {
    draw_glow_fan(center, Vec3::X, Vec3::Y, radius, color, alpha);
}

/// [`draw_glow_disc`] laid flat on a horizontal surface such as the lava.
pub fn draw_glow_disc_on_plane(center: Vec3, radius: f32, color: Color, alpha: f32) {
    draw_glow_fan(center, Vec3::X, Vec3::Z, radius, color, alpha);
}

fn draw_glow_fan(center: Vec3, axis_a: Vec3, axis_b: Vec3, radius: f32, color: Color, alpha: f32) {
    const SEGMENTS: usize = 16;
    let mut vertices = Vec::with_capacity(SEGMENTS + 1);
    let mut indices = Vec::with_capacity(SEGMENTS * 3);
    vertices.push(Vertex::new2(
        center,
        Vec2::ZERO,
        Color::new(color.r, color.g, color.b, alpha),
    ));

    for segment in 0..SEGMENTS {
        let angle = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let rim = center + (axis_a * angle.cos() + axis_b * angle.sin()) * radius;
        vertices.push(Vertex::new2(
            rim,
            Vec2::ZERO,
            Color::new(color.r, color.g, color.b, 0.0),
        ));
        indices.extend_from_slice(&[
            0,
            1 + segment as u16,
            1 + ((segment + 1) % SEGMENTS) as u16,
        ]);
    }

    draw_mesh(&Mesh {
        vertices,
        indices,
        texture: None,
    });
}

/// Translucent light pools around the lamps. Must be drawn after every opaque
/// element of the 3D pass.
pub fn draw_lamp_glow(lights: &SceneLights) {
    let glow = lights.lamp_glow();

    for position in SceneLights::lamp_positions() {
        let center = position + Vec3::Z * 0.02;
        draw_glow_disc(center, 2.6, glow, 0.22);
        draw_glow_disc(center, 1.2, glow, 0.20);
    }
}

/// The bottom bar fronts the pit: a solid steel post at either end, and
/// between them an open grille looking straight in at the melt. Bars are two
/// pixels wide on a four-pixel pitch so they land on whole framebuffer pixels
/// and line up with the floor grate above. Horizontal ribs brace the bars at
/// the floor line and at every unit below it.
fn draw_furnace_front(
    center: Vec3,
    size: Vec3,
    textures: &SceneTextures,
    lights: &SceneLights,
) {
    let pixel = 1.0 / CELL_PIXEL_PITCH;
    let top = center.y + size.y * 0.5;
    let bottom = center.y - size.y * 0.5;
    let min_x = center.x - size.x * 0.5;
    let max_x = center.x + size.x * 0.5;
    let front_z = center.z + size.z * 0.5;
    let post_width = BEZEL_FASCIA_WIDTH + BEZEL_BEVEL_WIDTH;

    for post_x in [min_x, max_x - post_width] {
        let post_center = Vec3::new(post_x + post_width * 0.5, center.y, center.z);
        let post_size = Vec3::new(post_width, size.y, size.z);
        draw_shaded_box(post_center, post_size, BEZEL_FRAME_COLOR, 1.0);
        draw_cube_wires(post_center, post_size, BEZEL_SEAM_COLOR);
        draw_tiled_front_face(
            post_center,
            post_size,
            textures.gunmetal(),
            BEZEL_TEXTURE_COLOR,
            0,
            lights,
        );
        draw_front_face_outline(post_center, post_size, BEZEL_SEAM_COLOR);
    }

    // The grille bars stand against the glow behind them, so they read dark:
    // the camera sees their unlit fronts, edged by a thin rim of lava light.
    let span_half = ((size.x * 0.5 - post_width) * CELL_PIXEL_PITCH).floor() * pixel;
    let grate = lit(GRATE_COLOR, lights.at(Vec3::new(center.x, top, front_z)));
    let bar_face = shaded(grate, 0.55, 1.0);
    let bar_rim = shaded(grate, 1.1, 1.0);
    let bar_z = front_z + 0.006;

    let bar_pitch = 6.0 * pixel;
    let bar_width = 2.0 * pixel;
    let bar_count = (span_half * 2.0 / bar_pitch).floor() as usize;
    let bars_left = center.x - bar_pitch * bar_count as f32 * 0.5 + pixel * 2.0;
    for bar in 0..bar_count {
        let x = bars_left + bar as f32 * bar_pitch;
        draw_quad(
            Vec3::new(x, top, bar_z),
            Vec3::X * bar_width,
            Vec3::NEG_Y * size.y,
            None,
            [bar_rim, bar_face, bar_face, bar_rim],
        );
    }

    // A rib braces the bars at the floor line and another at the melt.
    for rib_top in [top, (LAVA_Y + 0.35).max(bottom + 0.2)] {
        draw_quad(
            Vec3::new(center.x - span_half, rib_top, bar_z + 0.002),
            Vec3::X * span_half * 2.0,
            Vec3::NEG_Y * (2.0 * pixel),
            None,
            [bar_rim, bar_rim, bar_face, bar_face],
        );
    }
}

/// Draw the chamfered sides between the cabinet face and the playfield
/// opening. The opening is left open at the top, so pieces visibly enter a
/// chute, and open at the bottom, where the floor grate hands over to the pit.
fn draw_bezel_throat(half_w: f32, half_h: f32, lights: &SceneLights) {
    let depth = BEZEL_FRONT_Z - BEZEL_THROAT_Z;
    let rim_top = half_h + BEZEL_LINTEL_CLEARANCE;
    let rim_bottom = LAVA_Y;

    // The side chamfers run the full height of the pillars, up past the
    // visible rows to the lintel and down the pit to the melt, and are lit one
    // row at a time so the glow pools at their feet instead of fading linearly
    // up the whole throat.
    let mut y_top = rim_top;
    while y_top > rim_bottom + 0.0001 {
        let height = (y_top - rim_bottom).min(1.0);
        for (x_sign, side_shade) in [(-1.0, 1.0), (1.0, 0.78)] {
            let inner_top = Vec3::new(x_sign * half_w, y_top, BEZEL_THROAT_Z);
            draw_lit_quad(
                inner_top,
                Vec3::new(x_sign * BEZEL_BEVEL_WIDTH, 0.0, depth),
                Vec3::NEG_Y * height,
                None,
                shaded(BEZEL_SIDE_COLOR, side_shade, 1.0),
                lights,
            );
        }
        y_top -= height;
    }

    // A lit rim down each side of the opening, where the chamfer meets the
    // block plane.
    let edge_z = BEZEL_THROAT_Z + 0.004;
    for x_sign in [-1.0, 1.0] {
        draw_line_3d(
            Vec3::new(x_sign * half_w, rim_bottom, edge_z),
            Vec3::new(x_sign * half_w, rim_top, edge_z),
            BEZEL_EDGE_COLOR,
        );
    }
}

/// Draw the cabinet: a sloped throat into the well, riveted steel pillars
/// carrying a heavy lintel, the furnace grille along the bottom, and a caged
/// lamp on each pillar. The throat stays open between lintel and well so
/// pieces visibly drop out of the machine rather than appear behind glass.
fn draw_well_bezel(half_w: f32, half_h: f32, textures: &SceneTextures, lights: &SceneLights) {
    draw_bezel_throat(half_w, half_h, lights);

    for (index, (center, size)) in bezel_fascia_boxes(half_w, half_h).into_iter().enumerate() {
        // The bottom bar is open grille between its end posts, so it draws
        // its own solids rather than one box that would wall off the pit.
        if index == FASCIA_BOTTOM {
            draw_furnace_front(center, size, textures, lights);
            continue;
        }

        draw_shaded_box(center, size, BEZEL_FRAME_COLOR, 1.0);
        draw_cube_wires(center, size, BEZEL_SEAM_COLOR);
        draw_tiled_front_face(
            center,
            size,
            textures.gunmetal(),
            BEZEL_TEXTURE_COLOR,
            index,
            lights,
        );
        draw_front_face_outline(center, size, BEZEL_SEAM_COLOR);
    }

    for bolt in bolt_positions(half_w, half_h) {
        draw_bolt(bolt, lights);
    }

    for lamp in SceneLights::lamp_positions() {
        draw_lamp(lamp, lights);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_block_shows_exactly_its_front_and_top_faces() {
        let visible = visible_block_faces();

        assert!(visible[FACE_FRONT]);
        assert!(visible[FACE_TOP]);
        assert!(!visible[FACE_BACK]);
        assert!(!visible[FACE_BOTTOM]);
        assert!(!visible[FACE_LEFT]);
        assert!(!visible[FACE_RIGHT]);
    }

    #[test]
    fn blocks_omit_edges_owned_only_by_hidden_faces() {
        let faces = visible_block_faces();
        let edges = visible_block_edges(faces);

        assert_eq!(edges.iter().filter(|&&visible| visible).count(), 7);
        for hidden_edge in [0, 1, 3, 8, 9] {
            assert!(!edges[hidden_edge]);
        }
    }

    #[test]
    fn camera_looks_down_into_the_well_without_yaw() {
        let view = view_direction();

        assert_eq!(view.x, 0.0);
        assert!(view.y > 0.1, "the camera should be pitched down, not level");
        assert!(view.z > 0.95, "but only modestly, so the board stays front-on");
    }

    #[test]
    fn top_faces_are_a_few_pixels_tall_on_screen() {
        let front_lip = world_to_screen(Vec3::new(0.0, 0.5, 0.5));
        let back_lip = world_to_screen(Vec3::new(0.0, 0.5, -0.5));
        let height = front_lip.y - back_lip.y;

        assert!(height > 2.0 && height < 4.0, "top face height {height}px");
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
    fn a_tumbling_cube_always_shows_exactly_three_faces() {
        let view = view_direction();

        for euler in [
            Vec3::new(0.5, 0.8, 0.3),
            Vec3::new(2.1, -0.7, 1.9),
            Vec3::new(-1.3, 0.4, 2.6),
            Vec3::new(4.2, 3.3, 0.35),
        ] {
            let rotation = Quat::from_euler(EulerRot::XYZ, euler.x, euler.y, euler.z);
            let normals = rotated_face_normals(rotation);

            for pair in 0..3 {
                let sum = normals[pair * 2] + normals[pair * 2 + 1];
                assert!(sum.length() < 0.000_01, "opposite faces should oppose: {euler}");
            }

            // A convex cube seen from any generic direction shows exactly
            // three faces; pairs culled together would give an even count.
            let visible = normals.iter().filter(|normal| normal.dot(view) > 0.0).count();
            assert_eq!(visible, 3, "rotation {euler}");
        }
    }

    #[test]
    fn screen_to_world_round_trips_through_world_to_screen() {
        for point in [
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(-7.3, 11.2, -1.0),
            Vec3::new(9.0, -12.5, 0.5),
        ] {
            let screen = world_to_screen(point);
            let back = screen_to_world_on_plane(screen, point.z);

            assert!((back - point).length() < 0.001, "{point} -> {screen} -> {back}");
        }
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
    fn bezel_fascia_forms_a_gapless_frame_with_an_open_throat() {
        let [bottom, left, right, lintel] = bezel_fascia_boxes(WELL_WIDTH / 2.0, WELL_HEIGHT / 2.0);
        let (bottom_min, bottom_max) = (bottom.0 - bottom.1 * 0.5, bottom.0 + bottom.1 * 0.5);
        let (left_min, left_max) = (left.0 - left.1 * 0.5, left.0 + left.1 * 0.5);
        let (right_min, right_max) = (right.0 - right.1 * 0.5, right.0 + right.1 * 0.5);
        let (lintel_min, lintel_max) = (lintel.0 - lintel.1 * 0.5, lintel.0 + lintel.1 * 0.5);

        const EPSILON: f32 = 0.000_001;
        assert!((bottom_max.y - left_min.y).abs() < EPSILON);
        assert!((bottom_max.y - right_min.y).abs() < EPSILON);
        assert!((bottom_min.x - left_min.x).abs() < EPSILON);
        assert!((bottom_max.x - right_max.x).abs() < EPSILON);
        assert!((left_max.y - right_max.y).abs() < EPSILON);
        assert!((left_max.y - lintel_min.y).abs() < EPSILON);
        assert!((lintel_min.x - left_min.x).abs() < EPSILON);
        assert!((lintel_max.x - right_max.x).abs() < EPSILON);

        // The lintel sits clear of the visible well, so a piece is never
        // hidden behind it once it has entered play.
        assert!(lintel_min.y >= WELL_HEIGHT * 0.5);

        // Backs are flush while the lintel's front projects ahead of the pillars.
        assert!((lintel_min.z - left_min.z).abs() < EPSILON);
        assert!(lintel_max.z > left_max.z);
    }

    #[test]
    fn blocks_sit_just_behind_a_narrow_rim() {
        assert!(BEZEL_THROAT_Z > BLOCK_INSET * 0.5);
        assert!(BEZEL_FRONT_Z > BEZEL_THROAT_Z);

        // The frame's front is within a quarter unit of the block faces, and
        // the chamfer between them is a lip, not a ledge.
        assert!(BEZEL_FRONT_Z - BLOCK_INSET * 0.5 < 0.25);
        assert!(BEZEL_BEVEL_WIDTH < 0.2);
    }

    #[test]
    fn pieces_emerge_through_an_open_slot_under_the_lintel() {
        let (lintel_top_left, lintel_bottom_right) = lintel_front_bounds();
        let lintel_bottom_edge =
            world_to_screen(Vec3::new(0.0, lintel_bottom_right.y, lintel_bottom_right.z)).y;
        let top_row_top_edge =
            world_to_screen(Vec3::new(0.0, WELL_HEIGHT * 0.5, BLOCK_INSET * 0.5)).y;

        // Screen y grows downward: the lintel's lower edge must project a few
        // pixels above the top row, leaving a visible slot to drop through.
        let slot = top_row_top_edge - lintel_bottom_edge;
        assert!(slot > 3.0, "slot {slot}px");

        // The shaft's back wall shows through that slot but stops short of the
        // lintel's top, so it never peeks out above the frame.
        let wall_top = world_to_screen(Vec3::new(
            0.0,
            shaft_top_y(WELL_HEIGHT * 0.5),
            BACK_WALL_Z,
        ))
        .y;
        let lintel_top_back = world_to_screen(Vec3::new(
            0.0,
            lintel_top_left.y,
            BEZEL_FRONT_Z - BEZEL_BOX_DEPTH,
        ))
        .y;
        assert!(wall_top <= lintel_bottom_edge, "wall must fill the slot");
        assert!(wall_top >= lintel_top_back, "wall must hide behind the lintel");
    }

    #[test]
    fn bolts_sit_on_the_fascia_front_and_never_overlap_the_lamps() {
        let bolts = bolt_positions(WELL_WIDTH / 2.0, WELL_HEIGHT / 2.0);
        let [_, left, _, lintel] = bezel_fascia_boxes(WELL_WIDTH / 2.0, WELL_HEIGHT / 2.0);
        let pillar_front_z = left.0.z + left.1.z * 0.5;
        let lintel_front_z = lintel.0.z + lintel.1.z * 0.5;

        assert!(bolts.len() >= 12);
        for bolt in &bolts {
            assert!(
                (bolt.z - pillar_front_z).abs() < 0.001 || (bolt.z - lintel_front_z).abs() < 0.001
            );
            for lamp in SceneLights::lamp_positions() {
                let clearance = (bolt.truncate() - lamp.truncate()).length();
                assert!(clearance > BOLT_SIZE + LAMP_HOUSING_SIZE.y * 0.5);
            }
        }
    }

    #[test]
    fn lintel_front_bounds_span_the_fascia_above_the_well() {
        let (top_left, bottom_right) = lintel_front_bounds();

        assert!(top_left.x < -WELL_WIDTH * 0.5);
        assert!(bottom_right.x > WELL_WIDTH * 0.5);
        assert!(bottom_right.y >= WELL_HEIGHT * 0.5);
        assert!(top_left.y > bottom_right.y);
        assert!(top_left.z > BEZEL_FRONT_Z);
    }

    #[test]
    fn shrapnel_voxel_defaults_to_inactive_and_airborne() {
        let voxel = ShrapnelVoxel::default();
        assert!(!voxel.active);
        assert_eq!(voxel.age, 0.0);
        assert!(!voxel.is_sinking());
    }

    #[test]
    fn floor_grate_bars_are_pixel_aligned_and_leave_gaps_to_fall_through() {
        let pixel = 1.0 / CELL_PIXEL_PITCH;
        let bar_width = FLOOR_BAR_WIDTH_PIXELS * pixel;
        let edges: Vec<f32> = floor_bar_left_edges().collect();

        assert!(edges.len() > 30, "{} bars", edges.len());
        for pair in edges.windows(2) {
            let gap = pair[1] - (pair[0] + bar_width);
            assert!((gap - (FLOOR_BAR_PITCH_PIXELS - FLOOR_BAR_WIDTH_PIXELS) * pixel).abs() < 1e-5);
        }

        // Every bar starts on a whole framebuffer pixel relative to the
        // well's centre line, and is symmetric about it.
        for &left in &edges {
            let offset_pixels = left * CELL_PIXEL_PITCH;
            assert!((offset_pixels - offset_pixels.round()).abs() < 1e-3, "{left}");
        }
        assert!((edges[0] + bar_width + edges[edges.len() - 1]).abs() < 1e-4);

        // A point over a bar is caught; the midpoint of the next gap is not.
        let first = edges[0];
        assert!(!floor_gap_contains(first + bar_width * 0.5));
        assert!(floor_gap_contains(first + bar_width + pixel));
    }

    #[test]
    fn the_pit_is_fronted_by_the_grille_and_floored_by_lava() {
        let [bottom, left, _, _] = bezel_fascia_boxes(WELL_WIDTH / 2.0, WELL_HEIGHT / 2.0);
        let bottom_top = bottom.0.y + bottom.1.y * 0.5;
        let bottom_bottom = bottom.0.y - bottom.1.y * 0.5;
        let left_bottom = left.0.y - left.1.y * 0.5;

        assert!((bottom_top - FLOOR_Y).abs() < 1e-5, "grille starts at the floor grate");
        assert!(bottom_bottom < LAVA_Y, "grille reaches below the melt");
        assert!((left_bottom - bottom_top).abs() < 1e-5, "pillars stand on the grille");
        assert!(LAVA_Y < FLOOR_Y - 1.0, "there is a real drop into the pit");
    }
}
