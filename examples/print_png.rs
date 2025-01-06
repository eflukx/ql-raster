use image::{DynamicImage, ImageReader};
use ql_raster::prelude::*;

fn main() {
    let mut printer =
        printer::from_addr("labelprinter_3:9100").expect("Unable to connect to printer!");

    let status = printer.get_status();
    eprintln!("Status {:?}", status);
    let name = printer.get_snmp_name();
    eprintln!("name {:?}", name);
    let model = printer.get_snmp_model();
    eprintln!("model {:?}", model);

    let image = ImageReader::open("label.png").unwrap().decode().unwrap();
    let raster_data = rasterize_image(image);
    print_raster_data(&mut printer, raster_data).expect("Printing miserably failed!");

    let status = printer.get_snmp_status();
    eprintln!("Status {:?}", status);
}

/// We assume a fixed label width...
fn print_raster_data(
    printer: &mut PTouchPrinter<PTouchTcpInterface>,
    raster_data: RasterBuffer,
) -> Result<()> {
    printer.invalidate()?;
    printer.init()?;
    printer.switch_mode(ql_raster::commands::Mode::Raster)?;

    let pi = PrintInfo {
        kind: Some(ql_raster::status::MediaKind::ContinuousLengthTape),
        width: Some(29), // needs to be correct value for installed label type!
        length: None,
        raster_no: 1,
        recover: true,
    };
    printer.set_print_info(&pi)?;

    printer.set_various_mode(VariousMode::AUTO_CUT)?;
    // printer.set_advanced_mode(AdvancedMode::HIGH_RES)?;
    printer.set_page_no(1)?;

    printer.set_margin(0)?;
    printer.set_compression_mode(ql_raster::commands::CompressionMode::None)?;

    for line_data in raster_data.iter() {
        printer.transfer_raster_line(line_data)?;
    }

    printer.print_and_feed()?;

    printer.flush()
}

fn rasterize_image(image: impl Into<DynamicImage>) -> RasterBuffer {
    let image = image.into().into_luma8();

    let threshold = 128;
    let hshift = 720 - image.width();
    let mut lines = RasterBuffer::new(image.height());

    // Iterate over each pixel in the luma image
    for (x, y, pixel) in image.enumerate_pixels() {
        let x = x + hshift;
        // Extract the luma value (0-255)
        let luma = pixel.0[0];
        // Determine if the pixel is "on" or "off" based on a threshold
        if x < 720 && luma <= threshold {
            let pline = lines.get_mut(y as usize).unwrap();
            let idx = 89 - x / 8;
            let bit = x % 8;

            pline[idx as usize] |= 1 << bit;
        }
    }

    lines
}
