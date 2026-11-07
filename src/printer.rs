//! A printer by its address, on the network or USB alike.
//!
//! How to use: `Printer::new("192.168.1.57")` or `Printer::new("usb:000H6Z560817")`, then
//! [`Printer::status`] and [`Printer::send_job`] (a job from [`crate::job`]). The address is
//! what a user types or a search ([`crate::net::discover`], [`crate::usb::find`]) gives.
//!
//! Working principle: the address says the way: `usb:` and a serial is a USB printer
//! ([`crate::usb`]), anything else a host on the network ([`crate::net`]: TCP 9100, status over
//! SNMP). Nothing is held open between calls, so a printer that is switched off or unplugged in
//! between costs only that call.

use crate::{PrinterError, Status, usb};

/// A Brother QL printer by its address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Printer {
    address: String,
}

impl Printer {
    /// The printer at `address`: a host name or IP address, or `usb:<serial>`.
    #[must_use]
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into().trim().to_owned(),
        }
    }

    /// Its address as given.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Whether it's on USB.
    #[must_use]
    pub fn is_usb(&self) -> bool {
        usb::is_usb(&self.address)
    }

    /// Its status: model, errors, loaded media.
    ///
    /// # Errors
    ///
    /// The printer not answering (off, unplugged, no SNMP), or an invalid answer.
    pub fn status(&self) -> Result<Status, PrinterError> {
        if self.is_usb() {
            usb::query_status(&self.address)
        } else {
            #[cfg(feature = "net")]
            return crate::net::query_status(&self.address);
            #[cfg(not(feature = "net"))]
            return Err(self.no_network());
        }
    }

    /// Sends a job. Over USB it returns once the printer has printed it (an error it reports
    /// is the result); over the network once the printer has taken the bytes.
    ///
    /// # Errors
    ///
    /// The printer not reachable, or (USB) reporting an error.
    pub fn send_job(&self, job: &[u8]) -> Result<(), PrinterError> {
        if self.is_usb() {
            usb::send_job(&self.address, job)
        } else {
            #[cfg(feature = "net")]
            return crate::net::send_job(&self.address, job);
            #[cfg(not(feature = "net"))]
            return Err(self.no_network());
        }
    }

    /// Stops a job the printer still waits on. Only USB printers can be told so; on the
    /// network this does nothing (switch the printer off and on).
    ///
    /// # Errors
    ///
    /// The USB printer not plugged in.
    pub fn abort(&self) -> Result<(), PrinterError> {
        if self.is_usb() {
            usb::abort(&self.address)
        } else {
            Ok(())
        }
    }

    #[cfg(not(feature = "net"))]
    fn no_network(&self) -> PrinterError {
        PrinterError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            format!("{}: network printing is off (feature `net`)", self.address),
        ))
    }
}

impl std::fmt::Display for Printer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_says_the_way() {
        assert!(Printer::new("usb:000H6Z560817").is_usb());
        assert!(!Printer::new("192.168.1.57").is_usb());
        assert_eq!(Printer::new(" labelprinter \n").address(), "labelprinter");
    }

    #[test]
    fn abort_on_the_network_does_nothing() {
        assert!(Printer::new("192.0.2.1").abort().is_ok());
    }
}
