//! Procedural material textures for the well, its blocks and the backdrop.
//!
//! Every texture is authored at the exact texel density it is displayed at: a
//! block's front face is 16 framebuffer pixels wide, so its material is 16x16
//! and one texel lands on one pixel. Drawing a larger hand-painted source
//! through a point sampler at this size turns its detail into shimmer; at
//! native size the bevels, rivets and vent slots stay crisp and read as
//! deliberate pixel art. All materials are neutral grey so they can be tinted
//! by the piece palette and scene lighting at draw time.
//!
//! Everything is packed into one atlas texture: the materials, a plain white
//! swatch for untextured geometry, and the glyphs of both bitmap fonts.
//! macroquad starts a new GPU batch whenever the texture changes, so with a
//! single texture a whole frame can go to the GPU in a handful of batches
//! instead of one per block.

use macroquad::prelude::*;

use crate::pixel_font::{
    digit_glyph, small_glyph, DIGIT_GLYPH_CHARS, DIGIT_HEIGHT, SMALL_GLYPH_CHARS,
    SMALL_GLYPH_HEIGHT, SMALL_GLYPH_WIDTH,
};

/// Edge length of a block face material, in texels.
pub const BLOCK_TEXTURE_SIZE: usize = 16;

/// Edge length of the seamless stone tile used behind the cabinet.
pub const STONE_TEXTURE_SIZE: usize = 32;

/// Edge length of the atlas, in texels. Plenty for the four materials, the
/// white swatch and about sixty small glyphs.
const ATLAS_SIZE: usize = 128;

/// Empty texels between packed regions. Nearest sampling inside a region never
/// reaches its edge, but the gutter keeps rounding at the very edge from ever
/// picking up a neighbour.
const ATLAS_GUTTER: usize = 1;

/// Edge length of the white swatch. Untextured quads sample its centre, so a
/// few texels is more than enough.
const WHITE_SWATCH_SIZE: usize = 4;

/// A region of the atlas, in texture coordinates.
#[derive(Copy, Clone, Debug, PartialEq)]
struct UvRect {
    min: Vec2,
    max: Vec2,
}

impl UvRect {
    /// Map a point in the region's own 0-1 space into the atlas.
    fn at(&self, local: Vec2) -> Vec2 {
        self.min + (self.max - self.min) * local
    }
}

/// A drawable material: a region of the atlas together with the texture it
/// lives in, so a draw call can bind the texture and map its own 0-1 texture
/// coordinates into the region.
#[derive(Copy, Clone, Debug)]
pub struct Material<'a> {
    texture: &'a Texture2D,
    region: UvRect,
}

impl<'a> Material<'a> {
    pub fn texture(&self) -> &'a Texture2D {
        self.texture
    }

    /// Map a point in the material's own 0-1 texture space into the atlas.
    pub fn uv(&self, local: Vec2) -> Vec2 {
        self.region.at(local)
    }
}

/// Packs regions into the atlas in shelves: left to right, then a new shelf
/// below the tallest region of the last one.
struct AtlasBuilder {
    texels: Vec<[u8; 4]>,
    cursor_x: usize,
    cursor_y: usize,
    shelf_height: usize,
}

impl AtlasBuilder {
    fn new() -> Self {
        Self {
            texels: vec![[0; 4]; ATLAS_SIZE * ATLAS_SIZE],
            cursor_x: 0,
            cursor_y: 0,
            shelf_height: 0,
        }
    }

    /// Reserve a `width` x `height` region, returning its top-left texel.
    fn reserve(&mut self, width: usize, height: usize) -> (usize, usize) {
        assert!(width <= ATLAS_SIZE && height <= ATLAS_SIZE);

        if self.cursor_x + width > ATLAS_SIZE {
            self.cursor_y += self.shelf_height + ATLAS_GUTTER;
            self.cursor_x = 0;
            self.shelf_height = 0;
        }
        assert!(
            self.cursor_y + height <= ATLAS_SIZE,
            "material atlas is full"
        );

        let origin = (self.cursor_x, self.cursor_y);
        self.cursor_x += width + ATLAS_GUTTER;
        self.shelf_height = self.shelf_height.max(height);

        origin
    }

