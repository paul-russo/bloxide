//! Procedural material textures for the well, its blocks and the backdrop.
//!
//! Every texture is authored at the exact texel density it is displayed at: a
//! block's front face is 16 framebuffer pixels wide, so its material is 16x16
//! and one texel lands on one pixel. Drawing a larger hand-painted source
//! through a point sampler at this size turns its detail into shimmer; at
//! native size the bevels, rivets and vent slots stay crisp and read as
//! deliberate pixel art. All materials are neutral grey so they can be tinted
//! by the piece palette and scene lighting at draw time.

use macroquad::prelude::*;

/// Edge length of a block face material, in texels.
pub const BLOCK_TEXTURE_SIZE: usize = 16;

/// Edge length of the seamless stone tile used behind the cabinet.
pub const STONE_TEXTURE_SIZE: usize = 32;

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

    pub fn into_texture(self) -> Texture2D {
        let mut bytes = Vec::with_capacity(self.size * self.size * 4);
        for value in &self.value {
            let grey = (value * 255.0).round() as u8;
            bytes.extend_from_slice(&[grey, grey, grey, 255]);
        }

        let texture = Texture2D::from_rgba8(self.size as u16, self.size as u16, &bytes);
        texture.set_filter(FilterMode::Nearest);
        texture
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

/// Every material the renderer needs, generated once at startup.
#[derive(Clone)]
pub struct SceneTextures {
    armor: Texture2D,
    vent: Texture2D,
    gunmetal: Texture2D,
    stone: Texture2D,
}

impl SceneTextures {
    pub fn new() -> Self {
        Self {
            armor: armor_plate().into_texture(),
            vent: vent_panel().into_texture(),
            gunmetal: gunmetal().into_texture(),
            stone: stone_blocks().into_texture(),
        }
    }

    /// Choose a stable block material from the piece colour. Position-based
    /// variation would make an active piece visibly swap textures as it moves.
    pub fn for_color(&self, color: Color) -> &Texture2D {
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

    pub fn gunmetal(&self) -> &Texture2D {
        &self.gunmetal
    }

    pub fn stone(&self) -> &Texture2D {
        &self.stone
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
}
