//! Transport helpers — UDP (control + data) and TCP.
//!
//! The radar typically listens on UDP (default `5002` for the app) and/or TCP
//! (default `5001`). These thin wrappers keep the protocol modules transport-agnostic.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use crate::control::ControlFrame;
use crate::data::DataFrame;
use crate::error::Result;

/// A UDP endpoint connected to one radar.
pub struct UdpRadar {
    socket: UdpSocket,
    peer: SocketAddr,
}

impl UdpRadar {
    /// Bind a local socket and "connect" it to the radar address.
    pub fn connect<A: ToSocketAddrs>(radar: A) -> Result<Self> {
        let peer = radar.to_socket_addrs()?.next().ok_or_else(|| {
            crate::error::Error::Io("could not resolve radar address".into())
        })?;
        let bind: SocketAddr = if peer.is_ipv4() {
            "0.0.0.0:0".parse().unwrap()
        } else {
            "[::]:0".parse().unwrap()
        };
        let socket = UdpSocket::bind(bind)?;
        socket.connect(peer)?;
        Ok(UdpRadar { socket, peer })
    }

    /// The radar address this endpoint talks to.
    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    /// Set the socket read timeout.
    pub fn set_read_timeout(&self, t: Option<Duration>) -> Result<()> {
        self.socket.set_read_timeout(t)?;
        Ok(())
    }

    /// Send a control frame.
    pub fn send_control(&self, frame: &ControlFrame) -> Result<usize> {
        Ok(self.socket.send(&frame.encode())?)
    }

    /// Send raw bytes.
    pub fn send_raw(&self, bytes: &[u8]) -> Result<usize> {
        Ok(self.socket.send(bytes)?)
    }

    /// Receive one datagram into `buf`; returns the number of bytes.
    pub fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.socket.recv(buf)?)
    }

    /// Receive one datagram and parse it as a checksum-validated data frame.
    pub fn recv_data_frame(&self) -> Result<DataFrame> {
        let mut buf = [0u8; 65535];
        let n = self.socket.recv(&mut buf)?;
        DataFrame::parse(&buf[..n])
    }
}

/// A TCP connection to one radar.
pub struct TcpRadar {
    stream: TcpStream,
}

impl TcpRadar {
    /// Connect, with an optional read timeout.
    pub fn connect<A: ToSocketAddrs>(radar: A, timeout: Option<Duration>) -> Result<Self> {
        let stream = TcpStream::connect(radar)?;
        if let Some(t) = timeout {
            stream.set_read_timeout(Some(t))?;
        }
        Ok(TcpRadar { stream })
    }

    /// Send a control frame.
    pub fn send_control(&mut self, frame: &ControlFrame) -> Result<()> {
        self.stream.write_all(&frame.encode())?;
        self.stream.flush()?;
        Ok(())
    }

    /// Send raw bytes.
    pub fn send_raw(&mut self, bytes: &[u8]) -> Result<()> {
        self.stream.write_all(bytes)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Read into `buf`; returns the byte count (0 == EOF).
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.stream.read(buf)?)
    }
}
