use std::io;

pub mod commands;
pub mod interface;
pub mod printer;
pub mod status;
pub mod raster;

pub mod prelude {
    pub use super::interface::{PTouchInterface, PTouchTcpInterface};
    pub use super::printer::{self, PTouchPrinter};

    pub use super::commands::{Commands, PrintInfo, VariousMode};
    pub use super::status::{GetStatus, Status};
    pub use super::Result;
    pub use super::raster::RasterBuffer;
}

pub type Result<T> = std::result::Result<T, PTouchError>;

#[derive(Debug)]
pub enum PTouchError {
    IoError(io::Error),
    InvalidStatusPayload,
    SNMPError,
}

impl From<io::Error> for PTouchError {
    fn from(io_error: io::Error) -> Self {
        PTouchError::IoError(io_error)
    }
}
