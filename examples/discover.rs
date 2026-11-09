//! Searches the network for label printers, as the apps' "Search the network" does.
//!
//! How to use: `cargo run --example discover [-- <wait ms> [known address…]]`; prints every
//! device that answered SNMP with its model, and how long the search took.

use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

fn main() {
    // `ping <address> [times]`: asks one device again and again; how often and how fast.
    let all: Vec<String> = std::env::args().skip(1).collect();
    if all.first().map(String::as_str) == Some("ping") {
        let address: Ipv4Addr = all[1].parse().expect("an address");
        let times: usize = all.get(2).and_then(|n| n.parse().ok()).unwrap_or(20);
        let target = std::net::SocketAddr::from((address, 161));
        for _ in 0..times {
            let started = Instant::now();
            let found = ql_raster::net::probe(&[target], Duration::from_millis(1000));
            match found.first() {
                Some(device) => println!("{} in {:?}", device.model, started.elapsed()),
                None => println!("no answer in 1 s"),
            }
        }
        return;
    }
    // `name <address>…`: how long reading each one's own name takes.
    if all.first().map(String::as_str) == Some("name") {
        for host in &all[1..] {
            let started = Instant::now();
            let name = ql_raster::net::query_name(host);
            println!("{host}: {name:?} in {:?}", started.elapsed());
        }
        return;
    }
    let mut args = all.into_iter();
    let wait = args
        .next()
        .and_then(|ms| ms.parse().ok())
        .map_or(Duration::from_millis(1500), Duration::from_millis);
    let known: Vec<Ipv4Addr> = args.filter_map(|address| address.parse().ok()).collect();

    println!(
        "searching {:?} for {wait:?}",
        ql_raster::net::searched_networks(&known)
    );
    let started = Instant::now();
    for found in ql_raster::net::discover(wait, &known) {
        println!(
            "{:<16} {:<30} {}",
            found.address,
            found.model,
            found.name.as_deref().unwrap_or("-")
        );
    }
    println!("took {:?}", started.elapsed());
}
