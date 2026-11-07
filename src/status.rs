//! The 32-byte status block (Raster Command Reference, "Status information").

use std::fmt;

use crate::PrinterError;
use crate::media::Media;

/// Printer model (status byte 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// QL-710W.
    Ql710w,
    /// QL-720NW.
    Ql720nw,
    /// QL-800.
    Ql800,
    /// QL-810W.
    Ql810w,
    /// QL-820NWB.
    Ql820nwb,
    /// Something else.
    Other(u8),
}

impl Model {
    /// Whether it prints in high resolution (300 × 600 dpi): every known model; an unknown one
    /// is taken not to.
    #[must_use]
    pub fn high_resolution(self) -> bool {
        !matches!(self, Self::Other(_))
    }

    /// Whether it takes TIFF (PackBits) compressed raster lines: the QL-710W, 720NW, 810W and
    /// 820NWB do; the QL-800 doesn't (their references).
    #[must_use]
    pub fn compression(self) -> bool {
        matches!(
            self,
            Self::Ql710w | Self::Ql720nw | Self::Ql810w | Self::Ql820nwb
        )
    }

    /// Its code in the status (byte 4).
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::Ql710w => 0x36,
            Self::Ql720nw => 0x37,
            Self::Ql800 => 0x38,
            Self::Ql810w => 0x39,
            Self::Ql820nwb => 0x41,
            Self::Other(code) => code,
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            0x36 => Self::Ql710w,
            0x37 => Self::Ql720nw,
            0x38 => Self::Ql800,
            0x39 => Self::Ql810w,
            0x41 => Self::Ql820nwb,
            other => Self::Other(other),
        }
    }
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ql710w => f.write_str("QL-710W"),
            Self::Ql720nw => f.write_str("QL-720NW"),
            Self::Ql800 => f.write_str("QL-800"),
            Self::Ql810w => f.write_str("QL-810W"),
            Self::Ql820nwb => f.write_str("QL-820NWB"),
            Self::Other(code) => write!(f, "model 0x{code:02x}"),
        }
    }
}

/// The loaded media kind (status byte 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    /// Nothing loaded.
    None,
    /// Continuous length tape (paper or film).
    Continuous,
    /// Die-cut labels.
    DieCut,
    /// Something else.
    Other(u8),
}

impl MediaKind {
    fn from_code(code: u8) -> Self {
        match code {
            0x00 => Self::None,
            // 0Ah in the QL-800 reference; 4Ah on some firmware (seen on our QL-810W).
            0x0a | 0x4a => Self::Continuous,
            0x0b | 0x4b => Self::DieCut,
            other => Self::Other(other),
        }
    }

    /// The code for the "print information" command.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::None => 0x00,
            Self::Continuous => 0x0a,
            Self::DieCut => 0x0b,
            Self::Other(code) => code,
        }
    }
}

/// Error bits (status bytes 8 and 9), as named in the reference.
const ERRORS: [(u16, &str); 14] = [
    (0x0001, "no media"),
    (0x0002, "end of media"),
    (0x0004, "cutter jam"),
    (0x0010, "printer in use"),
    (0x0020, "printer turned off"),
    (0x0040, "high-voltage adapter"),
    (0x0080, "fan motor error"),
    (0x0100, "replace media"),
    (0x0200, "expansion buffer full"),
    (0x0400, "communication error"),
    (0x0800, "communication buffer full"),
    (0x1000, "cover open"),
    (0x2000, "overheating"),
    (0x8000, "system error"),
];

/// A decoded status block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// The model.
    pub model: Model,
    /// Raw error bits (byte 8 low, byte 9 high).
    pub errors: u16,
    /// Media width in millimetres.
    pub media_width_mm: u8,
    /// Media length in millimetres (0 for continuous tape).
    pub media_length_mm: u8,
    /// Media kind.
    pub media_kind: MediaKind,
}

impl TryFrom<&[u8]> for Status {
    type Error = PrinterError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let block: &[u8; 32] = bytes
            .try_into()
            .map_err(|_| PrinterError::InvalidStatus(bytes.len()))?;

        Ok(Self {
            model: Model::from_code(block[4]),
            errors: u16::from_le_bytes([block[8], block[9]]),
            media_width_mm: block[10],
            media_length_mm: block[17],
            media_kind: MediaKind::from_code(block[11]),
        })
    }
}

impl Status {
    /// The names of the error bits that are set.
    #[must_use]
    pub fn error_names(&self) -> Vec<&'static str> {
        ERRORS
            .iter()
            .filter(|(bit, _)| self.errors & bit != 0)
            .map(|(_, name)| *name)
            .collect()
    }

    /// Whether the printer can print: no errors and media loaded.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.errors == 0 && self.media_kind != MediaKind::None
    }

    /// The loaded medium, if it is one of the media table.
    #[must_use]
    pub fn loaded_media(&self) -> Option<&'static Media> {
        let continuous = match self.media_kind {
            MediaKind::Continuous => true,
            MediaKind::DieCut => false,
            MediaKind::None | MediaKind::Other(_) => return None,
        };
        Media::reported(continuous, self.media_width_mm, self.media_length_mm)
    }

    /// Whether `media` is loaded.
    #[must_use]
    pub fn has_media(&self, media: &Media) -> bool {
        self.loaded_media() == Some(media)
    }

    /// The loaded media in words: `62 mm continuous`, `29 × 90 mm die-cut`.
    #[must_use]
    pub fn media_description(&self) -> String {
        if let Some(media) = self.loaded_media() {
            return media.to_string();
        }

        match self.media_kind {
            MediaKind::None => "no media".to_owned(),
            MediaKind::Continuous => format!("{} mm continuous", self.media_width_mm),
            MediaKind::DieCut => format!(
                "{} × {} mm die-cut",
                self.media_width_mm, self.media_length_mm
            ),
            MediaKind::Other(code) => {
                format!("{} mm, media type 0x{code:02x}", self.media_width_mm)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_models_have_high_resolution() {
        for code in [0x36, 0x37, 0x38, 0x39, 0x41] {
            let model = Model::from_code(code);
            assert_eq!(model.code(), code);
            assert!(model.high_resolution(), "{model}");
        }
        assert!(!Model::Other(0x64).high_resolution());
    }

    /// A QL-810W answer captured by the ql-raster fork: 50 mm continuous, no errors.
    const QL810W_50MM: [u8; 32] = [
        128, 32, 66, 52, 57, 48, 4, 0, 0, 0, 50, 10, 0, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0,
        0, 0, 0, 0, 0,
    ];

    #[test]
    fn decodes_a_real_status() {
        let status = Status::try_from(QL810W_50MM.as_slice()).expect("32 bytes");

        assert_eq!(status.model, Model::Ql810w);
        assert_eq!(status.media_kind, MediaKind::Continuous);
        assert_eq!(status.media_width_mm, 50);
        assert!(status.is_ready());
        assert_eq!(status.loaded_media(), Media::by_id("50"));
        assert!(!status.has_media(Media::by_id("62").expect("known")));
        assert_eq!(status.media_description(), "50 mm continuous");
    }

    #[test]
    fn names_errors_and_rejects_bad_sizes() {
        let mut block = QL810W_50MM;
        block[8] = 0x01;
        block[9] = 0x10;

        let status = Status::try_from(block.as_slice()).expect("32 bytes");
        assert_eq!(status.error_names(), vec!["no media", "cover open"]);
        assert!(!status.is_ready());

        assert!(Status::try_from(&block[..31]).is_err());
    }
}
