//! Prints a text label on whatever tape the printer has loaded.
//!
//! How to use: `cargo run --example print_text -- <printer> "first line" "second line"`, the
//! printer as a host name or IP address (network) or `usb:<serial>` (USB, Windows).

use ql_raster::label::{Length, Run, TextLabel};
use ql_raster::{job, net, raster, usb};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let printer = args.next().ok_or("usage: print_text <printer> <line>…")?;
    let text = args.collect::<Vec<_>>().join("\n");

    // The status says which tape is loaded; the label is drawn for it.
    let status = if usb::is_usb(&printer) {
        usb::query_status(&printer)?
    } else {
        net::query_status(&printer)?
    };
    let media = status.loaded_media().ok_or("no known tape loaded")?;
    println!("{} with {media}", status.model);

    let length = match media.print_length_dots {
        0 => Length::Free,
        dots => Length::Fixed(u32::from(dots)),
    };
    let bitmap = TextLabel::new(&text).render(u32::from(media.print_pins), length, Run::AlongTape);
    let job = job::job_bytes(&raster::raster_lines(&bitmap, media)?, media);

    if usb::is_usb(&printer) {
        usb::send_job(&printer, &job)?;
    } else {
        net::send_job(&printer, &job)?;
    }
    Ok(())
}
