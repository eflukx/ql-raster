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

/// A raster line packed for TIFF compression mode (`M 02`, Raster Command Reference "Select
/// compression mode"): PackBits. A run of 2 to 128 equal bytes is (count − 1) as a negative
/// number and the byte; 1 to 128 different bytes are (count − 1) and the bytes. Zeros at the end
/// are packed too, not left out. When that is longer than the line, the line goes as one
/// literal: 91 bytes for a 90-byte line. The `g` command then gets the packed length.
#[must_use]
pub fn pack_bits(line: &[u8]) -> Vec<u8> {
    let mut packed = Vec::with_capacity(line.len() + 1);
    let mut i = 0;
    while i < line.len() {
        let mut run = 1;
        while i + run < line.len() && run < 128 && line[i + run] == line[i] {
            run += 1;
        }
        if run >= 2 {
            packed.push((1 - run as i16) as i8 as u8);
            packed.push(line[i]);
            i += run;
            continue;
        }
        // Different bytes, up to where two equal ones start.
        let start = i;
        while i < line.len() && i - start < 128 {
            if i + 1 < line.len() && line[i] == line[i + 1] {
                break;
            }
            i += 1;
        }
        packed.push((i - start - 1) as u8);
        packed.extend_from_slice(&line[start..i]);
    }
    if packed.len() > line.len() {
        let mut literal = Vec::with_capacity(line.len() + 1);
        literal.push((line.len() - 1) as u8);
        literal.extend_from_slice(line);
        return literal;
    }
    packed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unpacks PackBits, as the printer does.
    fn unpack(mut packed: &[u8]) -> Vec<u8> {
        let mut line = Vec::new();
        while let Some((&count, rest)) = packed.split_first() {
            let count = count as i8;
            if count >= 0 {
                let n = count as usize + 1;
                line.extend_from_slice(&rest[..n]);
                packed = &rest[n..];
            } else {
                line.extend(std::iter::repeat_n(
                    rest[0],
                    (1 - i16::from(count)) as usize,
                ));
                packed = &rest[1..];
            }
        }
        line
    }

    #[test]
    fn the_reference_example() {
        // 20 × 00, 2 × 22, then six different bytes: ED 00 FF 22 05 23 BA BF A2 22 2B.
        let mut line = vec![0x00; 20];
        line.extend_from_slice(&[0x22, 0x22, 0x23, 0xba, 0xbf, 0xa2, 0x22, 0x2b]);
        line.resize(90, 0x00);
        let packed = pack_bits(&line);
        assert_eq!(
            &packed[..11],
            &[0xed, 0x00, 0xff, 0x22, 0x05, 0x23, 0xba, 0xbf, 0xa2, 0x22, 0x2b]
        );
        // The 62 zeros at the end are packed, not left out.
        assert_eq!(&packed[11..], &[0xc3, 0x00]);
        assert_eq!(unpack(&packed), line);
    }

    #[test]
    fn lines_unpack_to_themselves() {
        let blank = [0_u8; 90];
        // One run of 90 zeros: 1 − 90 = −89 = A7h.
        assert_eq!(pack_bits(&blank), vec![0xa7, 0x00]);
        assert_eq!(unpack(&pack_bits(&blank)), blank);

        // No two equal bytes next to each other: one literal, 91 bytes.
        let busy: Vec<u8> = (0..90).map(|i| (i * 7 + i / 3) as u8).collect();
        let packed = pack_bits(&busy);
        assert!(packed.len() <= 91);
        assert_eq!(unpack(&packed), busy);

        // Runs and different bytes mixed.
        let mut mixed = vec![0xff; 40];
        mixed.extend((0..30).map(|i| i as u8));
        mixed.extend([0x0f; 20]);
        assert_eq!(unpack(&pack_bits(&mixed)), mixed);
    }
}
