# ql-raster

Brother QL label printers from Rust, without Brother's driver or b-PAC: draw a label as a
1-bit bitmap, turn it into a raster job, print it over the network (TCP 9100) or USB, and read
the printer's status.

Tested on the QL-710W and QL-810W; the QL-720NW, QL-800 and QL-820NWB follow the same
reference.

## What's in it

| Module | What it does |
|---|---|
| `label` | Labels as 1-bit bitmaps at 300 dpi: `Bitmap`, `TextLabel` (lines set as large as they fit, `*bold*` and `_italic_`), typefaces Barlow, Roboto and Source Serif 4 (compiled in, OFL). |
| `media` | Every tape and label size of the 720-pin QL printers, with its print area on the head. |
| `raster` | A bitmap as raster lines for the head; PackBits (TIFF) compression. |
| `job` | The command stream of a job: pages, cutting (each, at the end, off), high resolution, compression. |
| `status` | The 32-byte status block: model, errors, loaded media. |
| `net` | Jobs over TCP 9100; status, model, name and serial over SNMP; searching the network for printers. Feature `net` (default). |
| `Printer` | A printer by its address, network or USB alike: status, send a job, abort. |
| `usb` | The same on USB, addressed as `usb:<serial>`. Windows, through its own "USB Printing Support" driver: no Brother driver, no Zadig. |

## Example

```rust
use ql_raster::label::{Length, Run, TextLabel};
use ql_raster::{Printer, job, raster};

let printer = Printer::new("192.168.1.57"); // or "usb:000H6Z560817"
let status = printer.status()?;
let media = status.loaded_media().ok_or("no known tape loaded")?;
let bitmap = TextLabel::new("Hello\n*label*").render(u32::from(media.print_pins), Length::Free, Run::AlongTape);
printer.send_job(&job::job_bytes(&raster::raster_lines(&bitmap, media)?, media))?;
```

`cargo run --example print_text -- <printer> "first line" "second line"` prints on whatever tape
is loaded; `<printer>` is a host name, an IP address or `usb:<serial>`.

## Good to know

- A QL on USB *and* the network: every USB request (a status poll too) holds the printer to USB
  until USB has been quiet for about 6 seconds. Network jobs are accepted meanwhile but wait.
  Don't poll USB while printing over the network.
- The QL's own network port 9100 is one-way: status comes over SNMP, not as an answer on the
  socket.
- Over USB the QL prints while it receives and stops at a zero raster line without TIFF
  compression: blank lines go as `g` lines.

The Brother references the code is checked against are in `doc/`.

## Credits

The command definitions started from Ryan Kurte's
[rust-ptouch](https://github.com/ryankurte/rust-ptouch) (MPL-2.0), for the P-touch tape
printers. Thanks, Ryan!

## License

MIT or Apache-2.0, at your option. The fonts in `fonts/` are under the SIL Open Font License
(their licences are next to them).
