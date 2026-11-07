//! The command stream of one print job (Raster Command Reference, "Print sequence").
//!
//! Sequence for one label: invalidate, initialise, raster mode, print information (media type,
//! width and length, line count), auto cut, cut every label, cut at the end, feed margin (35
//! dots on continuous tape, 0 on die-cut labels), no compression, the raster lines (all as
//! `g`: `Z` is for TIFF mode only), print with feeding. The medium comes from the ql-raster media table.
//!
//! Several labels are pages of one job: from the print information on, each page is sent (the
//! information marks the first page, the others as later pages), every page but the last ends
//! with "print" (0Ch), the last with "print with feeding" (1Ah). [`Cut`] says where the cutter
//! cuts: after each label, once after the last, or not at all. [`Resolution::High`] sets the
//! expanded mode's high-resolution bit: 600 dpi along the tape, twice the raster lines
//! ([`crate::raster::raster_lines_at`]); the feed margin stays in 300-dpi dots, as in brother_ql.
//! High resolution also asks for quality first (print information, n1 bit 6). [`Resolution`],
//! the mode bits and the print information come from ql-raster.

pub use crate::commands::CompressionMode as Compression;
use crate::commands::{AdvancedMode, PrintInfo};
use crate::media::Media;
pub use crate::media::Resolution;

use crate::raster::{BYTES_PER_LINE, RasterLine};

/// Where the cutter cuts in a job.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Cut {
    /// After every label.
    #[default]
    Each,
    /// Once, after the last label: the batch stays one strip.
    End,
    /// Not at all: tear off by hand.
    Off,
}

impl Cut {
    /// Various mode (auto cut bit), cut every n labels, expanded mode (cut at end bit).
    fn commands(self, pages: usize) -> [[u8; 4]; 3] {
        let every = u8::try_from(pages.clamp(1, 255)).unwrap_or(255);
        let (auto, every, at_end) = match self {
            Self::Each => (0x40, 1, 0x08),
            Self::End => (0x40, every, 0x08),
            Self::Off => (0x00, 1, 0x00),
        };
        [
            [0x1b, 0x69, 0x4d, auto],
            [0x1b, 0x69, 0x41, every],
            [0x1b, 0x69, 0x4b, at_end],
        ]
    }
}

/// Builds the bytes of a one-label job on `media`, cut after it.
#[must_use]
pub fn job_bytes(lines: &[RasterLine], media: &Media) -> Vec<u8> {
    pages_bytes(
        &[lines],
        media,
        Cut::Each,
        Resolution::Normal,
        Compression::None,
    )
}

/// Builds the bytes of a job printing `pages` (labels, each its raster lines) on `media`,
/// cut as `cut` says, the lines at `resolution`, packed as `compression` says (TIFF only for
/// models that take it: [`crate::Model::compression`]).
#[must_use]
pub fn pages_bytes(
    pages: &[&[RasterLine]],
    media: &Media,
    cut: Cut,
    resolution: Resolution,
    compression: Compression,
) -> Vec<u8> {
    let lines: usize = pages.iter().map(|page| page.len()).sum();
    let mut bytes = Vec::with_capacity(lines * (BYTES_PER_LINE + 3) + 64 * pages.len() + 512);

    // Invalidate (clears a half-received job), initialise, switch to raster mode.
    bytes.extend_from_slice(&[0x00; 400]);
    bytes.extend_from_slice(&[0x1b, 0x40]);
    bytes.extend_from_slice(&[0x1b, 0x69, 0x61, 0x01]);

    let mut commands = cut.commands(pages.len());
    if resolution == Resolution::High {
        commands[2][3] |= AdvancedMode::HIGH_RES.bits();
    }
    for (index, lines) in pages.iter().enumerate() {
        page_bytes(
            &mut bytes,
            lines,
            media,
            index == 0,
            commands,
            resolution,
            compression,
        );
        // Print; the last page with feeding.
        bytes.push(if index + 1 == pages.len() { 0x1a } else { 0x0c });
    }
    bytes
}

