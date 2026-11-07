//! A text label: a few lines of text, as large as the label allows.
//!
//! How to use: `TextLabel::new("Rack 4\n**do not move**").render(696, Length::AtMost(1392),
//! Run::AlongTape)` gives the raster bitmap for 62 mm continuous tape (696 print dots across),
//! the text running along the tape, the label at most twice the tape's width long. Markdown:
//! `**bold**`, `*italic*`, `***both***`, `\*` for a star.
//!
//! Working principle: the lines are laid out in a box of the tape's width and the label's
//! length ([`Length`]: fixed for a die-cut label, else as long as needed, up to a limit or not).
//! [`Run::AlongTape`] reads along the tape: the tape's width is the text's height. The font size
//! is the largest that fits the box; a free side gets as long as the text needs. Each line is
//! centred. Along the tape, the image is turned a quarter for the raster, its start printed
//! first. [`Typeface`]: Barlow by default; its italic is the upright face slanted 12° (Barlow
//! comes without italic files), Roboto and Source Serif have real italics. [`Self::render_fine`]:
//! for a QL's high resolution, 600 dpi along the tape: drawn at 600 dpi, every two dots across
//! the tape joined into one.

use ab_glyph::{Font, PxScale, ScaleFont};

use super::Bitmap;
use super::typeface::{Faces, Typeface};

/// Margin around the text, as part of the tape's print width. Not at the ends of a label on
/// continuous tape: there the printer's feed (35 dots, 3 mm) is the margin, as on ChopShop's
/// labels.
const MARGIN: f32 = 0.06;

/// The margins of the text box: above and below, left and right.
#[derive(Debug, Clone, Copy)]
struct Margins {
    height: f32,
    width: f32,
}

/// Space between lines, as part of the font size.
const LINE_GAP: f32 = 0.08;

/// Slant of italic text: dots sideways per dot up (tan 12°).
const SLANT: f32 = 0.21;

/// Shortest raster on continuous tape: with 2 × 35 dots feed the printer's 150-dot minimum.
const MIN_CONTINUOUS_LINES: u32 = 80;

/// A run of text in one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// The text.
    pub text: String,
    /// `**bold**`.
    pub bold: bool,
    /// `*italic*`.
    pub italic: bool,
}

/// The spans of one line of markdown.
#[must_use]
pub fn parse_line(line: &str) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let (mut bold, mut italic) = (false, false);
    let mut chars = line.chars().peekable();
    let push = |c: char, bold: bool, italic: bool, spans: &mut Vec<Span>| match spans.last_mut() {
        Some(span) if span.bold == bold && span.italic == italic => span.text.push(c),
        _ => spans.push(Span {
            text: c.to_string(),
            bold,
            italic,
        }),
    };

    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    push(next, bold, italic, &mut spans);
                }
            }
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                bold = !bold;
            }
            '*' => italic = !italic,
            other => push(other, bold, italic, &mut spans),
        }
    }
    spans
}

/// How long the label is, along the tape, in raster lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Length {
    /// A die-cut label: exactly this long.
    Fixed(u32),
    /// Continuous tape: as long as the text needs, at most this.
    AtMost(u32),
    /// Continuous tape: as long as the text needs.
    Free,
}

impl Length {
    fn limit(self) -> Option<u32> {
        match self {
            Self::Fixed(lines) | Self::AtMost(lines) => Some(lines),
            Self::Free => None,
        }
    }
}

/// Which way the text reads on the tape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// Along the tape: the lines lie across its width.
    AlongTape,
    /// Across the tape: a line spans the tape's width.
    AcrossTape,
}

/// A label of plain lines with a little markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLabel {
    lines: Vec<Vec<Span>>,
    fill_lines: bool,
    typeface: Typeface,
    /// Dots per 300-dpi dot: 2 while drawing for [`Self::render_fine`].
    scale: u32,
}

