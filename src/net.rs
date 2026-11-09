//! Printing over TCP 9100, reading the status over SNMP (community `public`), and finding
//! printers on the local network.
//!
//! Finding ([`discover`]): one SNMP GET of the model name (`hrDeviceDescr.1`) to every address
//! of this PC's /24 network (and of the networks of addresses already known) at once, from one UDP socket; whoever answers within the time is
//! listed with its model. Brother QL printers answer `Brother QL-…`.

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use crate::PrinterError;
use crate::status::Status;

/// Raw printing port.
pub const PRINT_PORT: u16 = 9100;

/// Brother's status block OID (`brInfoPrinterUStatus`): the same 32 bytes as the raster
/// status reply.
const STATUS_OID: &[u32] = &[1, 3, 6, 1, 4, 1, 2435, 3, 3, 9, 1, 6, 1, 0];

/// The printer's own name OID (`sysName.0`): its node name, as set on the printer.
const NAME_OID: &[u32] = &[1, 3, 6, 1, 2, 1, 1, 5, 0];

/// Serial number OID (`prtGeneralSerialNumber.1`): `H6Z560817`, the serial on USB without its
/// leading zeros.
const SERIAL_OID: &[u32] = &[1, 3, 6, 1, 2, 1, 43, 5, 1, 1, 17, 1];

/// Model name OID (`hrDeviceDescr.1`).
const MODEL_OID: &[u32] = &[1, 3, 6, 1, 2, 1, 25, 3, 2, 1, 3, 1];

const TIMEOUT: Duration = Duration::from_millis(1500);

fn address(host: &str, port: u16) -> Result<SocketAddr, PrinterError> {
    (host.trim(), port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, format!("{host}: no address")).into()
        })
}

/// Sends a job to the printer at `host` (name or IP address).
pub fn send_job(host: &str, job: &[u8]) -> Result<(), PrinterError> {
    send_job_to(address(host, PRINT_PORT)?, job)
}

/// Sends a job to a socket address.
pub fn send_job_to(address: SocketAddr, job: &[u8]) -> Result<(), PrinterError> {
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT)?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    stream.write_all(job)?;
    stream.flush()?;
    Ok(())
}

/// One SNMP GET, asked a second time when the first gets no answer: UDP, and a printer asked
/// often now and then drops one.
fn snmp_get(host: &str, oid: &[u32]) -> Result<Vec<u8>, PrinterError> {
    snmp_get_once(host, oid).or_else(|_| snmp_get_once(host, oid))
}

fn snmp_get_once(host: &str, oid: &[u32]) -> Result<Vec<u8>, PrinterError> {
    let mut session = snmp::SyncSession::new(address(host, 161)?, b"public", Some(TIMEOUT), 0)?;
    let mut response = session.get(oid).map_err(|_| PrinterError::Snmp)?;

    match response.varbinds.next() {
        Some((_, snmp::Value::OctetString(bytes))) => Ok(bytes.to_vec()),
        _ => Err(PrinterError::Snmp),
    }
}

/// Reads the status block over SNMP.
pub fn query_status(host: &str) -> Result<Status, PrinterError> {
    Status::try_from(snmp_get(host, STATUS_OID)?.as_slice())
}

/// Reads the model name over SNMP (`Brother QL-810W`).
pub fn query_model_name(host: &str) -> Result<String, PrinterError> {
    Ok(String::from_utf8_lossy(&snmp_get(host, MODEL_OID)?)
        .trim()
        .to_owned())
}

/// Reads the printer's own name over SNMP (`sysName`, `BRW0080927AFBCE` or as set); `None`
/// when it has none.
pub fn query_name(host: &str) -> Result<Option<String>, PrinterError> {
    let name = String::from_utf8_lossy(&snmp_get(host, NAME_OID)?)
        .trim()
        .to_owned();
    Ok((!name.is_empty()).then_some(name))
}

/// Reads the printer's serial number over SNMP (`H6Z560817`); `None` when it gives none.
pub fn query_serial(host: &str) -> Result<Option<String>, PrinterError> {
    let serial = String::from_utf8_lossy(&snmp_get(host, SERIAL_OID)?)
        .trim()
        .to_owned();
    Ok((!serial.is_empty()).then_some(serial))
}

/// A device that answered the search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Its address.
    pub address: IpAddr,
    /// Its model name, as SNMP gives it (`Brother QL-810W`).
    pub model: String,
    /// Its own name (`sysName`: `LabelPrinter_2`, or `BRW…` when never set), for a Brother QL
    /// that gave one.
    pub name: Option<String>,
}

impl Found {
    /// Whether it is a Brother QL label printer (what we print on).
    #[must_use]
    pub fn is_brother_ql(&self) -> bool {
        self.model.contains("QL-")
    }
}

/// This PC's address on the network it would reach the internet through; no packet is sent.
#[must_use]
pub fn local_address() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(address) if !address.is_loopback() && !address.is_unspecified() => Some(address),
        _ => None,
    }
}

