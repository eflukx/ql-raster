//! Brother QL printers on USB, without Brother's driver ([`device`], Windows only).
//!
//! How to use: [`find`] lists them with their address (`usb:000H6Z560817`, the serial);
//! [`query_status`] and [`send_job`] take that address, as `net` takes a host. [`is_usb`] tells
//! the two apart.
//!
//! Working principle: the address names the printer by its USB serial, so it stays the same on
//! another USB port; each call finds the printer again and opens it for the moment. The job
//! bytes and the status are the same as over the network. A printer whose USB device has
//! another driver (WinUSB or libusb0 from Zadig) isn't seen: it needs Windows' "USB Printing
//! Support", which Windows picks by itself.

#[cfg(windows)]
pub mod device;

use crate::{PrinterError, Status};

/// The address prefix of a USB printer.
const PREFIX: &str = "usb:";

/// A Brother printer on USB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Its address: `usb:` and its serial.
    pub address: String,
    /// USB product id (`2043`h: QL-710W).
    pub product_id: u16,
}

/// Whether `address` is a USB printer's.
#[must_use]
pub fn is_usb(address: &str) -> bool {
    address.starts_with(PREFIX)
}

/// The Brother printers on USB.
#[must_use]
pub fn find() -> Vec<Found> {
    #[cfg(windows)]
    {
        device::find()
            .into_iter()
            .map(|printer| Found {
                address: format!("{PREFIX}{}", printer.serial),
                product_id: printer.product_id,
            })
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

/// Whether the USB printer at `address` is plugged in. Asks Windows, not the printer (no USB
/// traffic, so it doesn't hold the printer to USB): cheap enough to poll.
#[must_use]
pub fn is_present(address: &str) -> bool {
    is_usb(address)
        && find()
            .iter()
            .any(|found| found.address.eq_ignore_ascii_case(address))
}

/// The status of the USB printer at `address`.
pub fn query_status(address: &str) -> Result<Status, PrinterError> {
    #[cfg(windows)]
    {
        let status = open(address)?.status()?;
        Status::try_from(status.as_slice())
    }
    #[cfg(not(windows))]
    Err(not_here(address))
}

/// Sends a job to the USB printer at `address` and waits until it has printed it; an error it
/// reports is the result.
pub fn send_job(address: &str, job: &[u8]) -> Result<(), PrinterError> {
    #[cfg(windows)]
    {
        // A long label in high resolution takes a while.
        let last = open(address)?.print(job, std::time::Duration::from_secs(60))?;
        let status = Status::try_from(last.as_slice())?;
        if last[18] == device::STATUS_ERROR || status.errors != 0 {
            return Err(PrinterError::Io(std::io::Error::other(format!(
                "the printer reports {}",
                status.error_names().join(", ")
            ))));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = job;
        Err(not_here(address))
    }
}

/// Stops a job the USB printer at `address` still waits on (it shows "printing" and takes no
/// new job).
pub fn abort(address: &str) -> Result<(), PrinterError> {
    #[cfg(windows)]
    {
        open(address)?.abort();
        Ok(())
    }
    #[cfg(not(windows))]
    Err(not_here(address))
}

/// Whether the printer at USB `address` has the serial `serial` (as the network gives it:
/// USB pads it with zeros, `000H6Z560817` is `H6Z560817`).
#[must_use]
pub fn has_serial(address: &str, serial: &str) -> bool {
    let usb = address.strip_prefix(PREFIX).unwrap_or(address);
    let bare = |serial: &str| serial.trim().trim_start_matches('0').to_ascii_uppercase();
    !bare(serial).is_empty() && bare(usb) == bare(serial)
}

#[cfg(windows)]
fn open(address: &str) -> Result<device::Connection, PrinterError> {
    let serial = address.strip_prefix(PREFIX).unwrap_or(address);
    let printer = device::find()
        .into_iter()
        .find(|printer| printer.serial.eq_ignore_ascii_case(serial))
        .ok_or_else(|| not_here(address))?;
    Ok(device::Connection::open(&printer.path)?)
}

fn not_here(address: &str) -> PrinterError {
    PrinterError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("{address} is not plugged in"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_addresses() {
        assert!(is_usb("usb:000H6Z560817"));
        assert!(!is_usb("192.168.1.57"));
    }

    #[test]
    fn a_network_address_is_never_on_usb() {
        assert!(!is_present("192.168.1.57"));
    }

    #[test]
    fn the_network_serial_has_no_leading_zeros() {
        assert!(has_serial("usb:000H6Z560817", "H6Z560817"));
        assert!(has_serial("usb:000H6Z560817", "h6z560817 "));
        assert!(!has_serial("usb:000H6Z560817", "H6Z560818"));
        assert!(!has_serial("usb:000", ""));
    }
}
