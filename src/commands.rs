use bitflags::bitflags;

use crate::{prelude::*, printer::PTouchPrinter, status::MediaKind};

// Raw command API for the PTouch device.
/// This provides low-level access to the device (if desired)
pub trait Commands {
    /// Null command
    fn null(&mut self) -> Result<()>;

    /// Init command, sets up the device for printing
    fn init(&mut self) -> Result<()>;

    /// Invalidate command, resets the device
    fn invalidate(&mut self) -> Result<()>;

    /// Issue a status request
    // fn status_req(&mut self) -> Result<()>;

    /// Read a status response with the provided timeout
    // fn read_status(&mut self) -> Result<Status>;

    /// Switch mode, required for raster printing
    fn switch_mode(&mut self, mode: Mode) -> Result<()>;

    /// Set status notify (printer automatically sends status on change)
    // fn set_status_notify(&mut self, enabled: bool) -> Result<()>;

    /// Set print information
    fn set_print_info(&mut self, info: &PrintInfo) -> Result<()>;

    /// Set various mode flags
    fn set_various_mode(&mut self, mode: VariousMode) -> Result<()>;

    /// Set advanced mode flags
    fn set_advanced_mode(&mut self, mode: AdvancedMode) -> Result<()>;

    /// Set pre/post print margin
    fn set_margin(&mut self, dots: u16) -> Result<()>;

    /// Set print page number
    fn set_page_no(&mut self, no: u8) -> Result<()>;

    /// Set compression mode (None or Tiff).
    /// Note TIFF mode is currently... broken
    fn set_compression_mode(&mut self, mode: CompressionMode) -> Result<()>;

    /// Transfer raster data
    fn transfer_raster_line(&mut self, data: &[u8]) -> Result<()>;

    /// Send a zero raster line
    fn raster_zero(&mut self) -> Result<()>;

    /// Start a print
    fn print(&mut self) -> Result<()>;

    /// Start a print and feed
    fn print_and_feed(&mut self) -> Result<()>;
}

/// Low-level command API implementation
impl<I: PTouchInterface> Commands for PTouchPrinter<I> {
    fn null(&mut self) -> Result<()> {
        self.write([0x00])
    }

    fn invalidate(&mut self) -> Result<()> {
        self.write([0u8; 400])
    }

    fn init(&mut self) -> Result<()> {
        self.write([0x1b, 0x40])
    }

    // fn read_status(&mut self) -> Result<Status> {
    //     let response = self.send_raw_with_response([0x1b, 0x69, 0x53])?;

    //     println!("RESPONSE: {response:?}");

    //     // let status = Status::from(status_raw);
    //     // debug!("Status: {:?}", status);
    //     // trace!("Raw status: {:?}", &status_raw);
    //     // Ok(status)

    //     Status::try_from(response.as_slice())
    // }

    fn switch_mode(&mut self, mode: Mode) -> Result<()> {
        self.write([0x1b, 0x69, 0x61, mode as u8])
    }

    // fn set_status_notify(&mut self, enabled: bool) -> Result<()> {
    //     let en = match enabled {
    //         true => 0,
    //         false => 1,
    //     };

    //     self.write(&[0x1b, 0x69, 0x21, en])
    // }

    fn set_print_info(&mut self, info: &PrintInfo) -> Result<()> {
        let mut buff = [0u8; 13];

        // debug!("Set print info: {:?}", info);

        // Command header
        buff[0] = 0x1b;
        buff[1] = 0x69;
        buff[2] = 0x7a;

        if let Some(i) = &info.kind {
            buff[3] |= 0x02;
            buff[4] = u8::from(*i)
        }

        if let Some(w) = &info.width {
            buff[3] |= 0x04;
            buff[5] = *w;
        }

        if let Some(l) = &info.length {
            buff[3] |= 0x08;
            buff[6] = *l;
        }

        let raster_bytes = info.raster_no.to_le_bytes();
        buff[7..11].copy_from_slice(&raster_bytes);

        if info.recover {
            buff[3] |= 0x80;
        }

        self.write(&buff)
    }

    fn set_various_mode(&mut self, mode: VariousMode) -> Result<()> {
        // debug!("Set various mode: {:?}", mode);

        self.write([0x1b, 0x69, 0x4d, mode.bits()])
    }

    fn set_advanced_mode(&mut self, mode: AdvancedMode) -> Result<()> {
        // debug!("Set advanced mode: {:?}", mode);

        self.write([0x1b, 0x69, 0x4b, mode.bits()])
    }

    fn set_margin(&mut self, dots: u16) -> Result<()> {
        // debug!("Set margin: {:?}", dots);

        self.write([0x1b, 0x69, 0x64, dots as u8, (dots >> 8) as u8])
    }

    fn set_page_no(&mut self, no: u8) -> Result<()> {
        // debug!("Set page no: {:?}", no);
        self.write([0x1b, 0x69, 0x41, no])
    }

    fn set_compression_mode(&mut self, mode: CompressionMode) -> Result<()> {
        // debug!("Set compression mode: {:?}", mode);

        self.write([0x4D, mode as u8])
    }

    // We assume an uncomressed line!
    fn transfer_raster_line(&mut self, data: &[u8]) -> Result<()> {

        let mut buff = Vec::with_capacity(data.len() + 3);
        buff.push(0x67); // Transfer raster data command


        // alternative 'engine' (used by handheld labelers)
        // buff.push((data.len() & 0xFF) as u8);
        // buff.push((data.len() >> 8) as u8);

        buff.push(0); // 'always 0'
        buff.push(data.len() as u8); // add data

        buff.extend_from_slice(data);

        // trace!("Raster transfer: {:02x?}", &buff[..3 + data.len()]);
        println!("Raster transfer: {:02x?}", &buff);

        self.write(buff.as_slice())
    }

    fn raster_zero(&mut self) -> Result<()> {
        // debug!("Raster zero line");

        self.write([0x5a])
    }

    fn print(&mut self) -> Result<()> {
        // debug!("Print command");
        self.write([0x0c])
    }

    fn print_and_feed(&mut self) -> Result<()> {
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
}

impl Default for PrintInfo {
    fn default() -> Self {
        Self {
            kind: None,
            width: None,
            length: Some(0),
            raster_no: 0,
            recover: true,
        }
    }
}

/// Compression mode enumeration
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum CompressionMode {
    None = 0x00,
    Tiff = 0x02,
}
