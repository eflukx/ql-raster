//! Brother QL label printers (QL-710W, 720NW, 800, 810W, 820NWB): labels drawn as bitmaps,
//! turned into raster jobs and printed over the network (TCP 9100) or USB (Windows, without
//! Brother's driver), with the printer's status.
//!
//! - [`label`]: labels as 1-bit bitmaps: [`label::Bitmap`], [`label::TextLabel`], typefaces.
//! - [`media`]: every tape and label size, with its print area on the head.
//! - [`raster`]: a bitmap as raster lines for the 720-pin head; PackBits.
//! - [`job`]: the command stream of a print job (pages, cutting, resolution, compression).
//! - [`commands`]: command codes and parameter blocks.
//! - [`status`]: the 32-byte status block (model, errors, loaded media).
//! - [`net`]: sending a job over TCP 9100; status, model, name and serial over SNMP; searching
//!   the network for printers.
//! - [`usb`]: the same on USB, addressed as `usb:<serial>`.
//!
//! Checked against Brother's "Raster Command Reference" for these models (in `doc/`).

pub mod commands;
pub mod job;
pub mod label;
pub mod media;
#[cfg(feature = "net")]
pub mod net;
pub mod raster;
pub mod status;
pub mod usb;

pub use media::{Media, MediaShape};
pub use status::{MediaKind, Model, Status};

/// Errors talking to a printer.
#[derive(Debug)]
pub enum PrinterError {
    /// The network or the USB device failed.
    Io(std::io::Error),
    /// The printer didn't answer the SNMP request.
    Snmp,
    /// The status block had the wrong size.
    InvalidStatus(usize),
}

impl std::fmt::Display for PrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Snmp => f.write_str("no SNMP answer from the printer"),
            Self::InvalidStatus(size) => {
                write!(f, "the printer sent an invalid status ({size} bytes)")
            }
        }
    }
}

impl std::error::Error for PrinterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PrinterError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
