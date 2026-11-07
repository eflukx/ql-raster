//! The typefaces a text label can be set in: Barlow (ours), Roboto and Source Serif.
//!
//! How to use: `TextLabel::new("…").typeface(Typeface::Serif)`; `Typeface::ALL` for a choice
//! list, `code()` / `from_code()` for settings.
//!
//! Working principle: the fonts are compiled in. Roboto and Source Serif 4 are variable fonts:
//! one file per style, set to a weight (and Source Serif to an optical size) when first used.
//! Barlow has no italic files: its italic is slanted ([`Faces::italic`] is `None`). All OFL,
//! licences in `fonts/`.

use std::sync::LazyLock;

use ab_glyph::{FontRef, VariableFont};

use super::{FONT_BOLD, FONT_REGULAR};

/// A typeface for a text label.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Typeface {
    /// Barlow Semi Condensed: narrow, our house font.
    #[default]
    Barlow,
    /// Roboto: a plain, wider sans.
    Roboto,
    /// Source Serif 4: a serif.
    Serif,
}

impl Typeface {
    /// Every typeface, in menu order.
    pub const ALL: [Self; 3] = [Self::Barlow, Self::Roboto, Self::Serif];

    /// Its name, as shown.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Barlow => "Barlow",
            Self::Roboto => "Roboto",
            Self::Serif => "Serif",
        }
    }

    /// Its code in settings.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Barlow => "barlow",
            Self::Roboto => "roboto",
            Self::Serif => "serif",
        }
    }

    /// The typeface for `code`; Barlow for an unknown one.
    #[must_use]
    pub fn from_code(code: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|typeface| typeface.code() == code)
            .unwrap_or_default()
    }

    pub(crate) fn faces(self) -> &'static Faces {
        match self {
            Self::Barlow => &BARLOW,
            Self::Roboto => &ROBOTO,
            Self::Serif => &SERIF,
        }
    }
}

/// The fonts of one typeface.
pub(crate) struct Faces {
    pub regular: FontRef<'static>,
    pub bold: FontRef<'static>,
    /// Italic and bold italic; `None`: slant the upright ones.
    pub italic: Option<(FontRef<'static>, FontRef<'static>)>,
}

impl Faces {
    /// The font for a span, and whether it must be slanted.
    pub fn font(&self, bold: bool, italic: bool) -> (&FontRef<'static>, bool) {
        match (&self.italic, italic) {
            (Some((regular, bold_italic)), true) => {
                (if bold { bold_italic } else { regular }, false)
            }
            (None, true) => (if bold { &self.bold } else { &self.regular }, true),
            (_, false) => (if bold { &self.bold } else { &self.regular }, false),
        }
    }
}

const ROBOTO_FILE: &[u8] = include_bytes!("../../fonts/Roboto.ttf");
const ROBOTO_ITALIC_FILE: &[u8] = include_bytes!("../../fonts/Roboto-Italic.ttf");
const SERIF_FILE: &[u8] = include_bytes!("../../fonts/SourceSerif4.ttf");
const SERIF_ITALIC_FILE: &[u8] = include_bytes!("../../fonts/SourceSerif4-Italic.ttf");

/// `file` set to `axes` (tag, value).
fn instance(file: &'static [u8], axes: &[(&[u8; 4], f32)]) -> FontRef<'static> {
    let mut font = FontRef::try_from_slice(file).expect("embedded font");
    for (tag, value) in axes {
        font.set_variation(tag, *value);
    }
    font
}

static BARLOW: LazyLock<Faces> = LazyLock::new(|| Faces {
    regular: FONT_REGULAR.clone(),
    bold: FONT_BOLD.clone(),
    italic: None,
});

// A little heavier than text weights: thermal print thins strokes.
static ROBOTO: LazyLock<Faces> = LazyLock::new(|| {
    let weight = |file, wght| instance(file, &[(b"wght", wght)]);
    Faces {
        regular: weight(ROBOTO_FILE, 500.0),
        bold: weight(ROBOTO_FILE, 700.0),
        italic: Some((
            weight(ROBOTO_ITALIC_FILE, 500.0),
            weight(ROBOTO_ITALIC_FILE, 700.0),
        )),
    }
});

// Optical size 20: the display cut, for text of label size.
static SERIF: LazyLock<Faces> = LazyLock::new(|| {
    let weight = |file, wght| instance(file, &[(b"wght", wght), (b"opsz", 20.0)]);
    Faces {
        regular: weight(SERIF_FILE, 500.0),
        bold: weight(SERIF_FILE, 700.0),
        italic: Some((
            weight(SERIF_ITALIC_FILE, 500.0),
            weight(SERIF_ITALIC_FILE, 700.0),
        )),
    }
});

#[cfg(test)]
mod tests {
    use ab_glyph::Font;

    use super::*;

    #[test]
    fn codes_round_trip_and_fonts_load() {
        for typeface in Typeface::ALL {
            assert_eq!(Typeface::from_code(typeface.code()), typeface);
            let faces = typeface.faces();
            assert!(faces.regular.glyph_id('A').0 > 0);
            assert!(faces.font(true, true).0.glyph_id('a').0 > 0);
        }
        assert_eq!(Typeface::from_code("comic"), Typeface::Barlow);
    }

    #[test]
    fn the_weight_axis_makes_bold_wider() {
        let width = |font: &FontRef<'_>| crate::label::Bitmap::text_width(font, 100.0, "Wobble");
        let roboto = Typeface::Roboto.faces();
        assert!(width(&roboto.bold) > width(&roboto.regular));
    }
}
