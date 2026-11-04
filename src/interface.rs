use std::{
    io::{Read, Write},
    net::{IpAddr, TcpStream, ToSocketAddrs},
    time::Duration,
};

use crate::PTResult;

pub trait PTouchInterface: Sized {
    fn name(&self) -> String;

    fn read(&mut self, buf: &mut [u8]) -> PTResult<usize>;

    fn read_vec(&mut self) -> PTResult<Vec<u8>>;

    fn write(&mut self, data: &[u8]) -> PTResult<()>;

    fn flush(&mut self) -> PTResult<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct PTouchTcpInterface {
    socket: TcpStream,
    pub(crate) ip_addr: Option<IpAddr>,
}

impl PTouchTcpInterface {
    pub fn new<A: ToSocketAddrs>(addr: A, read_timeout: Option<Duration>) -> PTResult<Self> {
        let ip_addr = addr.to_socket_addrs()?.next().map(|sa| sa.ip());

        let socket = TcpStream::connect(addr)?;
        socket.set_read_timeout(read_timeout)?;

        Ok(PTouchTcpInterface { socket, ip_addr })
    }
}

impl PTouchInterface for PTouchTcpInterface {
    fn name(&self) -> String {
        format!(
            "PTouch TCP interface on {}",
            self.socket
                .peer_addr()
                .map(|sa| sa.to_string())
                .unwrap_or_default()
        )
    }

    fn read(&mut self, buf: &mut [u8]) -> PTResult<usize> {
        Ok(self.socket.read(buf)?)
    }

    fn read_vec(&mut self) -> PTResult<Vec<u8>> {
        let mut buf = Vec::new();
        self.socket.read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn write(&mut self, data: &[u8]) -> PTResult<()> {
        self.socket.write_all(data)?;
        Ok(())
    }

    fn flush(&mut self) -> PTResult<()> {
        self.socket.flush()?;
        Ok(())
    }
}
