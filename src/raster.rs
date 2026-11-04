use std::ops::{Deref, DerefMut};

/// Rasterbuffer containing rasterlines, currently only for the 720px wide printers.
pub struct RasterBuffer(Vec<RasterLine>);
pub type RasterLine = [u8; RasterBuffer::BYTES_PER_LINE];

impl RasterBuffer {
    pub const WIDTH: u32 = 720;
    pub const BYTES_PER_LINE: usize = Self::WIDTH as usize / 8;

    pub fn new(height: u32) -> Self {
        let mut pbuf = Vec::with_capacity(height as usize);

        for _line in 0..height {
            pbuf.push([0; Self::BYTES_PER_LINE]);
        }

        RasterBuffer(pbuf)
    }
}

impl DerefMut for RasterBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for RasterBuffer {
    type Target = Vec<RasterLine>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
