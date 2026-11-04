use std::{error::Error, fmt::Display, io};

pub mod commands;
pub mod interface;
pub mod printer;
pub mod raster;
pub mod status;

pub mod prelude {
    pub use super::interface::{PTouchInterface, PTouchTcpInterface};
    pub use super::printer::{self, PTouchPrinter};

    pub use super::commands::{Commands, PrintInfo, VariousMode};
    pub use super::raster::RasterBuffer;
    pub use super::status::{GetStatus, Status};
    pub use super::PTResult;
}

pub type PTResult<T> = std::result::Result<T, PTouchError>;

#[derive(Debug)]
pub enum PTouchError {
    IoError(io::Error),
    InvalidStatusPayload,
    SNMPError,
}

impl Error for PTouchError {}

impl Display for PTouchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<io::Error> for PTouchError {
    fn from(io_error: io::Error) -> Self {
        PTouchError::IoError(io_error)
    }
}
