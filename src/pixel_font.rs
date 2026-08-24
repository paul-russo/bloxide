//! Bitmap fonts for the low-resolution HUD.
//!
//! Macroquad's antialiased default font loses stems when it is rasterised at
//! 600x450 and then enlarged. These glyphs are composed only of whole
//! framebuffer pixels, so they stay crisp at every integer presentation scale.
//! Two faces are provided: a compact 5x7 alphabet for labels, and a bold 7x11
//! numeral set with two-pixel strokes for the score and status readouts, in
//! the spirit of a mid-90s shooter's status-bar digits.
//!
//! This module holds the glyph bitmaps and their metrics. The bitmaps are
//! packed into the material atlas at startup and each glyph is drawn as one
//! textured quad, so the layout functions here only say where every glyph
//! cell lands.

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

/// Every character the 5x7 face has a glyph for. Lookups are
/// case-insensitive, so lowercase letters share the uppercase glyphs.
pub const SMALL_GLYPH_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789/>!:,.-";

/// Every character the bold numeral face has a glyph for.
pub const DIGIT_GLYPH_CHARS: &str = "0123456789,";

/// The 5x7 alphabet, one bit per pixel, most significant bit leftmost.
/// Characters outside [`SMALL_GLYPH_CHARS`] are blank.
pub fn small_glyph(character: char) -> [u8; 7] {
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

/// Lay out `text` in the 5x7 face with its left edge at `x` and its baseline
/// at `baseline_y`, both in framebuffer pixels: each character with the
/// rectangle its glyph cell covers.
pub fn small_text_glyphs(
    text: &str,
    x: f32,
    baseline_y: f32,
    pixel: f32,
) -> impl Iterator<Item = (char, Rect)> + '_ {
    let origin_x = x.round();
    let origin_y = (baseline_y - SMALL_GLYPH_HEIGHT * pixel).round();

    text.chars().enumerate().map(move |(index, character)| {
        let glyph_x = origin_x + index as f32 * pixel * SMALL_GLYPH_PITCH;
        let cell = Rect::new(
            glyph_x,
            origin_y,
            SMALL_GLYPH_WIDTH * pixel,
            SMALL_GLYPH_HEIGHT * pixel,
        );

        (character, cell)
    })
}

/// Bold numerals: each glyph is eleven rows of `#` and `.`. A comma is a
/// narrow three-column glyph so thousands separators do not open up gaps.
pub fn digit_glyph(character: char) -> Option<&'static [&'static str; DIGIT_HEIGHT]> {
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

/// Lay out `text` in bold numerals with its left edge at `x` and its top at
/// `top_y`, both in framebuffer pixels: each character with the rectangle its
/// glyph cell covers. Characters without a glyph still advance a full cell.
pub fn digit_text_glyphs(
    text: &str,
    x: f32,
    top_y: f32,
    pixel: f32,
) -> impl Iterator<Item = (char, Rect)> + '_ {
    let top_y = top_y.round();
    let mut cursor_x = x.round();

    text.chars().map(move |character| {
        let width = digit_glyph(character).map_or(DIGIT_WIDTH, |glyph| glyph[0].len());
        let cell = Rect::new(
            cursor_x,
            top_y,
            width as f32 * pixel,
            DIGIT_HEIGHT as f32 * pixel,
        );
        cursor_x += digit_advance(character) as f32 * pixel;

        (character, cell)
    })
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

    #[test]
    fn every_listed_glyph_character_has_a_bitmap() {
        for character in SMALL_GLYPH_CHARS.chars() {
            assert_ne!(small_glyph(character), [0; 7], "{character}");
        }
        for character in DIGIT_GLYPH_CHARS.chars() {
            assert!(digit_glyph(character).is_some(), "{character}");
        }
    }

    #[test]
    fn small_text_cells_sit_on_whole_pixels_at_the_glyph_pitch() {
        let cells: Vec<(char, Rect)> = small_text_glyphs("AB", 10.4, 30.0, 2.0).collect();

        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].0, 'A');
        assert_eq!(cells[0].1, Rect::new(10.0, 16.0, 10.0, 14.0));
        assert_eq!(cells[1].1.x, 10.0 + SMALL_GLYPH_PITCH * 2.0);
        assert_eq!(small_text_width("AB", 2.0), cells[1].1.x + cells[1].1.w - cells[0].1.x);
    }

    #[test]
    fn digit_text_cells_advance_by_each_glyphs_own_width() {
        let cells: Vec<(char, Rect)> = digit_text_glyphs("1,2", 0.0, 0.0, 1.0).collect();

        assert_eq!(cells[0].1, Rect::new(0.0, 0.0, 7.0, 11.0));
        assert_eq!(cells[1].1, Rect::new(8.0, 0.0, 3.0, 11.0));
        assert_eq!(cells[2].1.x, 12.0);
        assert_eq!(digit_text_width("1,2", 1.0), cells[2].1.x + cells[2].1.w);
    }
}
