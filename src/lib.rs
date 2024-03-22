use std::io;

pub mod interface;
pub mod printer;
pub mod commands;
pub mod status;

pub mod prelude {
    pub use super::interface::{PTouchInterface, PTouchTcpInterface};
    pub use super::printer;
    pub use super::status::Status;
    pub use super::Result;
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
