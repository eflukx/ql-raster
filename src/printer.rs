use crate::{prelude::*, PTouchError};
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    time::Duration,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1500);

/// SNMP OIDs for getting information from the printer over the network interface e.g. `Status`
#[allow(dead_code)]
mod snmp_oid {
    pub const STATUS: &[u32] = &[1, 3, 6, 1, 4, 1, 2435, 3, 3, 9, 1, 6, 1, 0];
    pub const NAME: &[u32] = &[1, 3, 6, 1, 2, 1, 1, 6, 0];
    pub const MODEL: &[u32] = &[1, 3, 6, 1, 2, 1, 25, 3, 2, 1, 3, 1];
    pub const SERIAL: &[u32] = &[1, 3, 6, 1, 2, 1, 43, 5, 1, 1, 17];
    pub const IP_ADDR: &[u32] = &[1, 3, 6, 1, 4, 1, 1240, 2, 3, 4, 5, 2, 3, 0];
    pub const SUBNET: &[u32] = &[1, 3, 6, 1, 4, 1, 1240, 2, 3, 4, 5, 2, 4, 0];
    pub const MAC: &[u32] = &[1, 3, 6, 1, 4, 1, 1240, 2, 3, 4, 5, 2, 12, 0];
}

// pub enum EngineHandHeld {} // 128px wide @ 180dpi (18mm)
// pub enum EngineDeskLabel {} // 720px wide @ 300dpi (61mm)
// pub enum EngineDeskLabelWide {} // 1296px wide @ 300dpi (110mm)

// const DOTS_PER_LINE: usize = 720;
// const BYTES_PER_LINE: usize = DOTS_PER_LINE / 8;

pub struct PTouchPrinter<D> {
    pub interface: D,
    pub ip_addr: Option<IpAddr>,
    send_buffer: Option<Vec<u8>>, // Probably use a type (of PTouchPrinter) to diff between buffered and direct io
}

impl PTouchPrinter<PTouchTcpInterface> {
    pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        from_addr(addr)
    }

    pub fn get_snmp_status(&self) -> Result<Status> {
        Status::try_from(self.get_snmp(snmp_oid::STATUS)?.as_slice())
    }

    pub fn get_snmp_name(&self) -> Result<String> {
        self.get_snmp_string(snmp_oid::NAME)
    }

    pub fn get_snmp_model(&self) -> Result<String> {
        self.get_snmp_string(snmp_oid::MODEL)
    }

    pub fn get_snmp_serial(&self) -> Result<String> {
        self.get_snmp_string(snmp_oid::SERIAL)
    }

    fn get_snmp_string(&self, oid: &[u32]) -> Result<String> {
        let r = self.get_snmp(oid)?;
        Ok(String::from_utf8_lossy(r.as_slice()).into())
    }

    fn get_snmp(&self, oid: &[u32]) -> Result<Vec<u8>> {
        use snmp::{SyncSession, Value};

        if let Some(ip_addr) = self.ip_addr {
            let addr = SocketAddr::new(ip_addr, 161);
            let timeout = Duration::from_millis(500);
            let mut snmp_session = SyncSession::new(addr, b"public", Some(timeout), 0)?;

            let mut response = snmp_session.get(oid).map_err(|_| PTouchError::SNMPError)?;
            if let Some((_oid, Value::OctetString(response_data))) = response.varbinds.next() {
                return Ok(response_data.into());
            }
        }

        Err(PTouchError::SNMPError)
    }
}

pub fn from_addr<A: ToSocketAddrs>(addr: A) -> Result<PTouchPrinter<PTouchTcpInterface>> {
    let ip_addr = addr
        .to_socket_addrs()
        .ok()
        .and_then(|mut e| e.next())
        .map(|sa| sa.ip());

    Ok(PTouchPrinter {
        ip_addr,
        interface: PTouchTcpInterface::new(addr, Some(DEFAULT_TIMEOUT))?,
        // send_buffer: Some(Vec::with_capacity(2048)), // buffered IO
        send_buffer: None, // unbuffered, immediate IO
    })
}

impl<D: PTouchInterface> PTouchPrinter<D> {
    // pub fn get_status(&mut self) -> Result<Status> {
    //     Ok(Status)
    // }

    // Todo: send `Command` type
    pub fn write(&mut self, data: impl AsRef<[u8]>) -> Result<()> {
        if let Some(buffer) = self.send_buffer.as_mut() {
            buffer.extend_from_slice(data.as_ref());
            Ok(())
        } else {
            self.interface.write(data.as_ref())
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        if let Some(buffer) = self.send_buffer.as_mut() {
            self.interface.write(buffer.as_slice())?;
            buffer.clear();
        }

        Ok(())
    }

    // // Todo: return `Reponse` type
    // pub fn send_raw_with_response(&mut self, command: impl AsRef<[u8]>) -> Result<Vec<u8>> {
    //     self.interface.write(command.as_ref())?;
    //     self.interface.read_vec()
    // }
}