impl TextLabel {
    /// Lines from `text`; blank lines at the start and the end go.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let lines: Vec<&str> = text.lines().collect();
        let first = lines.iter().position(|line| !line.trim().is_empty());
        let last = lines.iter().rposition(|line| !line.trim().is_empty());
        let lines = match (first, last) {
            (Some(first), Some(last)) => {
                lines[first..=last].iter().map(|l| parse_line(l)).collect()
            }
            _ => Vec::new(),
        };
        Self {
            lines,
            fill_lines: false,
            typeface: Typeface::default(),
            scale: 1,
        }
    }

    /// Set in `typeface` (one for the whole label); Barlow unless set.
    #[must_use]
    pub fn typeface(mut self, typeface: Typeface) -> Self {
        self.typeface = typeface;
        self
    }

    fn faces(&self) -> &'static Faces {
        self.typeface.faces()
    }

    /// Every line as large as fills the width, each its own size; all shrink together when
    /// they get too tall. Off: one size for all lines.
    #[must_use]
    pub fn fill_lines(mut self, on: bool) -> Self {
        self.fill_lines = on;
        self
    }

    /// Whether there is nothing to print.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The raster bitmap: `across` dots wide (the tape's print width), as long as `length`
    /// says.
    #[must_use]
    pub fn render(&self, across: u32, length: Length, run: Run) -> Bitmap {
        match run {
            Run::AlongTape => rotate_into_raster(&self.landscape(across, length, run)),
            Run::AcrossTape => self.landscape(across, length, run),
        }
    }

    /// As [`Self::render`], for a printer in high resolution: 600 dpi along the tape, so twice
    /// the raster lines (`length` stays in 300-dpi lines), `across` dots wide as before.
    #[must_use]
    pub fn render_fine(&self, across: u32, length: Length, run: Run) -> Bitmap {
        let doubled = match length {
            Length::Fixed(lines) => Length::Fixed(lines * 2),
            Length::AtMost(lines) => Length::AtMost(lines * 2),
            Length::Free => Length::Free,
        };
        let fine = Self {
            scale: 2,
            ..self.clone()
        }
        .render(across * 2, doubled, run);
        // The head has 300 dpi: a dot is black when either of its two is.
        let mut raster = Bitmap::new(across, fine.height());
        for y in 0..fine.height() {
            for x in 0..across {
                if fine.get(2 * x, y) || fine.get(2 * x + 1, y) {
                    raster.set(i64::from(x), i64::from(y), true);
                }
            }
        }
        raster
    }

    /// The label as read: for the preview. Along the tape the tape's width is its height;
    /// across, its width.
    #[must_use]
    pub fn landscape(&self, across: u32, length: Length, run: Run) -> Bitmap {
        let margin = (across as f32 * MARGIN).max(6.0);
        // A die-cut label has edges of its own; tape has the feed.
        let ends = if matches!(length, Length::Fixed(_)) {
            margin
        } else {
            0.0
        };
        let limit = length.limit().map(|lines| lines as f32);
        let fixed = match length {
            Length::Fixed(lines) => Some(lines),
            _ => None,
        };
        match run {
            Run::AlongTape => {
                let margins = Margins {
                    height: margin,
                    width: ends,
                };
                let sizes = self.sizes(margins, Some(across as f32), limit);
                self.draw(margins, Some(across), fixed, length.limit(), &sizes)
            }
            Run::AcrossTape => {
                let margins = Margins {
                    height: ends,
                    width: margin,
                };
                let sizes = self.sizes(margins, limit, Some(across as f32));
                self.draw(margins, fixed, Some(across), length.limit(), &sizes)
            }
        }
    }

    /// The font size of each line: one size from [`Self::fit`], or with `fill_lines` each line
    /// filling the width (the box's, else the widest line's at one size), then all shrunk
    /// together until the lines fit `height`.
    fn sizes(&self, margins: Margins, height: Option<f32>, width: Option<f32>) -> Vec<f32> {
        let uniform = self.fit(margins, height, width);
        if !self.fill_lines || self.lines.len() < 2 {
            return vec![uniform; self.lines.len()];
        }

        let room = width.map_or_else(|| self.widest(uniform), |w| w - 2.0 * margins.width);
        let mut sizes: Vec<f32> = self
            .lines
            .iter()
            .map(|line| {
                // A blank line keeps the one size: it only makes room.
                let per_size = line_width(self.faces(), line, 100.0) / 100.0;
                if per_size > 0.0 {
                    room / per_size
                } else {
                    uniform
                }
            })
            .collect();
        if let Some(height) = height {
            let room = height - 2.0 * margins.height;
            let block = block(self.faces(), &sizes);
            if block > room {
                let shrink = room / block;
                sizes.iter_mut().for_each(|size| *size *= shrink);
            }
        }
        sizes.into_iter().map(|size| size.max(1.0)).collect()
    }

    /// The largest font size whose lines fit `height` and whose widest line fits `width`.
    fn fit(&self, margins: Margins, height: Option<f32>, width: Option<f32>) -> f32 {
        let count = self.lines.len().max(1) as f32;
        let by_height = height.map(|h| {
            (h - 2.0 * margins.height)
                / (count * line_height(self.faces(), 1.0) + (count - 1.0) * LINE_GAP)
        });
        // Widths grow with the size: measure at 100 and scale.
        let per_size = self.widest(100.0) / 100.0;
        let by_width = width
            .filter(|_| per_size > 0.0)
            .map(|w| (w - 2.0 * margins.width) / per_size);
        let size = match (by_height, by_width) {
            (Some(h), Some(w)) => h.min(w),
            (Some(one), None) | (None, Some(one)) => one,
            (None, None) => 1.0,
        };
        size.max(1.0)
    }

    fn widest(&self, size: f32) -> f32 {
        self.lines
            .iter()
            .map(|line| line_width(self.faces(), line, size))
            .fold(0.0_f32, f32::max)
    }

    /// The lines at their `sizes`, centred, in a `height` × `width` image; a side not given is
    /// as long as the text needs (at least the printer's shortest label, at most `cap`).
    fn draw(
        &self,
        margins: Margins,
        height: Option<u32>,
        width: Option<u32>,
        cap: Option<u32>,
        sizes: &[f32],
    ) -> Bitmap {
        let shortest = MIN_CONTINUOUS_LINES * self.scale;
        let needed = |extent: f32, margin: f32| {
            let lines = ((extent + 2.0 * margin).ceil() as u32).max(shortest);
            cap.map_or(lines, |cap| lines.min(cap.max(shortest)))
        };
        let widest = self
            .lines
            .iter()
            .zip(sizes)
            .map(|(line, &size)| line_width(self.faces(), line, size))
            .fold(0.0_f32, f32::max);
        // A free side without margin (tape) is trimmed to the ink afterwards.
        let trim_width = width.is_none() && margins.width == 0.0;
        let trim_height = height.is_none() && margins.height == 0.0;
        let width = width.unwrap_or_else(|| needed(widest, margins.width));
        let height = height.unwrap_or_else(|| needed(block(self.faces(), sizes), margins.height));
        let mut bitmap = Bitmap::new(width.max(1), height.max(1));
        if self.lines.is_empty() {
            return bitmap;
        }

        let mut top = (height as f32 - block(self.faces(), sizes)) / 2.0;
        for (spans, &size) in self.lines.iter().zip(sizes) {
            let line = line_height(self.faces(), size);
            let mut x = (width as f32 - line_width(self.faces(), spans, size)) / 2.0;
            for span in spans {
                x += draw_span(&mut bitmap, self.faces(), span, size, (x, top), line);
            }
            top += line + size * LINE_GAP;
        }
        if trim_width {
            bitmap = trim(&bitmap, true, shortest);
        }
        if trim_height {
            bitmap = trim(&bitmap, false, shortest);
        }
        bitmap
    }
}

