//! Labels as 1-bit bitmaps at printer resolution: the drawing surface and text labels.
//!
//! - [`Bitmap`]: black and white dots, text drawing and measuring, conversion to an image for a
//!   preview.
//! - [`TextLabel`]: lines of text (with `*bold*` and `_italic_` runs) set as large as they fit.
//! - [`Typeface`]: Barlow Semi Condensed, Roboto, Source Serif 4, compiled in (all OFL, licences
//!   in `fonts/`).
//!
//! Working principle: a label is drawn directly at 300 dpi and thresholded to black and white, so
//! a preview shows exactly what the printer prints. [`crate::raster`] turns the bitmap into
//! raster lines for the medium.

use std::sync::LazyLock;

use ab_glyph::FontRef;

mod bitmap;
mod text;
mod typeface;

pub use bitmap::Bitmap;
pub use text::{Length, Run, Span, TextLabel, parse_line};
pub use typeface::Typeface;

/// Printer resolution in dots per inch.
pub const DPI: f32 = 300.0;

/// Points (1/72 inch) to dots.
pub const DOTS_PER_POINT: f32 = DPI / 72.0;

/// Converts millimetres to printer dots.
#[must_use]
pub fn mm_to_dots(mm: f32) -> u32 {
    // Label sizes are a few centimetres, far below u32::MAX dots.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let dots = (mm / 25.4 * DPI).round() as u32;
    dots
}

/// Barlow Semi Condensed Medium: the regular weight of the Barlow typeface.
pub static FONT_REGULAR: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(include_bytes!("../../fonts/BarlowSemiCondensed-Medium.ttf"))
        .expect("embedded font is valid")
});

/// Barlow Semi Condensed SemiBold: the bold weight of the Barlow typeface.
pub static FONT_BOLD: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(include_bytes!(
        "../../fonts/BarlowSemiCondensed-SemiBold.ttf"
    ))
    .expect("embedded font is valid")
});
