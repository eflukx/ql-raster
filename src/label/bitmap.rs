//! A 1-bit bitmap with the drawing operations labels need.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{GrayImage, Luma};

/// Pixels darker than this (0–255) print black.
const THRESHOLD: u8 = 128;

/// A black and white bitmap, row by row from the top. `true` is black (printed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    width: u32,
    height: u32,
    pixels: Vec<bool>,
}

impl Bitmap {
    /// Creates a white bitmap.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![false; (width * height) as usize],
        }
    }

    /// Width in dots.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in dots.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Whether the dot at (`x`, `y`) is black. Outside the bitmap is white.
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height && self.pixels[(y * self.width + x) as usize]
    }

    /// Sets one dot. Dots outside the bitmap are ignored.
    pub fn set(&mut self, x: i64, y: i64, black: bool) {
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            return;
        };

        if x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize] = black;
        }
    }

    /// Fills a rectangle.
    pub fn fill_rect(&mut self, x: i64, y: i64, width: u32, height: u32) {
        for dy in 0..i64::from(height) {
            for dx in 0..i64::from(width) {
                self.set(x + dx, y + dy, true);
            }
        }
    }

    /// Draws a grayscale image with its top left corner at (`x`, `y`), thresholded.
    pub fn draw_image(&mut self, image: &GrayImage, x: i64, y: i64) {
        for (px, py, Luma([value])) in image.enumerate_pixels() {
            if *value < THRESHOLD {
                self.set(x + i64::from(px), y + i64::from(py), true);
            }
        }
    }

    /// Draws `text` with its top left at (`x`, `y`), `size` dots tall (em size). Returns the
    /// advance width.
    pub fn draw_text(&mut self, font: &FontRef<'_>, size: f32, x: f32, y: f32, text: &str) -> f32 {
        let scaled = font.as_scaled(PxScale::from(size));
        let baseline = y + scaled.ascent();
        let mut caret = x;
        let mut previous = None;

        for c in text.chars() {
            let id = scaled.glyph_id(c);

            if let Some(previous) = previous {
                caret += scaled.kern(previous, id);
            }

            let glyph = id.with_scale_and_position(size, ab_glyph::point(caret, baseline));
            caret += scaled.h_advance(id);
            previous = Some(id);

            let Some(outline) = font.outline_glyph(glyph) else {
                continue;
            };

            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                if coverage >= 0.5 {
                    // Glyph bounds are within a few hundred dots of the label.
                    #[allow(clippy::cast_possible_truncation)]
                    let (px, py) = (
                        (bounds.min.x as i64) + i64::from(gx),
                        (bounds.min.y as i64) + i64::from(gy),
                    );
                    self.set(px, py, true);
                }
            });
        }

        caret - x
    }

    /// The horizontal extent of the ink of `text` drawn at x = 0: `(left, right)` in dots. Unlike
    /// the advance width it leaves out side bearings, so text centres on what is visible.
    #[must_use]
    pub fn text_ink_span(font: &FontRef<'_>, size: f32, text: &str) -> Option<(f32, f32)> {
        let scaled = font.as_scaled(PxScale::from(size));
        let mut caret = 0.0;
        let mut previous = None;
        let mut span: Option<(f32, f32)> = None;

        for c in text.chars() {
            let id = scaled.glyph_id(c);
            if let Some(previous) = previous {
                caret += scaled.kern(previous, id);
            }

            let glyph = id.with_scale_and_position(size, ab_glyph::point(caret, scaled.ascent()));
            if let Some(outline) = font.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                span = Some(match span {
                    Some((left, right)) => (left.min(bounds.min.x), right.max(bounds.max.x)),
                    None => (bounds.min.x, bounds.max.x),
                });
            }

            caret += scaled.h_advance(id);
            previous = Some(id);
        }

        span
    }

    /// Measures the advance width of `text` at `size`.
    #[must_use]
    pub fn text_width(font: &FontRef<'_>, size: f32, text: &str) -> f32 {
        let scaled = font.as_scaled(PxScale::from(size));
        let mut width = 0.0;
        let mut previous = None;

        for c in text.chars() {
            let id = scaled.glyph_id(c);
            if let Some(previous) = previous {
                width += scaled.kern(previous, id);
            }
            width += scaled.h_advance(id);
            previous = Some(id);
        }

        width
    }

    /// Converts to an 8-bit grayscale image (black on white) for previews and PNG export.
    #[must_use]
    pub fn to_gray_image(&self) -> GrayImage {
        GrayImage::from_fn(self.width, self.height, |x, y| {
            Luma([if self.get(x, y) { 0 } else { 255 }])
        })
    }

    /// Number of black dots (for tests).
    #[must_use]
    pub fn black_dots(&self) -> usize {
        self.pixels.iter().filter(|&&black| black).count()
    }
}