/// Searches this PC's /24 network and those of `known` addresses (printers set already,
/// over a VPN say) for devices answering SNMP, for `wait`.
#[must_use]
pub fn discover(wait: Duration, known: &[Ipv4Addr]) -> Vec<Found> {
    let own = local_address();
    let mut networks: Vec<[u8; 3]> = Vec::new();
    for address in own.iter().chain(known) {
        let [a, b, c, _] = address.octets();
        if !networks.contains(&[a, b, c]) {
            networks.push([a, b, c]);
        }
    }
    let addresses: Vec<SocketAddr> = networks
        .iter()
        .flat_map(|&[a, b, c]| (1..=254).map(move |d| Ipv4Addr::new(a, b, c, d)))
        .filter(|address| Some(*address) != own)
        .map(|address| SocketAddr::from((address, 161)))
        .collect();
    probe(&addresses, wait)
}

/// The /24 networks [`discover`] searches, as `192.168.1.x`.
#[must_use]
pub fn searched_networks(known: &[Ipv4Addr]) -> Vec<String> {
    let mut networks: Vec<String> = Vec::new();
    for address in local_address().iter().chain(known) {
        let [a, b, c, _] = address.octets();
        let network = format!("{a}.{b}.{c}.x");
        if !networks.contains(&network) {
            networks.push(network);
        }
    }
    networks
}

/// An SNMPv2c GET for several values (community `public`); the `snmp` crate builds one for a
/// single value only.
fn get_request(id: i32, oids: &[&[u32]]) -> Vec<u8> {
    fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
        let mut out = vec![tag];
        let length = content.len();
        if length < 0x80 {
            out.push(u8::try_from(length).unwrap_or(0));
        } else {
            let bytes: Vec<u8> = length
                .to_be_bytes()
                .into_iter()
                .skip_while(|byte| *byte == 0)
                .collect();
            out.push(0x80 | u8::try_from(bytes.len()).unwrap_or(0));
            out.extend(bytes);
        }
        out.extend_from_slice(content);
        out
    }
    fn integer(value: i32) -> Vec<u8> {
        let bytes = value.to_be_bytes();
        // The shortest two's complement form.
        let mut start = 0;
        while start < 3
            && ((bytes[start] == 0 && bytes[start + 1] & 0x80 == 0)
                || (bytes[start] == 0xff && bytes[start + 1] & 0x80 != 0))
        {
            start += 1;
        }
        tlv(0x02, &bytes[start..])
    }
    fn oid(parts: &[u32]) -> Vec<u8> {
        let mut out = vec![u8::try_from(parts[0] * 40 + parts[1]).unwrap_or(0)];
        for &part in &parts[2..] {
            let mut groups = vec![u8::try_from(part & 0x7f).unwrap_or(0)];
            let mut rest = part >> 7;
            while rest > 0 {
                groups.push(0x80 | u8::try_from(rest & 0x7f).unwrap_or(0));
                rest >>= 7;
            }
            groups.reverse();
            out.extend(groups);
        }
        tlv(0x06, &out)
    }

    let varbinds: Vec<u8> = oids
        .iter()
        .flat_map(|parts| tlv(0x30, &[oid(parts), vec![0x05, 0x00]].concat()))
        .collect();
    let pdu = tlv(
        0xa0,
        &[integer(id), integer(0), integer(0), tlv(0x30, &varbinds)].concat(),
    );
    // Version 1: SNMPv2c, as the snmp crate sends.
    tlv(0x30, &[integer(1), tlv(0x04, b"public"), pdu].concat())
}

/// When [`probe`] asks again the addresses that haven't answered, after the first ask.
///
/// Windows holds only a few packets while it looks up an address on the network (ARP), so a
/// burst to a whole /24 loses the request to most devices it hasn't talked to lately; a Wi-Fi
/// printer in power save misses one too. By the second ask the lookups are done.
const ASK_AGAIN: [Duration; 4] = [
    Duration::from_millis(300),
    Duration::from_millis(600),
    Duration::from_millis(900),
    Duration::from_millis(1200),
];

