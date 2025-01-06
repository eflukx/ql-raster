#![feature(array_chunks)]
use image::ImageReader;
use ql_raster::{commands::AdvancedMode, prelude::*};
use std::time::Duration;

fn main() {
    println!("Hello, world!");
    let mut printer =
        printer::from_addr("labelprinter_3:9100").expect("Unable to connect to printer!");

    let status = printer.get_status();
    println!("Status {:?}", status);
    let name = printer.get_snmp_name();
    println!("name {:?}", name);
    let model = printer.get_snmp_model();
    println!("model {:?}", model);

    print(&mut printer).expect("Printing miserably failed!");

    let status = printer.get_snmp_status();
    println!("Status {:?}", status);

    std::thread::sleep(Duration::from_secs(1));
}

fn print(printer: &mut PTouchPrinter<PTouchTcpInterface>) -> Result<()> {
    // let demo_line = [0x55; 40];
    // let demo_line2 = [0xaa; 40];
    let demo_line = [0xf0; 80];
    let demo_line2 = [0x0f; 80];

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

    let pixbuf = load_png(128);
    for line_data in pixbuf {
        printer.transfer_raster_line(&line_data)?;
    }
    // for y in 0..90 {
    //     let line_data = get_line(y, 720);
    //     printer.transfer_raster_line(&line_data)?;
    // }

    printer.print_and_feed()?;

    printer.flush()
}

fn get_line(y: u16, width: usize) -> Vec<u8> {
    let bytes: Vec<u8> = (0..width).map(|x| x as u8 ^ y as u8).collect();
    let bitmap: Vec<u8> = bytes
        .array_chunks::<8>()
        .map(|chunk| bytes_to_bitmap(chunk, y as u8))
        .collect();

    bitmap
}

fn bytes_to_bitmap(bytes: &[u8; 8], threshold: u8) -> u8 {
    let thresh = |val, shift| if val > threshold { 1u8 } else { 0 } << shift;

    let mut out = 0;
    for bit in 0..7 {
        out |= thresh(bytes[bit], bit)
    }

    out
}

fn load_png(threshold: u8) -> Vec<[u8; 90]> {
    let img = ImageReader::open("label.png").unwrap().decode().unwrap();

    let hshift = 720 - img.width();
    let mut lines = new_pixbuffer(img.height());

    let luma_img = img.into_luma8();
    // Iterate over each pixel in the luma image
    for (x, y, pixel) in luma_img.enumerate_pixels() {
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

fn new_pixbuffer(height: u32) -> Vec<[u8; 90]> {
    let mut pbuf = Vec::with_capacity(height as usize);

    for _line in 0..height {
        pbuf.push([0; 90]);
    }

    pbuf
}