/// `bitmap` cut to its ink along x (`columns`) or y: the glyphs' side bearings and line
/// spacing go too. At least `shortest` long (the printer's shortest label), the ink centred.
fn trim(bitmap: &Bitmap, columns: bool, shortest: u32) -> Bitmap {
    let (length, other) = if columns {
        (bitmap.width(), bitmap.height())
    } else {
        (bitmap.height(), bitmap.width())
    };
    let at = |i: u32, j: u32| {
        if columns {
            bitmap.get(i, j)
        } else {
            bitmap.get(j, i)
        }
    };
    let inked = |i: u32| (0..other).any(|j| at(i, j));
    let (Some(first), Some(last)) = (
        (0..length).find(|&i| inked(i)),
        (0..length).rev().find(|&i| inked(i)),
    ) else {
        return bitmap.clone();
    };
    let ink = last - first + 1;
    let new = ink.max(shortest);
    let start = i64::from(first) - i64::from((new - ink) / 2);
    let (width, height) = if columns { (new, other) } else { (other, new) };
    let mut out = Bitmap::new(width, height);
    for i in 0..new {
        let Ok(source) = u32::try_from(start + i64::from(i)) else {
            continue;
        };
        if source >= length {
            continue;
        }
        for j in 0..other {
            if at(source, j) {
                let (x, y) = if columns { (i, j) } else { (j, i) };
                out.set(i64::from(x), i64::from(y), true);
            }
        }
    }
    out
}

/// The height of lines at `sizes`, with the gap after each but the last.
fn block(faces: &Faces, sizes: &[f32]) -> f32 {
    let lines: f32 = sizes.iter().map(|&size| line_height(faces, size)).sum();
    let gaps: f32 = sizes
        .iter()
        .rev()
        .skip(1)
        .map(|&size| size * LINE_GAP)
        .sum();
    lines + gaps
}

/// Ascent to descent at `size`.
fn line_height(faces: &Faces, size: f32) -> f32 {
    let scaled = faces.regular.as_scaled(PxScale::from(size));
    scaled.ascent() - scaled.descent()
}

