//! Bitmap fonts for the low-resolution HUD.
//!
//! Macroquad's antialiased default font loses stems when it is rasterised at
//! 600x450 and then enlarged. These glyphs are composed only of whole
//! framebuffer pixels, so they stay crisp at every integer presentation scale.
//! Two faces are provided: a compact 5x7 alphabet for labels, and a bold 7x11
//! numeral set with two-pixel strokes for the score and status readouts, in
//! the spirit of a mid-90s shooter's status-bar digits.

use macroquad::prelude::*;

/// Width of a 5x7 glyph cell, in glyph pixels.
pub const SMALL_GLYPH_WIDTH: f32 = 5.0;

/// Height of a 5x7 glyph cell, in glyph pixels.
pub const SMALL_GLYPH_HEIGHT: f32 = 7.0;

/// Horizontal advance between 5x7 glyphs, in glyph pixels.
pub const SMALL_GLYPH_PITCH: f32 = 6.0;

/// Height of a bold numeral, in glyph pixels.
pub const DIGIT_HEIGHT: usize = 11;

/// Width of a bold numeral cell, in glyph pixels.
pub const DIGIT_WIDTH: usize = 7;

/// Gap between bold glyphs, in glyph pixels.
const DIGIT_GAP: usize = 1;

/// The 5x7 alphabet, one bit per pixel, most significant bit leftmost.
fn small_glyph(character: char) -> [u8; 7] {
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

/// Width in framebuffer pixels of `text` set in the 5x7 face at `pixel`
/// pixels per glyph pixel.
pub fn small_text_width(text: &str, pixel: f32) -> f32 {
    let count = text.chars().count();
    if count == 0 {
        return 0.0;
    }

    count as f32 * SMALL_GLYPH_PITCH * pixel - pixel
}

/// Draw `text` in the 5x7 face with its left edge at `x` and its baseline at
/// `baseline_y`, both in framebuffer pixels.
pub fn draw_small_text(text: &str, x: f32, baseline_y: f32, pixel: f32, color: Color) {
    let origin_x = x.round();
    let origin_y = (baseline_y - SMALL_GLYPH_HEIGHT * pixel).round();

    for (character_index, character) in text.chars().enumerate() {
        let glyph = small_glyph(character);
        let glyph_x = origin_x + character_index as f32 * pixel * SMALL_GLYPH_PITCH;

        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..SMALL_GLYPH_WIDTH as usize {
                if bits & (1 << (SMALL_GLYPH_WIDTH as usize - 1 - col)) != 0 {
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

/// Bold numerals: each glyph is eleven rows of `#` and `.`. A comma is a
/// narrow three-column glyph so thousands separators do not open up gaps.
fn digit_glyph(character: char) -> Option<&'static [&'static str; DIGIT_HEIGHT]> {
    let glyph: &[&str; DIGIT_HEIGHT] = match character {
        '0' => &[
            ".#####.", "#######", "##...##", "##...##", "##...##", "##...##", "##...##",
            "##...##", "##...##", "#######", ".#####.",
        ],
        '1' => &[
            "...##..", "..###..", ".####..", "...##..", "...##..", "...##..", "...##..",
            "...##..", "...##..", ".######", ".######",
        ],
        '2' => &[
            ".#####.", "#######", "##...##", ".....##", "....###", "..####.", ".###...",
            "###....", "##.....", "#######", "#######",
        ],
        '3' => &[
            ".#####.", "#######", "##...##", ".....##", "..####.", "..####.", ".....##",
            ".....##", "##...##", "#######", ".#####.",
        ],
        '4' => &[
            "....##.", "...###.", "..####.", ".##.##.", "##..##.", "##..##.", "#######",
            "#######", "....##.", "....##.", "....##.",
        ],
        '5' => &[
            "#######", "#######", "##.....", "##.....", "######.", "#######", ".....##",
            ".....##", "##...##", "#######", ".#####.",
        ],
        '6' => &[
            ".#####.", "#######", "##...##", "##.....", "######.", "#######", "##...##",
            "##...##", "##...##", "#######", ".#####.",
        ],
        '7' => &[
            "#######", "#######", ".....##", "....##.", "....##.", "...##..", "...##..",
            "..##...", "..##...", "..##...", "..##...",
        ],
        '8' => &[
            ".#####.", "#######", "##...##", "##...##", ".#####.", ".#####.", "##...##",
            "##...##", "##...##", "#######", ".#####.",
        ],
        '9' => &[
            ".#####.", "#######", "##...##", "##...##", "##...##", "#######", ".######",
            ".....##", "##...##", "#######", ".#####.",
        ],
        ',' => &[
            "...", "...", "...", "...", "...", "...", "...", "...", ".##", ".##", "##.",
        ],
        _ => return None,
    };

    Some(glyph)
}

/// Advance of one bold glyph (including its trailing gap), in glyph pixels.
fn digit_advance(character: char) -> usize {
    match digit_glyph(character) {
        Some(glyph) => glyph[0].len() + DIGIT_GAP,
        None => DIGIT_WIDTH + DIGIT_GAP,
    }
}

/// Width in framebuffer pixels of `text` set in bold numerals at `pixel`
/// pixels per glyph pixel.
pub fn digit_text_width(text: &str, pixel: f32) -> f32 {
    let advance: usize = text.chars().map(digit_advance).sum();
    if advance == 0 {
        return 0.0;
    }

    (advance - DIGIT_GAP) as f32 * pixel
}

/// Draw `text` in bold numerals with its left edge at `x` and its top at
/// `top_y`, both in framebuffer pixels. Characters without a glyph advance a
/// full cell and draw nothing.
pub fn draw_digit_text(text: &str, x: f32, top_y: f32, pixel: f32, color: Color) {
    let mut cursor_x = x.round();
    let top_y = top_y.round();

    for character in text.chars() {
        if let Some(glyph) = digit_glyph(character) {
            for (row, bits) in glyph.iter().enumerate() {
                for (col, cell) in bits.chars().enumerate() {
                    if cell == '#' {
                        draw_rectangle(
                            cursor_x + col as f32 * pixel,
                            top_y + row as f32 * pixel,
                            pixel,
                            pixel,
                            color,
                        );
                    }
                }
            }
        }

        cursor_x += digit_advance(character) as f32 * pixel;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_numeral_is_a_full_height_cell_of_consistent_width() {
        for character in "0123456789".chars() {
            let glyph = digit_glyph(character).expect("digit glyph");
            for row in glyph.iter() {
                assert_eq!(row.len(), DIGIT_WIDTH, "{character}: {row}");
                assert!(row.chars().all(|c| c == '#' || c == '.'));
            }

            let lit_rows = glyph.iter().filter(|row| row.contains('#')).count();
            assert_eq!(lit_rows, DIGIT_HEIGHT, "{character} should use all rows");
        }
    }

    #[test]
    fn thousands_separator_is_narrower_than_a_digit() {
        assert!(digit_text_width(",", 1.0) < digit_text_width("0", 1.0));
        assert_eq!(digit_text_width("12", 1.0), (DIGIT_WIDTH * 2 + DIGIT_GAP) as f32);
    }

    #[test]
    fn small_text_width_excludes_the_trailing_gap() {
        assert_eq!(small_text_width("", 2.0), 0.0);
        assert_eq!(small_text_width("A", 2.0), SMALL_GLYPH_WIDTH * 2.0);
        assert_eq!(
            small_text_width("AB", 1.0),
            SMALL_GLYPH_PITCH * 2.0 - 1.0
        );
    }
}
