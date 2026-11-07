//! The USB device layer (Windows): Brother printers through Windows' own USB printer driver
//! (no Brother driver needed). [`super`] builds `usb:` addresses on top.
//!
//! How to use: [`find`] lists the Brother printers plugged in; [`Connection::open`]
//! opens one by its path. Then `write` a job (the same bytes as over TCP port 9100) or
//! [`Connection::status`] for the 32-byte status (the same as over SNMP);
//! [`Connection::print`] sends a job and waits for the printer to say how it went.
//!
//! Working principle: Windows binds "USB Printing Support" (usbprint.sys) to a printer's USB
//! printer-class interface by itself, also without the Brother driver (that one uses it too).
//! Its device interface (GUID 28D78FAD-…) is found with SetupAPI and opened as a file: writes
//! go to the bulk-out endpoint, reads come from bulk-in. A read returns at once, with nothing
//! when the printer has nothing to say, so the status is polled for a moment. A printer whose
//! USB device has another driver (WinUSB, libusb0 from Zadig) has no such interface and isn't
//! listed. The device is opened exclusively. Note: over USB the QL prints while it receives
//! (concurrent printing) and stops at a zero raster line (5Ah) without TIFF compression; send
//! blank lines as `g` lines. `QL_USB_TRACE` logs every read and write. Windows only.

// Windows' SetupAPI and file calls: the one place with FFI, so `unsafe` is allowed here only.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::time::{Duration, Instant};

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING, ReadFile, WriteFile,
};
use windows_sys::core::GUID;

/// GUID_DEVINTERFACE_USBPRINT: usbprint.sys's printer interfaces.
const USBPRINT: GUID = GUID::from_u128(0x28d7_8fad_5a12_11d1_ae5b_0000_f803_a8c2);

/// Brother's USB vendor id.
pub const BROTHER_VENDOR_ID: u16 = 0x04f9;

/// Status type (byte 18): printing completed.
pub const STATUS_COMPLETED: u8 = 0x01;
/// Status type (byte 18): an error occurred.
pub const STATUS_ERROR: u8 = 0x02;

/// Windows' "in use by another process".
const ERROR_SHARING_VIOLATION: i32 = 32;

/// How long [`Connection::open`] waits for the spooler to let go.
const OPEN_WAIT: Duration = Duration::from_secs(5);

/// How long [`Connection::status`] waits for the answer.
const STATUS_WAIT: Duration = Duration::from_secs(2);

/// A Brother printer on USB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbPrinter {
    /// The device interface path, to open it.
    pub path: String,
    /// USB product id (`2043`h: QL-710W).
    pub product_id: u16,
    /// Its serial number (`000H6Z560817`), from the path; empty when it has none.
    pub serial: String,
}

/// The Brother printers on USB with Windows' USB printer driver.
#[must_use]
pub fn find() -> Vec<UsbPrinter> {
    device_paths()
        .into_iter()
        .filter_map(|path| {
            let (vendor, product_id, serial) = parse_path(&path)?;
            (vendor == BROTHER_VENDOR_ID).then_some(UsbPrinter {
                path,
                product_id,
                serial,
            })
        })
        .collect()
}

/// Vendor id, product id and serial from a path like
/// `\\?\usb#vid_04f9&pid_2043#000h6z560817#{28d78fad-…}`.
fn parse_path(path: &str) -> Option<(u16, u16, String)> {
    let mut parts = path.split('#');
    let _prefix = parts.next()?;
    let ids = parts.next()?.to_ascii_lowercase();
    let serial = parts.next().unwrap_or_default().to_ascii_uppercase();
    let hex = |key: &str| {
        let at = ids.find(key)? + key.len();
        u16::from_str_radix(ids.get(at..at + 4)?, 16).ok()
    };
    // A composite device's interface (`…&mi_00`) has an instance id instead of the serial.
    let serial = if serial.contains('&') {
        String::new()
    } else {
        serial
    };
    Some((hex("vid_")?, hex("pid_")?, serial))
}

