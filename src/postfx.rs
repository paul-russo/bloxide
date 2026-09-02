//! Presentation pass that blits the low-resolution framebuffer to the window
//! through a software-renderer style filter.
//!
//! Everything the game draws is full 24-bit colour. This pass quantises it to a
//! coarse per-channel palette with an ordered dither, drags the darkest tones
//! toward warm brown the way a 256-colour colormap would, and adds a light
//! vignette. The dither is evaluated in *framebuffer* pixels, not window pixels,
//! so it stays locked to the integer-upscaled grid.

use macroquad::prelude::*;

/// Colour levels per channel after quantisation. Sixteen is coarse enough that
/// smooth gradients visibly band into dithered steps without crushing the HUD.
const PALETTE_LEVELS: f32 = 16.0;

/// Strength of the ordered dither, as a fraction of one quantisation step.
/// Kept below one so flat fills near a palette level snap to it cleanly and
/// only gradients break into dither.
const DITHER_STRENGTH: f32 = 0.5;

/// How much the frame darkens toward its corners.
const VIGNETTE_STRENGTH: f32 = 0.42;

// The present pass samples the framebuffer texture at `highp`. At the original
// `lowp`, texture coordinates toward the top of the frame (v approaching 1.0)
// lost enough precision that on some GL stacks (Mesa/llvmpipe under Xvfb, used
// for headless captures) the upper rows sampled outside the texture and came
// back black, leaving the top third of the window blank. Both the `uv` varying
// and the fragment shader's default float precision are `highp` so the sample
// stays exact across the whole frame.
const VERTEX_SHADER: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying highp vec2 uv;
varying lowp vec4 color;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

const FRAGMENT_SHADER: &str = r#"#version 100
precision highp float;

varying highp vec2 uv;
varying lowp vec4 color;

uniform sampler2D Texture;
uniform vec2 TargetSize;
uniform float PaletteLevels;
uniform float DitherStrength;
uniform float VignetteStrength;

// 2x2 and 4x4 ordered (Bayer) thresholds in [0, 1), built arithmetically so
// no array indexing is needed on GLES2-class drivers.
float bayer2(vec2 p) {
    p = floor(p);
    return fract(p.x * 0.5 + p.y * p.y * 0.75);
}

float bayer4(vec2 p) {
    return bayer2(0.5 * p) * 0.25 + bayer2(p);
}

void main() {
    vec3 rgb = texture2D(Texture, uv).rgb * color.rgb;
    vec2 pixel = floor(uv * TargetSize);

    // Colormap: darks drift brown rather than toward neutral black, and the
    // very bottom of the range lifts slightly so shadows stay legible.
    float luma = dot(rgb, vec3(0.299, 0.587, 0.114));
    vec3 warm = rgb * vec3(1.05, 0.97, 0.86) + vec3(0.014, 0.009, 0.0);
    rgb = mix(warm, rgb, smoothstep(0.0, 0.45, luma));

    // Vignette is applied before quantisation so its falloff dithers too.
    vec2 centered = (uv - 0.5) * vec2(TargetSize.x / TargetSize.y, 1.0);
    float edge = dot(centered, centered) * 1.35;
    rgb *= 1.0 - VignetteStrength * edge * edge;

    float steps = PaletteLevels - 1.0;
    float threshold = (bayer4(pixel) - 0.5) * DitherStrength;
    rgb = floor(rgb * steps + 0.5 + threshold) / steps;

    gl_FragColor = vec4(clamp(rgb, 0.0, 1.0), 1.0);
}"#;

pub struct PostProcess {
    material: Material,
}

impl PostProcess {
    pub fn new(target_width: u32, target_height: u32) -> Self {
        let material = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX_SHADER,
                fragment: FRAGMENT_SHADER,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("TargetSize", UniformType::Float2),
                    UniformDesc::new("PaletteLevels", UniformType::Float1),
                    UniformDesc::new("DitherStrength", UniformType::Float1),
                    UniformDesc::new("VignetteStrength", UniformType::Float1),
                ],
                ..Default::default()
            },
        )
        .expect("present-pass material should compile");

        material.set_uniform(
            "TargetSize",
            vec2(target_width as f32, target_height as f32),
        );
        material.set_uniform("PaletteLevels", PALETTE_LEVELS);
        material.set_uniform("DitherStrength", DITHER_STRENGTH);
        material.set_uniform("VignetteStrength", VIGNETTE_STRENGTH);

        Self { material }
    }

    /// Blit `texture` to `origin`/`size` on the current camera through the
    /// filter, then hand the pipeline back to the default material.
    pub fn blit(&self, texture: &Texture2D, origin: Vec2, size: Vec2) {
        gl_use_material(&self.material);
        draw_texture_ex(
            texture,
            origin.x,
            origin.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(size),
                flip_y: true,
                ..Default::default()
            },
        );
        gl_use_default_material();
    }
}
