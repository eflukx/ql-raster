use std::ops::{Deref, DerefMut};

/// Rasterbuffer containing rasterlines, currently only for the 720px wide printers.
pub struct RasterBuffer(Vec<[u8; 720 / 8]>);

impl RasterBuffer {
    pub fn new(height: u32) -> Self {
        let mut pbuf = Vec::with_capacity(height as usize);

        for _line in 0..height {
            pbuf.push([0; 90]);
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
    type Target = Vec<[u8; 720 / 8]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