    /// Pack a region whose texels are given by `texel(x, y)`.
    fn add(
        &mut self,
        width: usize,
        height: usize,
        texel: impl Fn(usize, usize) -> [u8; 4],
    ) -> UvRect {
        let (x0, y0) = self.reserve(width, height);

        for y in 0..height {
            for x in 0..width {
                self.texels[(y0 + y) * ATLAS_SIZE + x0 + x] = texel(x, y);
            }
        }

        let scale = 1.0 / ATLAS_SIZE as f32;
        UvRect {
            min: Vec2::new(x0 as f32, y0 as f32) * scale,
            max: Vec2::new((x0 + width) as f32, (y0 + height) as f32) * scale,
        }
    }

    /// Pack a greyscale canvas as an opaque material.
    fn add_canvas(&mut self, canvas: &Canvas) -> UvRect {
        self.add(canvas.size, canvas.size, |x, y| {
            let grey = (canvas.get(x, y) * 255.0).round() as u8;
            [grey, grey, grey, 255]
        })
    }

    /// Pack a one-bit glyph: white where `set`, transparent elsewhere, so the
    /// vertex colour alone decides what the glyph looks like on screen.
    fn add_bitmap(
        &mut self,
        width: usize,
        height: usize,
        set: impl Fn(usize, usize) -> bool,
    ) -> UvRect {
        self.add(width, height, |x, y| {
            if set(x, y) {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 0]
            }
        })
    }

    fn into_texture(self) -> Texture2D {
        let bytes: Vec<u8> = self.texels.iter().flatten().copied().collect();
        let texture = Texture2D::from_rgba8(ATLAS_SIZE as u16, ATLAS_SIZE as u16, &bytes);
        texture.set_filter(FilterMode::Nearest);
        texture
    }
}

/// Brightness of a material's flat, undamaged surface. Chosen so a front face
/// tinted at [`crate::render3d`]'s front shade lands close to the palette colour
/// rather than a muddied version of it.
const BASE_BRIGHTNESS: f32 = 0.80;

/// Deterministic integer hash in `[0, 1)`. Textures are regenerated identically
/// on every launch, so nothing needs to be cached or shipped as an asset.
fn hash01(x: usize, y: usize, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA77)
        ^ seed.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

/// A rectangle of texels, by top-left corner and size.
#[derive(Copy, Clone, Debug)]
struct TexelRect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl TexelRect {
    const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    const fn square(x: usize, y: usize, size: usize) -> Self {
        Self::new(x, y, size, size)
    }
}

/// Square greyscale canvas, edited in place by the generators below and
/// uploaded once as an opaque, nearest-filtered texture.
#[derive(Clone)]
pub struct Canvas {
    size: usize,
    value: Vec<f32>,
}

impl Canvas {
    fn new(size: usize, fill: f32) -> Self {
        Self {
            size,
            value: vec![fill; size * size],
        }
    }

    fn index(&self, x: usize, y: usize) -> usize {
        (y % self.size) * self.size + (x % self.size)
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.value[self.index(x, y)]
    }

    fn set(&mut self, x: usize, y: usize, value: f32) {
        let index = self.index(x, y);
        self.value[index] = value.clamp(0.0, 1.0);
    }

    fn add(&mut self, x: usize, y: usize, delta: f32) {
        let current = self.get(x, y);
        self.set(x, y, current + delta);
    }

