//! The media of the 720-pin QL printers (QL-500 … QL-820NWB): every continuous tape and die-cut
//! label size, with where it sits on the print head.
//!
//! All numbers come from Brother's "Raster Command Reference" for the QL-600/700/710W/720NW
//! (v1.02, `doc/cv_ql600710720_eng_raster_102.pdf`):
//!
//! - 2.3.2 Page size: print area width and length in dots (300 dpi).
//! - 2.3.3 / 2.3.4: feed (margin) and the minimum and maximum length of continuous tape.
//! - 2.3.5 Raster line: the pins left of, in, and right of the print area per medium.
//! - 4 (3): the media width and length the printer reports in its status (60 × 86 mm reports
//!   a length of 87).
//!
//! A raster line covers all 720 pins. Dot `x` of a label row goes to pin
//! `left_pins + x`; the print area holds `print_pins` dots.
//!
//! ```
//! use ql_raster::media::Media;
//!
//! let tape = Media::by_id("62").unwrap();
//! assert_eq!((tape.left_pins, tape.print_pins), (12, 696));
//!
//! // What the printer says is loaded: die-cut, 29 × 90 mm.
//! let loaded = Media::reported(false, 29, 90).unwrap();
//! assert_eq!(loaded.id, "29x90");
//! assert_eq!(loaded.print_length_dots, 991);
//! ```

use std::fmt;

use crate::commands::PrintInfo;
use crate::status::{MediaKind, Status};

/// Pins on the print head of these printers.
pub const HEAD_PINS: u16 = 720;

/// Feed (margin) for continuous tape: the minimum, 3 mm (2.3.3).
pub const CONTINUOUS_FEED_DOTS: u16 = 35;

/// Shortest continuous label: 12.7 mm (2.3.4).
pub const CONTINUOUS_MIN_LENGTH_DOTS: u32 = 150;

/// Longest continuous label: 1 m (2.3.4).
pub const CONTINUOUS_MAX_LENGTH_DOTS: u32 = 11811;

/// The form of a medium.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MediaShape {
    /// Continuous length tape: the label is as long as the job.
    Continuous,
    /// Rectangular die-cut labels of a fixed size.
    DieCut,
    /// Round die-cut labels (reported as die-cut, width = length = diameter).
    Round,
}

/// One tape or label size.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Media {
    /// Short name: `62` (continuous, mm), `29x90` (die-cut), `d24` (round).
    pub id: &'static str,
    /// Brother's media id (the "ID" column of 2.3.2).
    pub brother_id: u16,
    /// The form.
    pub shape: MediaShape,
    /// Width in mm, as the status reports it and the print information command takes it.
    pub width_mm: u8,
    /// Length in mm as reported (0 for continuous tape).
    pub length_mm: u8,
    /// Pins left of the print area.
    pub left_pins: u16,
    /// Pins of the print area: the label's width in dots.
    pub print_pins: u16,
    /// Pins right of the print area.
    pub right_pins: u16,
    /// The print area's length in dots (0 for continuous tape: as long as the job).
    pub print_length_dots: u16,
}

const fn continuous(id: &'static str, brother_id: u16, width_mm: u8, pins: [u16; 3]) -> Media {
    Media {
        id,
        brother_id,
        shape: MediaShape::Continuous,
        width_mm,
        length_mm: 0,
        left_pins: pins[0],
        print_pins: pins[1],
        right_pins: pins[2],
        print_length_dots: 0,
    }
}

const fn die_cut(
    id: &'static str,
    brother_id: u16,
    shape: MediaShape,
    size_mm: [u8; 2],
    pins: [u16; 3],
    print_length_dots: u16,
) -> Media {
    Media {
        id,
        brother_id,
        shape,
        width_mm: size_mm[0],
        length_mm: size_mm[1],
        left_pins: pins[0],
        print_pins: pins[1],
        right_pins: pins[2],
        print_length_dots,
    }
}

use MediaShape::{DieCut, Round};

