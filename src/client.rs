//! TCP client: connect, heartbeat, send control, receive + parse.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::error::Result;
use crate::frame::{Frame, FrameReader};
use crate::messages::{self, Message};

/// A connected radar client over TCP.
pub struct RadarClient {
    stream: TcpStream,
    counter: u32,
    reader: FrameReader,
}

impl RadarClient {
    /// Connect to `addr` (e.g. `192.168.8.167:5001`) and send the login.
    pub fn connect(addr: &str, read_timeout: Option<Duration>) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(read_timeout)?;
        stream.set_nodelay(true).ok();
        let mut c = RadarClient { stream, counter: 1, reader: FrameReader::new() };
        // LOGIN first — the radar sends no data until this handshake.
        c.send_payload(&messages::login(0))?;
        Ok(c)
    }

    /// Next outgoing message counter.
    pub fn next_counter(&mut self) -> u32 {
        let c = self.counter;
        self.counter = self.counter.wrapping_add(1);
        c
    }

    /// Set the outgoing counter (to mirror the app's current value).
    pub fn set_counter(&mut self, c: u32) {
        self.counter = c;
    }

    fn send_payload(&mut self, payload: &[u8]) -> Result<()> {
        self.stream.write_all(&Frame::new(payload.to_vec()).encode())?;
        self.stream.flush()?;
        Ok(())
    }

    /// Send a heartbeat; returns the counter used.
    pub fn heartbeat(&mut self) -> Result<u32> {
        let c = self.next_counter();
        self.send_payload(&messages::heartbeat(c))?;
        Ok(c)
    }

    /// Send a SetRegister command.
    pub fn set_register(&mut self, regs: &[(u32, u32)]) -> Result<u32> {
        let c = self.next_counter();
        self.send_payload(&messages::set_register(c, regs))?;
        Ok(c)
    }

    /// Rotate the radar (working mode = Cir sweep).
    pub fn rotate(&mut self) -> Result<u32> {
        self.set_register(&[(messages::REG_ROTATE, messages::VAL_ROTATE)])
    }

    /// Stop the radar (working mode = Stand by).
    pub fn stop(&mut self) -> Result<u32> {
        self.set_register(&[(messages::REG_ROTATE, messages::VAL_STOP)])
    }

    /// Send a SetJson (settings) command.
    pub fn set_json(&mut self, json: &str) -> Result<u32> {
        let c = self.next_counter();
        self.send_payload(&messages::set_json(c, json))?;
        Ok(c)
    }

    /// Read whatever is available and return parsed messages.
    /// Returns `Ok(vec![])` on a read timeout (no data ready).
    pub fn recv(&mut self) -> Result<Vec<Message>> {
        let mut buf = [0u8; 16384];
        match self.stream.read(&mut buf) {
            Ok(0) => Ok(vec![]),
            Ok(n) => {
                let mut out = Vec::new();
                for f in self.reader.push(&buf[..n]) {
                    if let Ok(m) = Message::parse(&f.payload) {
                        out.push(m);
                    }
                }
                Ok(out)
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                Ok(vec![])
            }
            Err(e) => Err(e.into()),
        }
    }
}
