use std::time::Duration;

use ql_raster::{
    commands::{Commands, PrintInfo, VariousMode},
    prelude::*,
    printer::PTouchPrinter,
};

fn main() {
    println!("Hello, world!");
    let mut printer =
        printer::from_addr("labelprinter_3:9100").expect("Unable to connect to printer!");

    let status = printer.get_snmp_status();
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
        width: Some(50),
        length: None,
        raster_no: 1,
        recover: true,
    };
    printer.set_print_info(&pi)?;

    printer.set_various_mode(VariousMode::AUTO_CUT)?;
    // printer.set_advanced_mode(AdvancedMode::);
    printer.set_page_no(1)?;

    printer.set_margin(0)?;
    printer.set_compression_mode(ql_raster::commands::CompressionMode::None)?;

    for _x in 0..15 {
        printer.transfer_raster_line(&demo_line)?;
        printer.transfer_raster_line(&demo_line)?;
        printer.transfer_raster_line(&demo_line)?;
        printer.transfer_raster_line(&demo_line)?;
        printer.transfer_raster_line(&demo_line2)?;
        printer.transfer_raster_line(&demo_line2)?;
        printer.transfer_raster_line(&demo_line2)?;
        printer.transfer_raster_line(&demo_line2)?;
    }
    printer.print_and_feed()?;

    printer.flush()
}