/// Every present usbprint interface's path.
fn device_paths() -> Vec<String> {
    let mut paths = Vec::new();
    // SAFETY: SetupAPI calls with buffers sized as it asks; the list is destroyed at the end.
    unsafe {
        let set = SetupDiGetClassDevsW(
            &USBPRINT,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if set == -1 {
            return paths;
        }
        for index in 0.. {
            let mut interface = SP_DEVICE_INTERFACE_DATA {
                cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                InterfaceClassGuid: GUID::from_u128(0),
                Flags: 0,
                Reserved: 0,
            };
            if SetupDiEnumDeviceInterfaces(set, std::ptr::null(), &USBPRINT, index, &mut interface)
                == 0
            {
                break;
            }
            let mut needed = 0_u32;
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &interface,
                std::ptr::null_mut(),
                0,
                &mut needed,
                std::ptr::null_mut(),
            );
            if needed == 0 {
                continue;
            }
            // u32-aligned: the struct starts with cbSize.
            let mut buffer = vec![0_u32; (needed as usize).div_ceil(4)];
            let detail = buffer
                .as_mut_ptr()
                .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(
                set,
                &interface,
                detail,
                needed,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == 0
            {
                continue;
            }
            let path = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
            let units = (needed as usize - 4) / 2;
            let text = std::slice::from_raw_parts(path, units);
            let end = text.iter().position(|&unit| unit == 0).unwrap_or(units);
            paths.push(String::from_utf16_lossy(&text[..end]));
        }
        SetupDiDestroyDeviceInfoList(set);
    }
    paths
}

/// An open Brother printer on USB.
#[derive(Debug)]
pub struct Connection {
    handle: HANDLE,
}

// The handle is only used through &mut self.
unsafe impl Send for Connection {}

impl Connection {
    /// Opens the printer at `path` (from [`find`]), for this process alone: Windows' print
    /// spooler talks to a printer with a driver too (bidirectional support: status requests),
    /// and its requests in the middle of a job break the job. While the spooler has it open,
    /// tries again for a few seconds.
    pub fn open(path: &str) -> io::Result<Self> {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let deadline = Instant::now() + OPEN_WAIT;
        loop {
            // SAFETY: a NUL-terminated path; the handle is closed on drop.
            let handle = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    std::ptr::null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE {
                trace(format_args!("opened {path}"));
                return Ok(Self { handle });
            }
            let error = io::Error::last_os_error();
            trace(format_args!("open: {error}"));
            if error.raw_os_error() != Some(ERROR_SHARING_VIOLATION) || Instant::now() > deadline {
                return Err(error);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// The printer's 32-byte status: invalidate, initialise, `ESC i S`, and the answer.
    pub fn status(&mut self) -> io::Result<[u8; 32]> {
        self.drain()?;
        let mut request = vec![0_u8; 200];
        request.extend_from_slice(&[0x1b, 0x40, 0x1b, 0x69, 0x53]);
        self.write(&request)?;
        self.next_status(Instant::now() + STATUS_WAIT)
    }

    /// Sends a print job and reads the printer's messages until it reports "printing
    /// completed" or an error (status type, byte 18: 01h, 02h); that last status. On USB the
    /// printer tells how a job went, and expects them to be read.
    pub fn print(&mut self, job: &[u8], wait: Duration) -> io::Result<[u8; 32]> {
        self.drain()?;
        self.write(job)?;
        let deadline = Instant::now() + wait;
        let outcome = loop {
            match self.next_status(deadline) {
                Ok(status) if matches!(status[18], STATUS_COMPLETED | STATUS_ERROR) => {
                    break Ok(status);
                }
                Ok(_) => {}
                Err(error) => break Err(error),
            }
        };
        if outcome.is_err() {
            self.abort();
        }
        outcome
    }

    /// Stops a job the printer still waits on: invalidate (as many zeros as a raster line
    /// and more) and initialise, as the reference says for a transmission stopped midway.
    pub fn abort(&mut self) {
        trace(format_args!("abort"));
        let mut stop = vec![0_u8; 400];
        stop.extend_from_slice(&[0x1b, 0x40]);
        let _ = self.write(&stop);
    }

    /// Reads away what the printer said before (a job's messages).
    fn drain(&mut self) -> io::Result<()> {
        let mut stale = [0_u8; 64];
        while self.read(&mut stale)? > 0 {}
        Ok(())
    }

    /// The next 32-byte status block, waiting until `deadline`.
    fn next_status(&mut self, deadline: Instant) -> io::Result<[u8; 32]> {
        let mut status = [0_u8; 32];
        let mut got = 0;
        while got < status.len() {
            let read = self.read(&mut status[got..])?;
            got += read;
            if read == 0 {
                if Instant::now() > deadline {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "no status"));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        Ok(status)
    }
}

impl Connection {
    /// Reads what the printer sent; 0 bytes when it has nothing to say.
    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut read = 0_u32;
        let length = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        // SAFETY: buf is valid for `length` bytes.
        let ok = unsafe {
            ReadFile(
                self.handle,
                buf.as_mut_ptr(),
                length,
                &mut read,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let error = io::Error::last_os_error();
            trace(format_args!("read error {error}"));
            return Err(error);
        }
        if read > 0 {
            trace(format_args!("read {read}: {:02x?}", &buf[..read as usize]));
        }
        Ok(read as usize)
    }

    /// Reads everything the printer has sent so far.
    pub fn read_vec(&mut self) -> io::Result<Vec<u8>> {
        let mut all = Vec::new();
        let mut buf = [0_u8; 64];
        loop {
            let read = self.read(&mut buf)?;
            if read == 0 {
                return Ok(all);
            }
            all.extend_from_slice(&buf[..read]);
        }
    }

    /// Writes `data` to the printer (a job, a status request).
    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        let mut rest = data;
        while !rest.is_empty() {
            let mut written = 0_u32;
            // In pieces of 1 KB: progress shows in the trace.
            let length = rest.len().min(1024) as u32;
            // SAFETY: rest is valid for `length` bytes.
            let ok = unsafe {
                WriteFile(
                    self.handle,
                    rest.as_ptr(),
                    length,
                    &mut written,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                let error = io::Error::last_os_error();
                trace(format_args!("write {length} error {error}"));
                return Err(error);
            }
            trace(format_args!("write {length}: {written} written"));
            rest = &rest[written as usize..];
        }
        Ok(())
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // SAFETY: our own open handle.
        unsafe {
            CloseHandle(self.handle.cast::<c_void>());
        }
    }
}

/// With `QL_USB_TRACE` set: every read and write to stderr, with the time since the first.
fn trace(what: std::fmt::Arguments<'_>) {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    if std::env::var_os("QL_USB_TRACE").is_some() {
        let start = START.get_or_init(Instant::now);
        eprintln!("{:>8.3} s  {what}", start.elapsed().as_secs_f32());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_give_the_ids_and_serial() {
        let path = r"\\?\usb#vid_04f9&pid_2043#000h6z560817#{28d78fad-5a12-11d1-ae5b-0000f803a8c2}";
        assert_eq!(
            parse_path(path),
            Some((0x04f9, 0x2043, "000H6Z560817".to_owned()))
        );
        let composite = r"\\?\usb#vid_04f9&pid_209b&mi_00#6&1a2b3c&0&0000#{28d78fad-5a12-11d1-ae5b-0000f803a8c2}";
        assert_eq!(parse_path(composite), Some((0x04f9, 0x209b, String::new())));
    }
}