    fn fill_rect(&mut self, rect: TexelRect, value: f32) {
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                self.set(x, y, value);
            }
        }
    }

    /// Per-texel grain. Low amplitude keeps it reading as surface wear rather
    /// than static once the palette quantisation lands on top.
    fn grain(&mut self, amplitude: f32, seed: u32) {
        for y in 0..self.size {
            for x in 0..self.size {
                let delta = (hash01(x, y, seed) - 0.5) * 2.0 * amplitude;
                self.add(x, y, delta);
            }
        }
    }

    /// Blocky grain at `cell` texel resolution, giving broad grime patches that
    /// survive reduction far better than single-texel noise.
    fn coarse_grain(&mut self, cell: usize, amplitude: f32, seed: u32) {
        for y in 0..self.size {
            for x in 0..self.size {
                let delta = (hash01(x / cell, y / cell, seed) - 0.5) * 2.0 * amplitude;
                self.add(x, y, delta);
            }
        }
    }

    /// Smooth, seamlessly tiling value noise on a `lattice`-cell grid.
    fn tiling_noise(&mut self, lattice: usize, amplitude: f32, seed: u32) {
        let cell = self.size / lattice;
        for y in 0..self.size {
            for x in 0..self.size {
                let gx = x / cell;
                let gy = y / cell;
                let fx = (x % cell) as f32 / cell as f32;
                let fy = (y % cell) as f32 / cell as f32;
                let sample = |lx: usize, ly: usize| hash01(lx % lattice, ly % lattice, seed);
                let top = sample(gx, gy) + (sample(gx + 1, gy) - sample(gx, gy)) * fx;
                let bottom =
                    sample(gx, gy + 1) + (sample(gx + 1, gy + 1) - sample(gx, gy + 1)) * fx;
                let value = top + (bottom - top) * fy;

                self.add(x, y, (value - 0.5) * 2.0 * amplitude);
            }
        }
    }

    /// One-texel chamfer around a rectangle. A raised edge is lit on its top
    /// and left and shadowed on its bottom and right; `recessed` swaps them.
    fn bevel(&mut self, rect: TexelRect, light: f32, dark: f32, recessed: bool) {
        let (top_left, bottom_right) = if recessed {
            (dark, light)
        } else {
            (light, dark)
        };
        let x0 = rect.x;
        let y0 = rect.y;
        let x1 = rect.x + rect.width - 1;
        let y1 = rect.y + rect.height - 1;

        for x in x0..=x1 {
            self.set(x, y0, top_left);
            self.set(x, y1, bottom_right);
        }
        for y in y0..=y1 {
            self.set(x0, y, top_left);
            self.set(x1, y, bottom_right);
        }

        // The lit and shadowed edges meet at the off-diagonal corners; a
        // mid-tone there keeps the chamfer from reading as a torn corner.
        let corner = (light + dark) * 0.5;
        self.set(x1, y0, corner);
        self.set(x0, y1, corner);
    }

    /// A 2x2 domed fastener: lit crown, shadowed skirt.
    fn rivet(&mut self, x: usize, y: usize) {
        self.set(x, y, 1.0);
        self.set(x + 1, y, 0.72);
        self.set(x, y + 1, 0.72);
        self.set(x + 1, y + 1, 0.36);
    }

    /// Mean brightness, used to keep the materials' overall tint consistent
    /// with one another.
    #[cfg(test)]
    pub fn mean_brightness(&self) -> f32 {
        self.value.iter().sum::<f32>() / self.value.len() as f32
    }
}

/// Riveted armour plate: a two-texel chamfer, a recessed centre panel and four
/// corner fasteners.
pub fn armor_plate() -> Canvas {
    let mut canvas = Canvas::new(BLOCK_TEXTURE_SIZE, BASE_BRIGHTNESS);
    let size = BLOCK_TEXTURE_SIZE;
    let recess = TexelRect::square(4, 4, 8);
    canvas.coarse_grain(2, 0.05, 11);
    canvas.grain(0.025, 12);

    canvas.bevel(TexelRect::square(0, 0, size), 1.0, 0.38, false);
    canvas.bevel(TexelRect::square(1, 1, size - 2), 0.90, 0.58, false);

    canvas.fill_rect(recess, BASE_BRIGHTNESS - 0.07);
    canvas.coarse_grain(2, 0.03, 13);
    canvas.bevel(recess, 0.92, 0.50, true);

    for (x, y) in [(2, 2), (12, 2), (2, 12), (12, 12)] {
        canvas.rivet(x, y);
    }

    // A couple of chips out of the plate so no two edges look machine-clean.
    canvas.set(7, 13, 0.50);
    canvas.set(13, 6, 0.52);
    canvas.set(6, 6, 0.62);

    canvas
}