fn line_width(faces: &Faces, spans: &[Span], size: f32) -> f32 {
    let mut width: f32 = spans
        .iter()
        .map(|span| Bitmap::text_width(faces.font(span.bold, span.italic).0, size, &span.text))
        .sum();
    // A slanted end leans out to the right.
    if spans
        .last()
        .is_some_and(|span| faces.font(span.bold, span.italic).1)
    {
        width += line_height(faces, size) * SLANT;
    }
    width
}

/// Draws a span with its top left at (`x`, `top`); returns its advance.
fn draw_span(
    bitmap: &mut Bitmap,
    faces: &Faces,
    span: &Span,
    size: f32,
    (x, top): (f32, f32),
    line: f32,
) -> f32 {
    let (face, slanted) = faces.font(span.bold, span.italic);
    if !slanted {
        return bitmap.draw_text(face, size, x, top, &span.text);
    }

    // Upright into a scratch bitmap, then sheared: rows higher up shift further right.
    let advance = Bitmap::text_width(face, size, &span.text);
    let slant_room = line * SLANT;
    let width = (advance + slant_room).ceil().max(1.0) as u32;
    let height = line.ceil().max(1.0) as u32;
    let mut scratch = Bitmap::new(width, height);
    scratch.draw_text(face, size, 0.0, 0.0, &span.text);

    let ascent = faces.regular.as_scaled(PxScale::from(size)).ascent();
    for y in 0..height {
        let shift = ((ascent - y as f32) * SLANT).round() as i64;
        for sx in 0..width {
            if scratch.get(sx, y) {
                bitmap.set(
                    x.round() as i64 + i64::from(sx) + shift,
                    top.round() as i64 + i64::from(y),
                    true,
                );
            }
        }
    }
    advance
}