/// One page: print information, modes, margin and the raster lines.
fn page_bytes(
    bytes: &mut Vec<u8>,
    lines: &[RasterLine],
    media: &Media,
    first: bool,
    cut: [[u8; 4]; 3],
    resolution: Resolution,
    compression: Compression,
) {
    // Print information (ql-raster): media kind, width and length, recover on, quality first
    // in high resolution; raster line count; first or later page.
    let line_count = u32::try_from(lines.len()).expect("a label has far fewer lines than u32::MAX");
    let info = PrintInfo {
        quality: resolution == Resolution::High,
        later_page: !first,
        ..media.print_info(line_count)
    };
    bytes.extend_from_slice(&info.to_bytes());

    // Various mode (auto cut), cut every n labels, expanded mode (cut at the end).
    for command in cut {
        bytes.extend_from_slice(&command);
    }

    // Feed margin, no compression.
    let feed = media.feed_dots();
    bytes.extend_from_slice(&[0x1b, 0x69, 0x64]);
    bytes.extend_from_slice(&feed.to_le_bytes());
    bytes.extend_from_slice(&[0x4d, compression as u8]);

    for line in lines {
        match compression {
            // TIFF: a blank line is "zero raster graphics" (5Ah), the others packed.
            Compression::Tiff if line.iter().all(|&byte| byte == 0) => bytes.push(0x5a),
            Compression::Tiff => {
                let packed = crate::raster::pack_bits(line);
                bytes.extend_from_slice(&[0x67, 0x00, packed.len() as u8]);
                bytes.extend_from_slice(&packed);
            }
            // Every line as `g`, blank ones too: 5Ah is valid only with TIFF. Over the network
            // the QL-710W takes it anyway; over USB it prints while it receives (concurrent
            // printing) and stops at the first 5Ah.
            Compression::None => {
                bytes.extend_from_slice(&[0x67, 0x00, BYTES_PER_LINE as u8]);
                bytes.extend_from_slice(line);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_has_header_lines_and_print_command() {
        let mut busy = [0_u8; BYTES_PER_LINE];
        busy[10] = 0xff;
        let lines = [busy, [0; BYTES_PER_LINE], busy];

        let bytes = job_bytes(&lines, Media::by_id("62").expect("known"));

        assert!(bytes[..400].iter().all(|&byte| byte == 0));
        assert_eq!(&bytes[400..406], &[0x1b, 0x40, 0x1b, 0x69, 0x61, 0x01]);

        // Print information: continuous (0Ah), 62 mm, 3 raster lines.
        assert_eq!(&bytes[406..413], &[0x1b, 0x69, 0x7a, 0x8e, 0x0a, 62, 0]);
        assert_eq!(&bytes[413..417], &3_u32.to_le_bytes());

        // Feed 35 dots.
        let feed = bytes
            .windows(5)
            .position(|window| window[..3] == [0x1b, 0x69, 0x64])
            .expect("margin command");
        assert_eq!(&bytes[feed + 3..feed + 5], &[35, 0]);

        // After "no compression": three full lines, the blank one too (no 5Ah without TIFF),
        // print with feeding.
        let start = feed + 5 + 2;
        let line_command = [0x67, 0x00, 90];
        for line in 0..3 {
            assert_eq!(
                &bytes[start + line * 93..start + line * 93 + 3],
                &line_command
            );
        }
        assert!(bytes[start + 96..start + 186].iter().all(|&byte| byte == 0));
        assert_eq!(bytes.last(), Some(&0x1a));
        assert_eq!(
            bytes.len(),
            400 + 2 + 4 + 13 + 4 + 4 + 4 + 5 + 2 + 3 * (3 + 90) + 1
        );
    }

    #[test]
    fn labels_are_pages_of_one_job() {
        let media = Media::by_id("62").expect("known");
        let lines = [[0_u8; BYTES_PER_LINE]];
        let one = job_bytes(&lines, media);
        let three = pages_bytes(
            &[&lines, &lines, &lines],
            media,
            Cut::Each,
            Resolution::Normal,
            Compression::None,
        );
        let page = one.len() - 406;
        assert_eq!(three.len(), 406 + 3 * page);

        let infos: Vec<usize> = three
            .windows(3)
            .enumerate()
            .filter(|(_, window)| *window == [0x1b, 0x69, 0x7a])
            .map(|(at, _)| at)
            .collect();
        assert_eq!(infos.len(), 3);
        // First page marked 0, later pages 1.
        assert_eq!(
            infos.iter().map(|&at| three[at + 11]).collect::<Vec<_>>(),
            [0, 1, 1]
        );
        // Print after each page, with feeding after the last.
        assert_eq!(three[infos[1] - 1], 0x0c);
        assert_eq!(three[infos[2] - 1], 0x0c);
        assert_eq!(three.last(), Some(&0x1a));
    }

    #[test]
    fn cut_settings() {
        let media = Media::by_id("62").expect("known");
        let lines = [[0_u8; BYTES_PER_LINE]];
        let modes = |cut: Cut| {
            let bytes = pages_bytes(
                &[&lines, &lines, &lines],
                media,
                cut,
                Resolution::Normal,
                Compression::None,
            );
            let at = bytes
                .windows(3)
                .position(|window| window == [0x1b, 0x69, 0x4d])
                .expect("various mode");
            (bytes[at + 3], bytes[at + 7], bytes[at + 11])
        };
        assert_eq!(modes(Cut::Each), (0x40, 1, 0x08));
        assert_eq!(modes(Cut::End), (0x40, 3, 0x08));
        assert_eq!(modes(Cut::Off), (0x00, 1, 0x00));
    }

    #[test]
    fn tiff_packs_lines_and_sends_blank_ones_as_zero_raster() {
        let media = Media::by_id("62").expect("known");
        let mut busy = [0_u8; BYTES_PER_LINE];
        busy[10] = 0xff;
        let lines = [busy, [0; BYTES_PER_LINE], busy];
        let bytes = pages_bytes(
            &[&lines],
            media,
            Cut::Each,
            Resolution::Normal,
            Compression::Tiff,
        );
        let at = bytes
            .windows(2)
            .rposition(|window| window == [0x4d, 0x02])
            .expect("TIFF mode")
            + 2;
        // 10 zeros, FF, 79 zeros: F7 00, 00 FF, B2 00.
        let packed = [0xf7, 0x00, 0x00, 0xff, 0xb2, 0x00];
        let line = [&[0x67, 0x00, 6][..], &packed].concat();
        let expected = [&line[..], &[0x5a], &line, &[0x1a]].concat();
        assert_eq!(&bytes[at..], &expected[..]);
    }

    #[test]
    fn high_resolution_sets_the_expanded_mode_bit() {
        let media = Media::by_id("62").expect("known");
        let lines = [[0_u8; BYTES_PER_LINE]];
        let expanded = |cut: Cut| {
            let bytes = pages_bytes(&[&lines], media, cut, Resolution::High, Compression::None);
            let at = bytes
                .windows(3)
                .position(|window| window == [0x1b, 0x69, 0x4b])
                .expect("expanded mode");
            bytes[at + 3]
        };
        assert_eq!(expanded(Cut::Each), 0x48);
        assert_eq!(expanded(Cut::Off), 0x40);

        // Quality first in the print information (n1 bit 6), not in normal resolution.
        let n1 = |resolution| {
            pages_bytes(&[&lines], media, Cut::Each, resolution, Compression::None)[409]
        };
        assert_eq!(n1(Resolution::High) & 0x40, 0x40);
        assert_eq!(n1(Resolution::Normal) & 0x40, 0);
    }

    #[test]
    fn die_cut_jobs_carry_the_label_length_and_no_feed() {
        let bytes = job_bytes(
            &[[0; BYTES_PER_LINE]],
            Media::by_id("62x100").expect("known"),
        );

        // Print information: die-cut (0Bh), 62 × 100 mm.
        assert_eq!(&bytes[406..413], &[0x1b, 0x69, 0x7a, 0x8e, 0x0b, 62, 100]);

        let feed = bytes
            .windows(5)
            .position(|window| window[..3] == [0x1b, 0x69, 0x64])
            .expect("margin command");
        assert_eq!(&bytes[feed + 3..feed + 5], &[0, 0]);
    }
}