/// Stamped vent panel: the same chamfered plate with three louvred slots and
/// a pair of side fasteners.
pub fn vent_panel() -> Canvas {
    let mut canvas = Canvas::new(BLOCK_TEXTURE_SIZE, BASE_BRIGHTNESS);
    let size = BLOCK_TEXTURE_SIZE;
    canvas.coarse_grain(2, 0.05, 21);
    canvas.grain(0.025, 22);

    canvas.bevel(TexelRect::square(0, 0, size), 1.0, 0.38, false);
    canvas.bevel(TexelRect::square(1, 1, size - 2), 0.90, 0.58, false);

    // Each louvre is a dark slot with a lit lower lip, the way stamped steel
    // catches light from above.
    for slot_y in [4, 7, 10] {
        canvas.fill_rect(TexelRect::new(4, slot_y, 8, 1), 0.20);
        canvas.fill_rect(TexelRect::new(4, slot_y + 1, 8, 1), 0.96);
        canvas.set(4, slot_y, 0.30);
        canvas.set(11, slot_y + 1, 0.80);
    }

    canvas.rivet(2, 7);
    canvas.rivet(12, 7);
    canvas.set(9, 13, 0.52);

    canvas
}

/// Seamless worn gunmetal for the cabinet fascia and HUD housings.
pub fn gunmetal() -> Canvas {
    let mut canvas = Canvas::new(BLOCK_TEXTURE_SIZE, 0.60);
    canvas.tiling_noise(4, 0.14, 31);
    canvas.coarse_grain(2, 0.05, 32);
    canvas.grain(0.035, 33);

    // Sparse pitting and a few bright chips in the finish.
    for y in 0..BLOCK_TEXTURE_SIZE {
        for x in 0..BLOCK_TEXTURE_SIZE {
            let roll = hash01(x, y, 34);
            if roll > 0.955 {
                canvas.add(x, y, -0.22);
            } else if roll < 0.03 {
                canvas.add(x, y, 0.18);
            }
        }
    }

    canvas
}

/// Seamless cut-stone masonry: two courses of 32x16 blocks per tile, offset by
/// half a block, with dark mortar and a lit upper edge on every block.
pub fn stone_blocks() -> Canvas {
    let size = STONE_TEXTURE_SIZE;
    let course_height = size / 2;
    let mut canvas = Canvas::new(size, 0.58);
    canvas.tiling_noise(4, 0.08, 41);
    canvas.coarse_grain(2, 0.045, 42);
    canvas.grain(0.03, 43);

    for course in 0..2 {
        let y0 = course * course_height;
        let offset = if course == 0 { 0 } else { size / 2 };
        let block_tone = (hash01(course, 0, 44) - 0.5) * 0.08;

        for y in y0..y0 + course_height {
            for x in 0..size {
                canvas.add(x, y, block_tone);
            }
        }

        // Mortar runs along the bottom of the course and down one vertical
        // joint per course; both wrap so the tile repeats seamlessly.
        for x in 0..size {
            canvas.set(x, y0 + course_height - 1, 0.30);
            canvas.set(x, y0 + course_height - 2, 0.40);
            canvas.set(x, y0, 0.70);
        }
        let joint_x = (offset + size - 1) % size;
        for y in y0..y0 + course_height {
            canvas.set(joint_x, y, 0.30);
            canvas.set((joint_x + 1) % size, y, 0.66);
            canvas.set((joint_x + size - 1) % size, y, 0.44);
        }
    }

    canvas
}

/// Glyph regions indexed by ASCII code.
type GlyphRegions = [Option<UvRect>; 128];

fn glyph_index(character: char) -> Option<usize> {
    let index = character as usize;
    (index < 128).then_some(index)
}

/// Every material the renderer needs, generated once at startup into a single
/// atlas texture.
pub struct SceneTextures {
    atlas: Texture2D,
    armor: UvRect,
    vent: UvRect,
    gunmetal: UvRect,
    stone: UvRect,
    white: UvRect,
    small_glyphs: GlyphRegions,
    digit_glyphs: GlyphRegions,
}

