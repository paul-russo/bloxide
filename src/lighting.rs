//! Scene lighting, evaluated on the CPU and baked into vertex colours.
//!
//! macroquad's fixed pipeline has no lights, so the renderer fakes lightmaps:
//! every large surface is split into tiles and each tile's corners are tinted
//! by [`SceneLights::at`]. Two sources light the cabinet: a pair of caged lamps
//! mounted high on the pillars, and the pool of molten metal at the bottom of
//! the pit under the well. Both flicker on id Software's original lightstyle
//! strings, sampled at the same ten steps per second Quake used, so the whole
//! scene breathes together.

use macroquad::prelude::*;

use crate::render3d::{
    BEZEL_FRONT_Z, BEZEL_PILLAR_CENTER_X, LAVA_Y, WELL_DEPTH, WELL_HEIGHT, WELL_WIDTH,
};

/// Lightstyle 1 ("FLICKER, first variety"). `a` is dark, `m` is normal, `z`
/// is double brightness.
const LAMP_STYLE: &str = "mmnmmommommnonmmonqnmmo";

/// Lightstyle 6 ("FLICKER, second variety"), which reads as fire.
const FURNACE_STYLE: &str = "nmonqnmomnmomomno";

/// Lightstyles advance ten steps per second, deliberately unsmoothed.
const LIGHTSTYLE_RATE: f64 = 10.0;

/// Where the two cabinet lamps hang: centred on the pillar fronts, a little
/// below the lintel, shining down the well. The light sits just ahead of the
/// housing's lens.
pub const LAMP_HEIGHT: f32 = WELL_HEIGHT * 0.5 - 1.4;
pub const LAMP_OFFSET_X: f32 = BEZEL_PILLAR_CENTER_X;
pub const LAMP_Z: f32 = BEZEL_FRONT_Z + 0.38;

/// The furnace light is the lava surface itself: a glowing slab spanning the
/// pit at [`LAVA_Y`]. It is treated as an area source, so the nearest point
/// of the slab is used for falloff.
const FURNACE_Y: f32 = LAVA_Y;

/// Slightly cool, so unlit steel reads as steel and the warm sources stand
/// out against it instead of tinting everything the same brown.
const AMBIENT: Vec3 = Vec3::new(0.36, 0.38, 0.44);

const LAMP_COLOR: Vec3 = Vec3::new(1.0, 0.86, 0.62);
const LAMP_ALERT_COLOR: Vec3 = Vec3::new(1.0, 0.22, 0.14);
const LAMP_INTENSITY: f32 = 1.3;
const LAMP_RADIUS: f32 = 5.0;

const FURNACE_COLOR: Vec3 = Vec3::new(1.0, 0.42, 0.10);
const FURNACE_INTENSITY: f32 = 2.2;
const FURNACE_RADIUS: f32 = 3.2;

/// Sample a Quake lightstyle string at `time`, returning a brightness
/// multiplier where `m` is 1.0.
pub fn lightstyle(style: &str, time: f64) -> f32 {
    let bytes = style.as_bytes();
    let step = ((time * LIGHTSTYLE_RATE).floor() as usize) % bytes.len();
    (bytes[step] - b'a') as f32 / (b'm' - b'a') as f32
}

/// The lighting state for one frame.
#[derive(Copy, Clone, Debug)]
pub struct SceneLights {
    lamp_color: Vec3,
    lamp_intensity: f32,
    furnace_intensity: f32,
}

impl SceneLights {
    /// `danger` (0-1) turns the lamps red and sets them pulsing as the stack
    /// nears the top of the well; `flare` (0-1) momentarily overdrives them.
    pub fn new(time: f64, danger: f32, flare: f32) -> Self {
        let danger = danger.clamp(0.0, 1.0);
        let flare = flare.clamp(0.0, 1.0);
        let alert_pulse = 1.0 + danger * 0.35 * ((time * 6.0).sin() as f32);
        let lamp_flicker = lightstyle(LAMP_STYLE, time);
        let furnace_flicker = lightstyle(FURNACE_STYLE, time + 0.37)
            * (0.88 + 0.12 * ((time * 1.7).sin() as f32));

        Self {
            lamp_color: LAMP_COLOR.lerp(LAMP_ALERT_COLOR, danger),
            lamp_intensity: LAMP_INTENSITY * lamp_flicker * alert_pulse + flare * 0.9,
            furnace_intensity: FURNACE_INTENSITY * furnace_flicker,
        }
    }

    /// Lamps flickering at their normal colour, for menus and tests.
    pub fn idle(time: f64) -> Self {
        Self::new(time, 0.0, 0.0)
    }

