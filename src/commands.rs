//! Command codes and parameter blocks of the raster protocol (Raster Command Reference).

use bitflags::bitflags;

use crate::status::MediaKind;

/// Device mode for set_mode command
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub enum Mode {
    /// ESC/P mode (legacy hi-level Epson mode)
    EscP = 0x00,
    /// Raster mode, what this driver uses
    #[default]
    Raster = 0x01,
    /// Note PTouchTemplate is not available on most devices
    PTouchTemplate = 0x03,
}

bitflags! {
    /// Various mode flags
    #[derive(Copy, Clone, PartialEq, Debug)]
    pub struct VariousMode: u8 {
        const AUTO_CUT = (1 << 6);
        const MIRROR = (1 << 7);
    }
}

bitflags! {
    /// Advanced mode flags
    #[derive(Copy, Clone, PartialEq, Debug)]
    pub struct AdvancedMode: u8 {
        const HALF_CUT = (1 << 2);
        const NO_CHAIN = (1 << 3);
        const SPECIAL_TAPE = (1 << 4);
        const HIGH_RES = (1 << 6);
        const NO_BUFF_CLEAR = (1 << 7);
    }
}

/// Print information command
#[derive(Clone, PartialEq, Debug)]
pub struct PrintInfo {
    /// Media kind
    pub kind: Option<MediaKind>,
    /// Tape width in mm
    pub width: Option<u8>,
    /// Tape length, always set to 0
    pub length: Option<u8>,
    /// Raster number (??)
    pub raster_no: u32,
    /// Enable print recovery
    pub recover: bool,
    /// Priority to print quality over speed (n1 bit 0x40; QL-710W/720NW, 800 series; not
    /// with two-colour printing)
    pub quality: bool,
    /// A later page of a multi-page job (n9 = 1); the first page has 0
    pub later_page: bool,
}

impl PrintInfo {
    /// The command: `ESC i z n1 … n10`.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 13] {
        let mut buff = [0u8; 13];
        buff[..3].copy_from_slice(&[0x1b, 0x69, 0x7a]);

        if let Some(kind) = self.kind {
            buff[3] |= 0x02;
            // The status reports 4Ah / 4Bh; this command takes 0Ah / 0Bh.
            buff[4] = kind.code();
        }
        if let Some(width) = self.width {
            buff[3] |= 0x04;
            buff[5] = width;
        }
        if let Some(length) = self.length {
            buff[3] |= 0x08;
            buff[6] = length;
        }
        buff[7..11].copy_from_slice(&self.raster_no.to_le_bytes());
        if self.quality {
            buff[3] |= 0x40;
        }
        if self.recover {
            buff[3] |= 0x80;
        }
        buff[11] = u8::from(self.later_page);
        buff
    }
}

impl Default for PrintInfo {
    fn default() -> Self {
        Self {
            kind: None,
            width: None,
            length: Some(0),
            raster_no: 0,
            recover: true,
            quality: false,
            later_page: false,
        }
    }
}

/// Compression mode enumeration
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum CompressionMode {
    None = 0x00,
    Tiff = 0x02,
}