/// Every medium of the 720-pin printers, continuous tape first, narrow to wide.
pub const MEDIA: [Media; 20] = [
    continuous("12", 257, 12, [585, 106, 29]),
    continuous("29", 258, 29, [408, 306, 6]),
    continuous("38", 264, 38, [295, 413, 12]),
    continuous("50", 262, 50, [154, 554, 12]),
    continuous("54", 261, 54, [130, 590, 0]),
    continuous("62", 259, 62, [12, 696, 12]),
    die_cut("17x54", 269, DieCut, [17, 54], [555, 165, 0], 566),
    die_cut("17x87", 270, DieCut, [17, 87], [555, 165, 0], 956),
    die_cut("23x23", 370, DieCut, [23, 23], [442, 236, 42], 202),
    die_cut("29x42", 358, DieCut, [29, 42], [408, 306, 6], 425),
    die_cut("29x90", 271, DieCut, [29, 90], [408, 306, 6], 991),
    die_cut("38x90", 272, DieCut, [38, 90], [295, 413, 12], 991),
    die_cut("39x48", 367, DieCut, [39, 48], [289, 425, 6], 495),
    die_cut("52x29", 374, DieCut, [52, 29], [142, 578, 0], 271),
    die_cut("60x86", 383, DieCut, [60, 87], [24, 672, 24], 954),
    die_cut("62x29", 274, DieCut, [62, 29], [12, 696, 12], 271),
    die_cut("62x100", 275, DieCut, [62, 100], [12, 696, 12], 1109),
    die_cut("d12", 362, Round, [12, 12], [513, 94, 113], 94),
    die_cut("d24", 363, Round, [24, 24], [442, 236, 42], 236),
    die_cut("d58", 273, Round, [58, 58], [51, 618, 51], 618),
];

/// Why a label doesn't fit a medium.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MediaError {
    /// Wider than the print area.
    TooWide {
        /// The label's width in dots.
        width: u32,
        /// The print area's width.
        print_pins: u16,
    },
    /// A die-cut label must be exactly the print area's length.
    WrongLength {
        /// The label's length in dots.
        length: u32,
        /// The print area's length.
        print_length_dots: u16,
    },
    /// Continuous tape: shorter than 12.7 mm or longer than 1 m.
    LengthOutOfRange {
        /// The label's length in dots.
        length: u32,
    },
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooWide { width, print_pins } => {
                write!(f, "label is {width} dots wide, the print area {print_pins}")
            }
            Self::WrongLength {
                length,
                print_length_dots,
            } => write!(
                f,
                "label is {length} dots long, the die-cut print area {print_length_dots}"
            ),
            Self::LengthOutOfRange { length } => write!(
                f,
                "label is {length} dots long; continuous tape takes \
                 {CONTINUOUS_MIN_LENGTH_DOTS} to {CONTINUOUS_MAX_LENGTH_DOTS}"
            ),
        }
    }
}

impl std::error::Error for MediaError {}

impl Media {
    /// Every medium.
    #[must_use]
    pub fn all() -> &'static [Media] {
        &MEDIA
    }

    /// The medium with this id (`62`, `29x90`, `d24`).
    #[must_use]
    pub fn by_id(id: &str) -> Option<&'static Media> {
        MEDIA.iter().find(|media| media.id.eq_ignore_ascii_case(id))
    }

    /// The medium a printer reports: continuous or die-cut, width and length in mm (status
    /// bytes 11, 10 and 17).
    #[must_use]
    pub fn reported(continuous: bool, width_mm: u8, length_mm: u8) -> Option<&'static Media> {
        MEDIA.iter().find(|media| {
            media.is_continuous() == continuous
                && media.width_mm == width_mm
                && (continuous || media.length_mm == length_mm)
        })
    }

    /// The medium loaded according to a status; `None` without media or for unknown media.
    #[must_use]
    pub fn from_status(status: &Status) -> Option<&'static Media> {
        let continuous = match status.media_kind {
            MediaKind::ContinuousLengthTape => true,
            MediaKind::DieCutLabels => false,
            _ => return None,
        };
        Self::reported(continuous, status.media_width, status.media_length)
    }

    /// Whether this medium is what a status reports as loaded.
    #[must_use]
    pub fn is_loaded(&self, status: &Status) -> bool {
        Self::from_status(status) == Some(self)
    }

    /// Whether this is continuous tape.
    #[must_use]
    pub fn is_continuous(&self) -> bool {
        self.shape == MediaShape::Continuous
    }

    /// The media kind of this medium.
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        if self.is_continuous() {
            MediaKind::ContinuousLengthTape
        } else {
            MediaKind::DieCutLabels
        }
    }

    /// The media type byte of the print information command (`ESC i z`, n2): 0Ah continuous,
    /// 0Bh die-cut. (The status reports 4Ah and 4Bh for the same media.)
    #[must_use]
    pub fn print_info_type(&self) -> u8 {
        if self.is_continuous() {
            0x0a
        } else {
            0x0b
        }
    }

    /// The feed to set with `ESC i d`: 35 dots on continuous tape, 0 on die-cut labels (their
    /// length offset is fixed, 2.3.3).
    #[must_use]
    pub fn feed_dots(&self) -> u16 {
        if self.is_continuous() {
            CONTINUOUS_FEED_DOTS
        } else {
            0
        }
    }

    /// The pin for dot `x` of a label row; `None` outside the print area.
    #[must_use]
    pub fn pin(&self, x: u32) -> Option<u16> {
        let x = u16::try_from(x).ok()?;
        (x < self.print_pins).then(|| self.left_pins + x)
    }

    /// Checks a label of `width` × `length` dots against this medium.
    pub fn check_size(&self, width: u32, length: u32) -> Result<(), MediaError> {
        if width > u32::from(self.print_pins) {
            return Err(MediaError::TooWide {
                width,
                print_pins: self.print_pins,
            });
        }

        if self.is_continuous() {
            if !(CONTINUOUS_MIN_LENGTH_DOTS..=CONTINUOUS_MAX_LENGTH_DOTS).contains(&length) {
                return Err(MediaError::LengthOutOfRange { length });
            }
        } else if length != u32::from(self.print_length_dots) {
            return Err(MediaError::WrongLength {
                length,
                print_length_dots: self.print_length_dots,
            });
        }

        Ok(())
    }

    /// Print information for a job of `raster_no` lines on this medium.
    #[must_use]
    pub fn print_info(&self, raster_no: u32) -> PrintInfo {
        PrintInfo {
            kind: Some(self.kind()),
            width: Some(self.width_mm),
            length: Some(self.length_mm),
            raster_no,
            recover: true,
        }
    }
}