impl SceneTextures {
    pub fn new() -> Self {
        let mut builder = AtlasBuilder::new();
        let stone = builder.add_canvas(&stone_blocks());
        let armor = builder.add_canvas(&armor_plate());
        let vent = builder.add_canvas(&vent_panel());
        let gunmetal = builder.add_canvas(&gunmetal());
        let white = builder.add_bitmap(WHITE_SWATCH_SIZE, WHITE_SWATCH_SIZE, |_, _| true);

        let mut small_glyphs: GlyphRegions = [None; 128];
        for character in SMALL_GLYPH_CHARS.chars() {
            let rows = small_glyph(character);
            let region = builder.add_bitmap(
                SMALL_GLYPH_WIDTH as usize,
                SMALL_GLYPH_HEIGHT as usize,
                |x, y| rows[y] & (1 << (SMALL_GLYPH_WIDTH as usize - 1 - x)) != 0,
            );
            small_glyphs[glyph_index(character).expect("small glyphs are ASCII")] = Some(region);
        }

        let mut digit_glyphs: GlyphRegions = [None; 128];
        for character in DIGIT_GLYPH_CHARS.chars() {
            let rows = digit_glyph(character).expect("every digit glyph character has a glyph");
            let region = builder.add_bitmap(rows[0].len(), DIGIT_HEIGHT, |x, y| {
                rows[y].as_bytes()[x] == b'#'
            });
            digit_glyphs[glyph_index(character).expect("digit glyphs are ASCII")] = Some(region);
        }

        Self {
            atlas: builder.into_texture(),
            armor,
            vent,
            gunmetal,
            stone,
            white,
            small_glyphs,
            digit_glyphs,
        }
    }

