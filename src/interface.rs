use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use crate::Result;

pub trait PTouchInterface: Sized {
    fn name(&self) -> String;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    fn read_vec(&mut self) -> Result<Vec<u8>>;

    fn write(&mut self, data: &[u8]) -> Result<()>;

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct PTouchTcpInterface {
    socket: TcpStream,
}

impl PTouchTcpInterface {
    pub fn new<A: ToSocketAddrs>(addr: A, read_timeout: Option<Duration>) -> Result<Self> {
        let socket = TcpStream::connect(addr)?;
        socket.set_read_timeout(read_timeout)?;

        println!("sokket: {socket:?}");
        Ok(PTouchTcpInterface { socket })
    }
}

impl PTouchInterface for PTouchTcpInterface {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.socket.read(buf)?)
    }

    fn name(&self) -> String {
        format!(
            "PTouch TCP interface on {}",
            self.socket
                .peer_addr()
                .map(|sa| sa.to_string())
                .unwrap_or_default()
        )
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.socket.write_all(data)?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.socket.flush()?;
        Ok(())
    }

    fn read_vec(&mut self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.socket.read_to_end(&mut buf)?;
        Ok(buf)
    }
}
