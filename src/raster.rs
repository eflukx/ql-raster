//! Label bitmaps as raster lines for the 720-pin print head.
//!
//! One bitmap row is one raster line of 90 bytes. Where the label sits on the head depends on
//! the medium ([`Media`]: on 62 mm tape pins 12 to 707, on 29 mm tape pins 408 to 713); the
//! line is sent mirrored (the head's first byte is the label's right edge), as the ql-raster
//! fork does and the TLS configurator prints with.

use crate::label::Bitmap;
use crate::media::{Media, MediaError, Resolution};

/// Pins on the head.
pub const PINS: u32 = 720;

/// Bytes per raster line.
pub const BYTES_PER_LINE: usize = (PINS / 8) as usize;

/// One raster line.
pub type RasterLine = [u8; BYTES_PER_LINE];

/// Converts a label bitmap to raster lines for `media`, after checking that it fits (width,
/// and length: the die-cut size, or 12.7 mm to 1 m of continuous tape).
pub fn raster_lines(bitmap: &Bitmap, media: &Media) -> Result<Vec<RasterLine>, MediaError> {
    raster_lines_at(bitmap, media, Resolution::Normal)
}

/// As [`raster_lines`], for a bitmap at `resolution`: in high resolution two rows per 300-dpi
/// dot along the tape, so the size check takes half its height.
pub fn raster_lines_at(
    bitmap: &Bitmap,
    media: &Media,
    resolution: Resolution,
) -> Result<Vec<RasterLine>, MediaError> {
    media.check_raster(bitmap.width(), bitmap.height(), resolution)?;

    let mut lines = Vec::with_capacity(bitmap.height() as usize);

    for y in 0..bitmap.height() {
        let mut line = [0_u8; BYTES_PER_LINE];

        for x in 0..bitmap.width() {
            if !bitmap.get(x, y) {
                continue;
            }

            let Some(pin) = media.pin(x).map(u32::from) else {
                continue;
            };

            let byte = BYTES_PER_LINE - 1 - (pin / 8) as usize;
            line[byte] |= 1 << (pin % 8);
        }

        lines.push(line);
    }

    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_bits(line: &RasterLine) -> usize {
        line.iter().map(|byte| byte.count_ones() as usize).sum()
    }

    fn media(id: &str) -> &'static Media {
        Media::by_id(id).expect("known medium")
    }

    #[test]
    fn full_row_uses_the_print_area_only() {
        let mut bitmap = Bitmap::new(696, 150);
        bitmap.fill_rect(0, 0, 696, 1);

        let lines = raster_lines(&bitmap, media("62")).expect("fits");
        assert_eq!(lines.len(), 150);
        assert_eq!(set_bits(&lines[0]), 696);
        assert_eq!(set_bits(&lines[1]), 0);

        // 12 margin pins on either side stay clear: pin p sits in byte 89 - p/8, bit p%8.
        assert_eq!(lines[0][89] & 0x0f, 0, "pins 0-3 (left margin)");
        assert_eq!(lines[0][88], 0xf0, "pins 8-11 clear, 12-15 set");
        assert_eq!(lines[0][1], 0x0f, "pins 704-707 set, 708-711 clear");
        assert_eq!(lines[0][0], 0, "pins 712-719 (right margin)");
    }

    #[test]
    fn first_dot_is_pin_twelve() {
        let mut bitmap = Bitmap::new(696, 150);
        bitmap.set(0, 0, true);

        let line = raster_lines(&bitmap, media("62")).expect("fits")[0];
        assert_eq!(line[88], 1 << 4);
        assert_eq!(set_bits(&line), 1);
    }

    #[test]
    fn narrow_tape_uses_its_own_pins() {
        // 29 mm: pins 408 to 713. Pin 408 is byte 89 - 51 = 38, bit 0; pin 713 byte 0, bit 1.
        let mut bitmap = Bitmap::new(306, 150);
        bitmap.set(0, 0, true);
        bitmap.set(305, 0, true);

        let line = raster_lines(&bitmap, media("29")).expect("fits")[0];
        assert_eq!(line[38], 1);
        assert_eq!(line[0], 1 << 1);
        assert_eq!(set_bits(&line), 2);
    }

    #[test]
    fn refuses_labels_that_do_not_fit() {
        assert!(
            raster_lines(&Bitmap::new(696, 150), media("29")).is_err(),
            "too wide"
        );
        assert!(
            raster_lines(&Bitmap::new(306, 79), media("29")).is_err(),
            "too short: 79 lines and 2 × 35 feed is under 12.7 mm"
        );
        assert!(
            raster_lines(&Bitmap::new(306, 85), media("29")).is_ok(),
            "the PCB label"
        );
        assert!(
            raster_lines(&Bitmap::new(696, 300), media("62x29")).is_err(),
            "not die-cut size"
        );
        assert!(raster_lines(&Bitmap::new(696, 271), media("62x29")).is_ok());
    }

    #[test]
    fn high_resolution_takes_twice_the_lines() {
        let die_cut = media("62x100");
        let lines = u32::from(die_cut.print_length_dots);
        assert!(raster_lines_at(&Bitmap::new(696, 2 * lines), die_cut, Resolution::High).is_ok());
        assert!(raster_lines_at(&Bitmap::new(696, lines), die_cut, Resolution::High).is_err());
        // 150 rows are 75 dots: under the shortest label.
        assert!(raster_lines_at(&Bitmap::new(696, 150), media("62"), Resolution::High).is_err());
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
mod pack_tests {
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
            &[
                0xed, 0x00, 0xff, 0x22, 0x05, 0x23, 0xba, 0xbf, 0xa2, 0x22, 0x2b
            ]
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
