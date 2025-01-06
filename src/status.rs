use std::fmt::Debug;

use crate::{prelude::PTouchTcpInterface, printer::PTouchPrinter, PTouchError, Result};
use bitflags::bitflags;
use num_enum::{FromPrimitive, IntoPrimitive};

pub trait GetStatus {
    fn get_status(&mut self) -> Result<Status>;
}

impl GetStatus for PTouchPrinter<PTouchTcpInterface> {
    fn get_status(&mut self) -> Result<Status> {
        self.get_snmp_status()
    }
}

/// Device status message
#[derive(Clone, PartialEq, Debug)]
pub struct Status {
    pub model: Model,
    pub error_status: ErrorStatus,
    pub status_type: DeviceStatus,

    pub media_width: u8,
    pub media_length: u8,
    pub media_kind: MediaKind,
    // pub phase: Phase,
    pub tape_colour: TapeColour,
    pub text_colour: TextColour,
}

impl TryFrom<&[u8]> for Status {
    type Error = PTouchError;

    fn try_from(value: &[u8]) -> Result<Self> {
        let ary: [u8; 32] = value
            .try_into()
            .map_err(|_| PTouchError::InvalidStatusPayload)?;
        Ok(ary.into())
    }
}

impl From<[u8; 32]> for Status {
    fn from(r: [u8; 32]) -> Self {
        Self {
            model: Model::from(r[4]),
            error_status: ErrorStatus::from_bits_truncate(u16::from_le_bytes([r[8], r[9]])),

            media_width: r[10],
            media_length: r[17],
            media_kind: MediaKind::from(r[11]),
            status_type: DeviceStatus::from(r[18]),
            // phase: Phase::from(r[20]),
            tape_colour: TapeColour::from(r[24]),
            text_colour: TextColour::from(r[25]),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum Model {
    // Standard 300(600)DPI desk printers
    QL710W = 0x36,
    QL720NW = 0x37,
    QL800 = 0x38,
    QL810W = 0x39,
    QL820NWB = 0x41,
    QL600 = 0x47,

    // Here for future support/reference only (180DPI tape printers)
    PTH500 = 0x64,
    PTE500 = 0x65,
    PTP700 = 0x67,

    #[num_enum(catch_all)]
    Unknown(u8),
}

impl Model {
    pub fn dpi(&self) -> u16 {
        match self {
            Self::QL710W
            | Self::QL720NW
            | Self::QL800
            | Self::QL810W
            | Self::QL820NWB
            | Self::QL600 => 300,
            Self::PTH500 | Self::PTE500 | Self::PTP700 => 180,
            Self::Unknown(_) => 0,
        }
    }

    /// Does the printer support double vertical resolution (i.e. half speed)
    pub fn support_double_dpi(&self) -> bool {
        match self {
            Self::QL710W
            | Self::QL720NW
            | Self::QL800
            | Self::QL810W
            | Self::QL820NWB
            | Self::QL600 => true,
            Self::PTH500 | Self::PTE500 | Self::PTP700 => false,
            Self::Unknown(_) => false,
        }
    }
}

bitflags! {
    #[derive(Copy, Clone, PartialEq )]
    pub struct ErrorStatus: u16 {
        const NO_MEDIA = 0x0001;
        const END_OF_MEDIA = 0x0002;
        const CUTTER_JAM = 0x0004;
        const WEAK_BATT = 0x0008;
        const IN_USE = 0x0010;
        const PRINTER_OFF = 0x0020;
        const HIGH_VOLT = 0x0040;
        const FAN_MOTOR_ERROR = 0x0080;

        const WRONG_MEDIA = 0x0100;
        const EXP_BUFFER_FULL = 0x0200;
        const COMMS_ERROR = 0x0400;
        const COMMS_BUFFFER_FULL = 0x0800;
        const COVER_OPEN = 0x1000;
        const OVERHEAT = 0x2000;
        const MEDIA_END = 0x4000;
        const SYSTEM_ERROR = 0x8000;
    }
}

impl Debug for ErrorStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set()
            .entries(self.iter_names().map(|(s, _v)| s))
            .finish()
    }
}

/// Kind of media loaded in printer
#[derive(Copy, Clone, PartialEq, Debug, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum MediaKind {
    None = 0x00,
    LaminatedTape = 0x01,
    NonLaminatedTape = 0x03,
    #[num_enum(alternatives = [0x17])] // HeatSrink 3:1 (higher shrink ratio(?))
    HeatShrinkTube = 0x11,
    FlexibleTape = 0x14,
    #[num_enum(alternatives = [0x0a])] // At least for my QL810W printer....
    ContinuousLengthTape = 0x4a, // Used for both paper and film.
    DieCutLabels = 0x4b, // Used for both paper and film.

    #[num_enum(catch_all)]
    IncompatibleTape(u8),
}

#[derive(Copy, Clone, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum DeviceStatus {
    Reply = 0x00,
    Completed = 0x01,
    Error = 0x02,
    ExitIF = 0x03,
    TurnedOff = 0x04,
    Notification = 0x05,
    PhaseChange = 0x06,

    #[num_enum(catch_all)]
    Unknown(u8),
}

/// Tape colour enumerations
#[derive(Copy, Clone, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]

pub enum TapeColour {
    White = 0x01,
    Other = 0x02,
    ClearBlack = 0x03,
    Red = 0x04,
    Blue = 0x05,
    Black = 0x08,
    ClearWhite = 0x09,
    MatteWhite = 0x20,
    MatteClear = 0x21,
    MatteSilver = 0x22,
    SatinGold = 0x23,
    SatinSilver = 0x24,
    BlueD = 0x30,
    RedD = 0x31,
    FluroOrange = 0x40,
    FluroYellow = 0x41,
    BerryPinkS = 0x50,
    LightGrayS = 0x51,
    LimeGreenS = 0x52,
    YellowF = 0x60,
    PinkF = 0x61,
    BlueF = 0x62,
    WhiteHst = 0x70,
    WhiteFlexId = 0x90,
    YellowFlexId = 0x91,
    Cleaning = 0xF0,
    Stencil = 0xF1,

    #[num_enum(default)]
    Unknown = 0,
}

/// Text colour enumerations
#[derive(Copy, Clone, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum TextColour {
    White = 0x01,
    Red = 0x04,
    Blue = 0x05,
    Black = 0x08,
    Gold = 0x0A,
    BlueF = 0x62,
    Cleaning = 0xf0,
    Stencil = 0xF1,
    Other = 0x02,

    #[num_enum(default)]
    Unknown = 0,
}

#[test]
fn parse_status() {
    // 80204234393004000000320a00001b0000000000000000000001000000000000
    let _status_example = [
        128, 32, 66, 52, 57, 48, 4, 0, 0, 0, 50, 10, 0, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0,
        0, 0, 0, 0, 0,
    ];
}
