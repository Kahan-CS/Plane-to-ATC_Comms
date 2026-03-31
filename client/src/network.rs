// network.rs: TCP connection management for ATC Client
//
// Regulatory compliance:
//   - CARs SOR/96-433 Part V: reliable comms for safety-critical data
//   - DO-178C DAL-D: deterministic error handling, no silent failures
//
// REQ-CLT-010, REQ-COM-010, REQ-SYS-080, REQ-CLT-080, REQ-COM-020

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::packet::{Packet, PacketHeader, HEADER_SIZE};

// REQ-COM-020: max retries before terminal failure
pub const MAX_RETRY_ATTEMPTS: u32 = 3;
// REQ-CLT-080: timeout triggers connection-lost detection
pub const READ_TIMEOUT_SECS: u64 = 10;

pub struct Connection {
    pub stream: TcpStream,
}

impl Connection {
    /// Connect to ATC server. Retries up to MAX_RETRY_ATTEMPTS (REQ-COM-020).
    pub fn connect(addr: &str) -> io::Result<Self> {
        let mut attempts = 0u32;
        loop {
            match TcpStream::connect(addr) {
                Ok(stream) => {
                    stream.set_read_timeout(Some(Duration::from_secs(READ_TIMEOUT_SECS)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
                    stream.set_nodelay(true)?;
                    return Ok(Connection { stream });
                }
                Err(e) => {
                    attempts += 1;
                    eprintln!(
                        "[NET] Connect attempt {}/{} failed: {}",
                        attempts, MAX_RETRY_ATTEMPTS, e
                    );
                    if attempts >= MAX_RETRY_ATTEMPTS {
                        return Err(io::Error::new(
                            io::ErrorKind::ConnectionRefused,
                            format!(
                                "Terminal failure: could not connect after {} attempts",
                                MAX_RETRY_ATTEMPTS
                            ),
                        ));
                    }
                    std::thread::sleep(Duration::from_secs(2));
                }
            }
        }
    }

    /// Send a packet: header then payload, no raw strings (REQ-SYS-020).
    pub fn send_packet(&mut self, pkt: &Packet) -> io::Result<()> {
        self.stream.write_all(&pkt.header.to_bytes())?;
        if !pkt.payload.is_empty() {
            self.stream.write_all(&pkt.payload)?;
        }
        self.stream.flush()
    }

    /// Receive one packet: fixed header then dynamic payload (REQ-PKT-033).
    pub fn recv_packet(&mut self) -> io::Result<Packet> {
        let mut buf = [0u8; HEADER_SIZE];
        self.stream.read_exact(&mut buf)?;
        let header = PacketHeader::from_bytes(&buf);
        let mut payload = vec![0u8; header.payload_length as usize];
        if header.payload_length > 0 {
            self.stream.read_exact(&mut payload)?;
        }
        Ok(Packet { header, payload })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_fails_after_max_retries() {
        // REQ-COM-020: terminal failure after MAX_RETRY_ATTEMPTS
        let result = Connection::connect("127.0.0.1:19999");
        assert!(result.is_err());
    }
}
