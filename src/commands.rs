use bitflags::bitflags;

use crate::{prelude::*, printer::PTouchPrinter, status::MediaKind};

// Raw command API for the PTouch device.
/// This provides low-level access to the device (if desired)
pub trait Commands {
    /// Null command
    fn null(&mut self) -> PTResult<()>;

    /// Init command, sets up the device for printing
    fn init(&mut self) -> PTResult<()>;

    /// Invalidate command, resets the device
    fn invalidate(&mut self) -> PTResult<()>;

    /// Issue a status request
    // fn status_req(&mut self) -> Result<()>;

    /// Read a status response with the provided timeout
    // fn read_status(&mut self) -> Result<Status>;

    /// Set status notify (printer automatically sends status on change)
    // fn set_status_notify(&mut self, enabled: bool) -> Result<()>;

    /// Switch mode, required for raster printing
    fn switch_mode(&mut self, mode: Mode) -> PTResult<()>;

    /// Set print information
    fn set_print_info(&mut self, info: &PrintInfo) -> PTResult<()>;

    /// Set various mode flags
    fn set_various_mode(&mut self, mode: VariousMode) -> PTResult<()>;

    /// Set advanced mode flags
    fn set_advanced_mode(&mut self, mode: AdvancedMode) -> PTResult<()>;

    /// Set pre/post print margin
    fn set_margin(&mut self, dots: u16) -> PTResult<()>;

    /// Set print page number
    fn set_page_no(&mut self, no: u8) -> PTResult<()>;

    /// Set compression mode (None or Tiff).
    /// Note TIFF mode is currently... broken
    fn set_compression_mode(&mut self, mode: CompressionMode) -> PTResult<()>;

    /// Transfer raster data
    fn transfer_raster_line(&mut self, data: &[u8]) -> PTResult<()>;

    /// Send a zero raster line
    fn raster_zero(&mut self) -> PTResult<()>;

    /// Start a print
    fn print(&mut self) -> PTResult<()>;

    /// Start a print and feed
    fn print_and_feed(&mut self) -> PTResult<()>;
}

/// Low-level command API implementation
impl<I: PTouchInterface> Commands for PTouchPrinter<I> {
    fn null(&mut self) -> PTResult<()> {
        self.write([0x00])
    }

    fn invalidate(&mut self) -> PTResult<()> {
        self.write([0u8; 400])
    }

    fn init(&mut self) -> PTResult<()> {
        self.write([0x1b, 0x40])
    }

    fn switch_mode(&mut self, mode: Mode) -> PTResult<()> {
        self.write([0x1b, 0x69, 0x61, mode as u8])
    }

    // fn set_status_notify(&mut self, enabled: bool) -> Result<()> {
    //     let en = match enabled {
    //         true => 0,
    //         false => 1,
    //     };

    //     self.write(&[0x1b, 0x69, 0x21, en])
    // }

    fn set_print_info(&mut self, info: &PrintInfo) -> PTResult<()> {
        self.write(info.to_bytes())
    }

    fn set_various_mode(&mut self, mode: VariousMode) -> PTResult<()> {
        // debug!("Set various mode: {:?}", mode);

        self.write([0x1b, 0x69, 0x4d, mode.bits()])
    }

    fn set_advanced_mode(&mut self, mode: AdvancedMode) -> PTResult<()> {
        // debug!("Set advanced mode: {:?}", mode);

        self.write([0x1b, 0x69, 0x4b, mode.bits()])
    }

    fn set_margin(&mut self, dots: u16) -> PTResult<()> {
        // debug!("Set margin: {:?}", dots);

        self.write([0x1b, 0x69, 0x64, dots as u8, (dots >> 8) as u8])
    }

    fn set_page_no(&mut self, no: u8) -> PTResult<()> {
        // debug!("Set page no: {:?}", no);
        self.write([0x1b, 0x69, 0x41, no])
    }

    fn set_compression_mode(&mut self, mode: CompressionMode) -> PTResult<()> {
        // debug!("Set compression mode: {:?}", mode);

        self.write([0x4D, mode as u8])
    }

    // We assume an uncomressed line!
    fn transfer_raster_line(&mut self, data: &[u8]) -> PTResult<()> {
        let mut buff = Vec::with_capacity(data.len() + 3);
        buff.push(0x67); // Transfer raster data command

        // alternative 'engine' (used by handheld labelers)
        // buff.push((data.len() & 0xFF) as u8);
        // buff.push((data.len() >> 8) as u8);

        buff.push(0); // 'always 0'
        buff.push(data.len() as u8); // add data

        buff.extend_from_slice(data);

        // trace!("Raster transfer: {:02x?}", &buff[..3 + data.len()]);
        // println!("Raster transfer: {:02x?}", &buff);

        self.write(buff.as_slice())
    }

    /// Zero raster graphics (5Ah): valid only in TIFF compression mode (Raster Command
    /// Reference, QL-710W). Without compression send a `g` line of zeros: over USB the printer
    /// stops at a 5Ah.
    fn raster_zero(&mut self) -> PTResult<()> {
        // debug!("Raster zero line");

        self.write([0x5a])
    }

    fn print(&mut self) -> PTResult<()> {
        // debug!("Print command");
        self.write([0x0c])
    }

    fn print_and_feed(&mut self) -> PTResult<()> {
        // debug!("Print feed command");
        self.write([0x1a])
    }
}

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
            buff[4] = match kind {
                MediaKind::ContinuousLengthTape => 0x0a,
                MediaKind::DieCutLabels => 0x0b,
                other => u8::from(other),
            };
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