    fn material(&self, region: UvRect) -> Material<'_> {
        Material {
            texture: &self.atlas,
            region,
        }
    }

    /// Choose a stable block material from the piece colour. Position-based
    /// variation would make an active piece visibly swap textures as it moves.
    pub fn for_color(&self, color: Color) -> Material<'_> {
        let r = (color.r * 255.0).round() as u32;
        let g = (color.g * 255.0).round() as u32;
        let b = (color.b * 255.0).round() as u32;
        let signature = r * 3 + g * 5 + b * 7;

        if signature % 5 >= 3 {
            self.material(self.vent)
        } else {
            self.material(self.armor)
        }
    }

    pub fn gunmetal(&self) -> Material<'_> {
        self.material(self.gunmetal)
    }

    pub fn stone(&self) -> Material<'_> {
        self.material(self.stone)
    }

    /// Plain white, for geometry that is coloured by its vertices alone.
    pub fn white(&self) -> Material<'_> {
        self.material(self.white)
    }

    /// The 5x7 glyph for `character`, if the face has one. Lookups are
    /// case-insensitive, like the face itself.
    pub fn small_glyph(&self, character: char) -> Option<Material<'_>> {
        let index = glyph_index(character.to_ascii_uppercase())?;
        self.small_glyphs[index].map(|region| self.material(region))
    }

    /// The bold numeral glyph for `character`, if the face has one.
    pub fn digit_glyph(&self, character: char) -> Option<Material<'_>> {
        let index = glyph_index(character)?;
        self.digit_glyphs[index].map(|region| self.material(region))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_materials_share_a_common_brightness() {
        let armor = armor_plate().mean_brightness();
        let vent = vent_panel().mean_brightness();

        assert!((armor - vent).abs() < 0.06, "armor {armor} vs vent {vent}");
        assert!(armor > 0.65 && armor < 0.9);
    }

    #[test]
    fn block_materials_are_lit_from_the_top_left() {
        for canvas in [armor_plate(), vent_panel()] {
            let top_edge = canvas.get(8, 0);
            let bottom_edge = canvas.get(8, BLOCK_TEXTURE_SIZE - 1);
            let left_edge = canvas.get(0, 8);
            let right_edge = canvas.get(BLOCK_TEXTURE_SIZE - 1, 8);

            assert!(top_edge > bottom_edge + 0.3);
            assert!(left_edge > right_edge + 0.3);
        }
    }

    #[test]
    fn stone_courses_are_separated_by_dark_mortar() {
        let stone = stone_blocks();
        let mortar = stone.get(5, STONE_TEXTURE_SIZE / 2 - 1);
        let face = stone.get(5, STONE_TEXTURE_SIZE / 4);

        assert!(mortar < face - 0.2);
    }

    /// Texel bounds of a packed region.
    fn texel_bounds(region: UvRect) -> (usize, usize, usize, usize) {
        let scale = ATLAS_SIZE as f32;
        (
            (region.min.x * scale).round() as usize,
            (region.min.y * scale).round() as usize,
            (region.max.x * scale).round() as usize,
            (region.max.y * scale).round() as usize,
        )
    }

    #[test]
    fn atlas_packs_every_region_inside_the_texture_without_overlap() {
        let mut builder = AtlasBuilder::new();
        let mut regions = vec![
            builder.add_canvas(&stone_blocks()),
            builder.add_canvas(&armor_plate()),
            builder.add_canvas(&vent_panel()),
            builder.add_canvas(&gunmetal()),
            builder.add_bitmap(WHITE_SWATCH_SIZE, WHITE_SWATCH_SIZE, |_, _| true),
        ];
        for _ in SMALL_GLYPH_CHARS.chars() {
            regions.push(builder.add_bitmap(5, 7, |_, _| true));
        }
        for character in DIGIT_GLYPH_CHARS.chars() {
            let width = digit_glyph(character).unwrap()[0].len();
            regions.push(builder.add_bitmap(width, DIGIT_HEIGHT, |_, _| true));
        }

        let mut covered = vec![false; ATLAS_SIZE * ATLAS_SIZE];
        for region in regions {
            let (x0, y0, x1, y1) = texel_bounds(region);
            assert!(x1 <= ATLAS_SIZE && y1 <= ATLAS_SIZE, "{region:?} escapes the atlas");

            for y in y0..y1 {
                for x in x0..x1 {
                    assert!(!covered[y * ATLAS_SIZE + x], "{region:?} overlaps another");
                    covered[y * ATLAS_SIZE + x] = true;
                }
            }
        }
    }

    #[test]
    fn packed_canvas_keeps_its_texel_size_and_values() {
        let canvas = armor_plate();
        let mut builder = AtlasBuilder::new();
        let region = builder.add_canvas(&canvas);
        let (x0, y0, x1, y1) = texel_bounds(region);

        assert_eq!((x1 - x0, y1 - y0), (BLOCK_TEXTURE_SIZE, BLOCK_TEXTURE_SIZE));
        let texel = builder.texels[(y0 + 3) * ATLAS_SIZE + x0 + 3];
        assert_eq!(texel[0], (canvas.get(3, 3) * 255.0).round() as u8);
        assert_eq!(texel[3], 255);
    }

    #[test]
    fn packed_bitmap_is_white_where_set_and_transparent_elsewhere() {
        let mut builder = AtlasBuilder::new();
        let region = builder.add_bitmap(2, 1, |x, _| x == 0);
        let (x0, y0, _, _) = texel_bounds(region);

        assert_eq!(builder.texels[y0 * ATLAS_SIZE + x0], [255, 255, 255, 255]);
        assert_eq!(builder.texels[y0 * ATLAS_SIZE + x0 + 1], [0, 0, 0, 0]);
    }

    #[test]
    fn region_maps_local_texture_coordinates_into_the_atlas() {
        let region = UvRect {
            min: Vec2::new(0.25, 0.5),
            max: Vec2::new(0.5, 1.0),
        };

        assert_eq!(region.at(Vec2::ZERO), Vec2::new(0.25, 0.5));
        assert_eq!(region.at(Vec2::ONE), Vec2::new(0.5, 1.0));
        assert_eq!(region.at(Vec2::new(0.5, 0.5)), Vec2::new(0.375, 0.75));
    }
}