    pub fn lamp_positions() -> [Vec3; 2] {
        [
            Vec3::new(-LAMP_OFFSET_X, LAMP_HEIGHT, LAMP_Z),
            Vec3::new(LAMP_OFFSET_X, LAMP_HEIGHT, LAMP_Z),
        ]
    }

    /// Colour of the lamps' emissive faces and glow right now.
    pub fn lamp_glow(&self) -> Color {
        let glow = self.lamp_color * (self.lamp_intensity / LAMP_INTENSITY).min(1.6);
        Color::new(glow.x.min(1.0), glow.y.min(1.0), glow.z.min(1.0), 1.0)
    }

    /// Relative brightness of the melt (around 1.0), for the lava surface and
    /// the effects that scale with it.
    pub fn furnace_level(&self) -> f32 {
        self.furnace_intensity / FURNACE_INTENSITY
    }

    /// Incident light at a world-space point, as a per-channel multiplier.
    /// Values above 1.0 are expected near the sources; callers clamp after
    /// multiplying with their base colour.
    pub fn at(&self, point: Vec3) -> Vec3 {
        let mut light = AMBIENT;

        for lamp in Self::lamp_positions() {
            let distance_squared = (point - lamp).length_squared();
            let falloff = 1.0 / (1.0 + distance_squared / (LAMP_RADIUS * LAMP_RADIUS));
            light += self.lamp_color * (self.lamp_intensity * falloff);
        }

        // Distance to the nearest point of the lava slab.
        let nearest_x = point.x.clamp(-WELL_WIDTH * 0.5, WELL_WIDTH * 0.5);
        let nearest_z = point.z.clamp(-WELL_DEPTH * 0.5, WELL_DEPTH * 0.5);
        let furnace_delta = point - Vec3::new(nearest_x, FURNACE_Y, nearest_z);
        let furnace_falloff =
            1.0 / (1.0 + furnace_delta.length_squared() / (FURNACE_RADIUS * FURNACE_RADIUS));
        light += FURNACE_COLOR * (self.furnace_intensity * furnace_falloff);

        light
    }

    /// A restrained version of [`Self::at`] for gameplay blocks: the stack
    /// picks up the room's warmth without any piece colour becoming hard to
    /// tell apart from its neighbours.
    pub fn block_tint(&self, point: Vec3) -> Color {
        let light = self.at(point);
        let tint = Vec3::ONE.lerp(light, 0.3).clamp(Vec3::splat(0.82), Vec3::splat(1.2));
        Color::new(tint.x, tint.y, tint.z, 1.0)
    }
}

/// Multiply a base colour by incident light, clamping each channel.
pub fn lit(base: Color, light: Vec3) -> Color {
    Color::new(
        (base.r * light.x).clamp(0.0, 1.0),
        (base.g * light.y).clamp(0.0, 1.0),
        (base.b * light.z).clamp(0.0, 1.0),
        base.a,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lightstyle_normal_is_unity_and_steps_ten_times_per_second() {
        assert!((lightstyle("m", 0.0) - 1.0).abs() < 0.001);
        assert!((lightstyle("am", 0.0) - 0.0).abs() < 0.001);
        assert!((lightstyle("am", 0.1) - 1.0).abs() < 0.001);
        assert!((lightstyle("am", 0.2) - 0.0).abs() < 0.001);
    }

    #[test]
    fn furnace_lights_the_floor_more_than_the_rim() {
        let lights = SceneLights::idle(0.0);
        let floor = lights.at(Vec3::new(0.0, -WELL_HEIGHT * 0.5 + 0.5, 0.0));
        let rim = lights.at(Vec3::new(0.0, WELL_HEIGHT * 0.5 - 0.5, 0.0));

        assert!(floor.x > rim.x);
        assert!(floor.x - floor.z > rim.x - rim.z, "the floor glow is warm");
    }

    #[test]
    fn lamps_light_their_own_pillar_most() {
        let lights = SceneLights::idle(0.0);
        let [left_lamp, _] = SceneLights::lamp_positions();
        let beside_lamp = lights.at(left_lamp + Vec3::new(0.0, -1.0, 0.0));
        let far_below = lights.at(left_lamp + Vec3::new(0.0, -14.0, 0.0));

        assert!(beside_lamp.x > far_below.x * 1.5);
    }

    #[test]
    fn block_tint_stays_within_readable_bounds() {
        for time in [0.0, 0.35, 1.2, 2.9] {
            let lights = SceneLights::new(time, 1.0, 1.0);
            for y in [-9.5, 0.0, 9.5] {
                let tint = lights.block_tint(Vec3::new(-4.5, y, 0.0));
                for channel in [tint.r, tint.g, tint.b] {
                    assert!((0.82..=1.2).contains(&channel));
                }
            }
        }
    }
}