impl fmt::Display for Media {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.shape {
            MediaShape::Continuous => write!(f, "{} mm continuous", self.width_mm),
            MediaShape::DieCut => write!(f, "{} × {} mm die-cut", self.width_mm, self.length_mm),
            MediaShape::Round => write!(f, "{} mm round", self.width_mm),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_medium_spans_the_head() {
        for media in Media::all() {
            assert_eq!(
                media.left_pins + media.print_pins + media.right_pins,
                HEAD_PINS,
                "{}",
                media.id
            );
        }
    }

    #[test]
    fn ids_are_unique_and_reports_unambiguous() {
        for (i, a) in MEDIA.iter().enumerate() {
            for b in &MEDIA[i + 1..] {
                assert_ne!(a.id, b.id);
                assert_ne!(a.brother_id, b.brother_id);
                assert!(
                    (a.is_continuous(), a.width_mm, a.length_mm)
                        != (b.is_continuous(), b.width_mm, b.length_mm),
                    "{} and {} report the same",
                    a.id,
                    b.id
                );
            }
        }
    }

    #[test]
    fn finds_media_by_what_the_printer_reports() {
        assert_eq!(Media::reported(true, 62, 0).map(|m| m.id), Some("62"));
        assert_eq!(Media::reported(true, 29, 0).map(|m| m.id), Some("29"));
        assert_eq!(Media::reported(false, 62, 29).map(|m| m.id), Some("62x29"));
        assert_eq!(Media::reported(false, 60, 87).map(|m| m.id), Some("60x86"));
        assert_eq!(Media::reported(false, 24, 24).map(|m| m.id), Some("d24"));
        assert_eq!(Media::reported(true, 102, 0), None, "wide printers only");
    }

    #[test]
    fn places_dots_in_the_print_area() {
        let tape = Media::by_id("29").unwrap();
        assert_eq!(tape.pin(0), Some(408));
        assert_eq!(tape.pin(305), Some(713));
        assert_eq!(tape.pin(306), None);
    }

    #[test]
    fn checks_label_sizes() {
        let tape = Media::by_id("62").unwrap();
        assert!(tape.check_size(696, 248).is_ok());
        assert!(matches!(
            tape.check_size(697, 248),
            Err(MediaError::TooWide { .. })
        ));
        assert!(matches!(
            tape.check_size(696, 100),
            Err(MediaError::LengthOutOfRange { .. })
        ));

        let labels = Media::by_id("62x29").unwrap();
        assert!(labels.check_size(696, 271).is_ok());
        assert!(matches!(
            labels.check_size(696, 300),
            Err(MediaError::WrongLength { .. })
        ));
    }

    #[test]
    fn print_information_follows_the_medium() {
        let labels = Media::by_id("62x100").unwrap();
        let info = labels.print_info(1109);
        assert_eq!(info.kind, Some(MediaKind::DieCutLabels));
        assert_eq!((info.width, info.length), (Some(62), Some(100)));
        assert_eq!((labels.print_info_type(), labels.feed_dots()), (0x0b, 0));

        let tape = Media::by_id("62").unwrap();
        assert_eq!((tape.print_info_type(), tape.feed_dots()), (0x0a, 35));
        assert_eq!(tape.to_string(), "62 mm continuous");
        assert_eq!(Media::by_id("d58").unwrap().to_string(), "58 mm round");
    }
}