/// Turns the landscape label a quarter: its left edge becomes the first raster line, its top
/// the tape's far side.
fn rotate_into_raster(landscape: &Bitmap) -> Bitmap {
    let (width, height) = (landscape.width(), landscape.height());
    let mut raster = Bitmap::new(height, width);
    for y in 0..height {
        for x in 0..width {
            if landscape.get(x, y) {
                raster.set(i64::from(height - 1 - y), i64::from(x), true);
            }
        }
    }
    raster
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label::Typeface;

    #[test]
    fn markdown_spans() {
        let spans = parse_line(r"a **b** *c* ***d*** \*e");
        let styles: Vec<_> = spans
            .iter()
            .map(|span| (span.text.as_str(), span.bold, span.italic))
            .collect();
        assert_eq!(
            styles,
            [
                ("a ", false, false),
                ("b", true, false),
                (" ", false, false),
                ("c", false, true),
                (" ", false, false),
                ("d", true, true),
                (" *e", false, false),
            ]
        );
    }

    #[test]
    fn continuous_grows_with_the_text_and_fits_across() {
        let along = |text: &str| TextLabel::new(text).render(696, Length::Free, Run::AlongTape);
        let short = along("Hi");
        let long = along("A much longer line of text");
        assert_eq!((short.width(), long.width()), (696, 696));
        assert!(long.height() > short.height() * 3);
        assert!(short.height() >= MIN_CONTINUOUS_LINES);
        assert!(long.black_dots() > 0);

        // More lines: smaller text, so a shorter label for the same words.
        assert!(along("alpha\nbeta").height() < along("alpha beta").height());
    }

    #[test]
    fn a_length_limit_shrinks_the_text_not_the_tape() {
        let text = TextLabel::new("A much longer line of text");
        let free = text.render(696, Length::Free, Run::AlongTape);
        let limited = text.render(696, Length::AtMost(1392), Run::AlongTape);
        assert!(free.height() > 1392);
        assert!(
            limited.height() <= 1392 && limited.height() > 1300,
            "{}",
            limited.height()
        );
        assert!(limited.black_dots() < free.black_dots());

        // A short text stays short.
        let short = TextLabel::new("Hi").render(696, Length::AtMost(1392), Run::AlongTape);
        assert!(short.height() < 700);
    }

    #[test]
    fn fine_has_twice_the_lines_and_the_same_width() {
        let text = TextLabel::new("Rack 4").typeface(Typeface::Serif);
        let die_cut = text.render_fine(696, Length::Fixed(1100), Run::AlongTape);
        assert_eq!((die_cut.width(), die_cut.height()), (696, 2200));

        let normal = text.render(696, Length::Free, Run::AlongTape);
        let fine = text.render_fine(696, Length::Free, Run::AlongTape);
        assert_eq!(fine.width(), 696);
        let ratio = fine.height() as f32 / normal.height() as f32;
        assert!((ratio - 2.0).abs() < 0.02, "{ratio}");
        // About as much ink per area.
        let ink = |b: &Bitmap| b.black_dots() as f32 / b.height() as f32;
        assert!((ink(&fine) / ink(&normal) - 1.0).abs() < 0.15);

        let short = TextLabel::new(".").render_fine(696, Length::Free, Run::AlongTape);
        assert!(short.height() >= 2 * MIN_CONTINUOUS_LINES);
    }

    #[test]
    fn typefaces_differ() {
        let width = |typeface| {
            TextLabel::new("Wauw dit is echt")
                .typeface(typeface)
                .render(696, Length::Free, Run::AlongTape)
                .height()
        };
        // Barlow is the narrow one.
        assert!(width(Typeface::Barlow) < width(Typeface::Roboto));
        assert!(width(Typeface::Barlow) < width(Typeface::Serif));
    }

    #[test]
    fn on_tape_the_feed_is_the_margin_at_the_ends() {
        // Along the tape: ink from (almost) the first raster line to the last.
        let label = TextLabel::new("HH").landscape(696, Length::Free, Run::AlongTape);
        let inked = |x: u32| (0..label.height()).any(|y| label.get(x, y));
        let first = (0..label.width()).find(|&x| inked(x)).expect("ink");
        let last = (0..label.width()).rev().find(|&x| inked(x)).expect("ink");
        assert_eq!(first, 0, "first ink");
        assert_eq!(
            label.width() - 1,
            last,
            "last ink {last} of {}",
            label.width()
        );

        // A die-cut label keeps its margin at the ends.
        let die_cut = TextLabel::new("HH").landscape(696, Length::Fixed(1100), Run::AlongTape);
        let inked = |x: u32| (0..die_cut.height()).any(|y| die_cut.get(x, y));
        assert!((0..42).all(|x| !inked(x)));
    }

    #[test]
    fn sideways_lines_span_the_tape() {
        let label =
            TextLabel::new("Rack 4\ndo not move").render(696, Length::Free, Run::AcrossTape);
        assert_eq!(label.width(), 696);
        // Two lines stacked along the tape: shorter than wide.
        assert!(label.height() < 696, "{}", label.height());
        let edge_ink = (0..label.height()).any(|y| label.get(0, y) || label.get(695, y));
        assert!(!edge_ink, "text ran off the tape");
    }

    #[test]
    fn filled_lines_each_span_the_width() {
        let text = TextLabel::new("short\na much longer second line").fill_lines(true);
        let margin = 696.0 * MARGIN;
        let margins = Margins {
            height: margin,
            width: margin,
        };
        let sizes = text.sizes(margins, Some(1400.0), Some(696.0));
        let uniform = text.fit(margins, Some(1400.0), Some(696.0));

        // The short line grows, the long one keeps the one size; both span the room.
        assert!(sizes[0] > uniform * 3.0, "{sizes:?} vs {uniform}");
        let room = 696.0 - 2.0 * margin;
        for (line, size) in text.lines.iter().zip(&sizes) {
            let width = line_width(text.faces(), line, *size);
            assert!((width - room).abs() < room * 0.02, "{width} vs {room}");
        }

        let label = text.landscape(696, Length::Fixed(1400), Run::AcrossTape);
        assert_eq!((label.width(), label.height()), (696, 1400));
    }

    #[test]
    fn filled_lines_shrink_together_to_fit_the_height() {
        let label = TextLabel::new("a\nb\nc").fill_lines(true).render(
            696,
            Length::Fixed(271),
            Run::AlongTape,
        );
        assert_eq!((label.width(), label.height()), (696, 271));
        let edge_ink = (0..label.width()).any(|x| label.get(x, 0) || label.get(x, 270))
            || (0..label.height()).any(|y| label.get(0, y) || label.get(695, y));
        assert!(!edge_ink, "text ran off the label");
    }

    #[test]
    fn die_cut_keeps_its_length_and_the_ink_stays_inside() {
        for run in [Run::AlongTape, Run::AcrossTape] {
            let label = TextLabel::new("**Something rather long for a small label**").render(
                696,
                Length::Fixed(271),
                run,
            );
            assert_eq!((label.width(), label.height()), (696, 271), "{run:?}");
            let edge_ink = (0..label.width()).any(|x| label.get(x, 0) || label.get(x, 270));
            assert!(!edge_ink, "text ran off the label ({run:?})");
            assert!(label.black_dots() > 200, "{run:?}");
        }
    }

    #[test]
    fn blank_text_is_empty() {
        assert!(TextLabel::new("\n  \n").is_empty());
        assert!(!TextLabel::new("\nx\n").is_empty());
    }
}