/// Asks every address for its model name, again after [`ASK_AGAIN`] those that haven't
/// answered; the ones that answer within `wait`, in address order.
#[must_use]
pub fn probe(addresses: &[SocketAddr], wait: Duration) -> Vec<Found> {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else {
        return Vec::new();
    };
    let ask = |found: &[Found]| {
        for (index, address) in addresses.iter().enumerate() {
            if found.iter().any(|known| known.address == address.ip()) {
                continue;
            }
            let id = i32::try_from(index).unwrap_or(i32::MAX);
            // The model and the device's own name in one request: a second request right
            // after the burst is often lost.
            let request = get_request(id, &[MODEL_OID, NAME_OID]);
            // A full send buffer or an unreachable address only loses that one.
            let _ = socket.send_to(&request, address);
        }
    };

    let started = Instant::now();
    let deadline = started + wait;
    let mut asks = ASK_AGAIN
        .iter()
        .map(|after| started + *after)
        .filter(|at| *at < deadline);
    let mut next_ask = asks.next();
    let mut found: Vec<Found> = Vec::new();
    ask(&found);
    let mut buffer = [0_u8; 1500];
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        if next_ask.is_some_and(|at| now >= at) {
            ask(&found);
            next_ask = asks.next();
        }
        // Wait for answers until the next ask or the end.
        let until = next_ask.map_or(deadline, |at| at.min(deadline));
        let left = until
            .saturating_duration_since(now)
            .max(Duration::from_millis(1));
        if socket.set_read_timeout(Some(left)).is_err() {
            break;
        }
        let (length, from) = match socket.recv_from(&mut buffer) {
            Ok(received) => received,
            // Windows reports a closed port (ICMP) of an earlier send this way: go on.
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => continue,
            // The read timed out: time to ask again, or the end.
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(_) => break,
        };
        let Ok(mut reply) = snmp::SnmpPdu::from_bytes(&buffer[..length]) else {
            continue;
        };
        let text = |value: Option<(snmp::ObjectIdentifier<'_>, snmp::Value<'_>)>| match value {
            Some((_, snmp::Value::OctetString(bytes))) => {
                Some(String::from_utf8_lossy(bytes).trim().to_owned())
            }
            _ => None,
        };
        if let Some(model) = text(reply.varbinds.next())
            && !found.iter().any(|known| known.address == from.ip())
        {
            let name = text(reply.varbinds.next()).filter(|name| !name.is_empty());
            found.push(Found {
                address: from.ip(),
                model,
                name,
            });
        }
    }
    found.sort_by_key(|device| device.address);
    found
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn job_arrives_byte_for_byte() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let address = listener.local_addr().expect("bound");

        let printer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("a connection");
            let mut received = Vec::new();
            stream.read_to_end(&mut received).expect("the job");
            received
        });

        let job: Vec<u8> = (0..5000_u32).map(|value| (value % 251) as u8).collect();
        send_job_to(address, &job).expect("sent");

        assert_eq!(printer.join().expect("printer thread"), job);
    }

    /// A minimal SNMP agent on `socket` that answers one GET with `model`.
    fn fake_agent(socket: UdpSocket, model: &'static str) -> std::thread::JoinHandle<()> {
        fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
            let mut out = vec![tag, u8::try_from(content.len()).expect("short")];
            out.extend_from_slice(content);
            out
        }
        std::thread::spawn(move || {
            let mut buffer = [0_u8; 1500];
            let (length, from) = socket.recv_from(&mut buffer).expect("a request");
            let request = snmp::SnmpPdu::from_bytes(&buffer[..length]).expect("SNMP");
            let id = request.req_id.to_be_bytes();
            // 1.3.6.1.2.1.25.3.2.1.3.1
            let oid = [0x2b, 6, 1, 2, 1, 25, 3, 2, 1, 3, 1];
            let varbind = tlv(
                0x30,
                &[tlv(0x06, &oid), tlv(0x04, model.as_bytes())].concat(),
            );
            let pdu = tlv(
                0xa2,
                &[
                    tlv(0x02, &id),
                    tlv(0x02, &[0]),
                    tlv(0x02, &[0]),
                    tlv(0x30, &varbind),
                ]
                .concat(),
            );
            // Version 1: SNMPv2c, as the crate asks.
            let message = tlv(0x30, &[tlv(0x02, &[1]), tlv(0x04, b"public"), pdu].concat());
            socket.send_to(&message, from).expect("reply");
        })
    }

    #[test]
    fn probe_lists_who_answers() {
        let agent = UdpSocket::bind("127.0.0.1:0").expect("a free port");
        let address = agent.local_addr().expect("bound");
        let quiet = UdpSocket::bind("127.0.0.1:0").expect("a free port");
        let silent = quiet.local_addr().expect("bound");
        let handle = fake_agent(agent, "Brother QL-810W");

        let found = probe(&[silent, address], Duration::from_millis(500));
        handle.join().expect("agent");
        assert_eq!(
            found,
            [Found {
                address: address.ip(),
                model: "Brother QL-810W".to_owned(),
                name: None,
            }]
        );
        assert!(found[0].is_brother_ql());
    }

    #[test]
    fn a_get_request_matches_the_snmp_crate_and_carries_several_values() {
        for id in [0, 1, 127, 128, 300, 70_000, i32::MAX] {
            let mut expected = snmp::pdu::Buf::default();
            snmp::pdu::build_get(b"public", id, MODEL_OID, &mut expected);
            assert_eq!(get_request(id, &[MODEL_OID]), &expected[..], "id {id}");
        }
        let both = get_request(5, &[MODEL_OID, NAME_OID]);
        let parsed = snmp::SnmpPdu::from_bytes(&both).expect("SNMP");
        assert_eq!(parsed.req_id, 5);
        assert_eq!(parsed.varbinds.count(), 2);
    }

    #[test]
    fn unknown_host_is_an_error() {
        assert!(send_job("printer.invalid", b"x").is_err());
    }
}
