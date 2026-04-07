/// receiver.rs - Background thread for receiving server packets
///
/// Runs continuously after handshake, reading all incoming packets
/// from the server without blocking the main menu thread.
///
/// Handles:
///   PKT_ACK     - plain ACK (seq confirmation) or clearance text (non-empty payload)
///   PKT_LARGE_DATA - logs byte count, confirms receipt
///   PKT_ERROR   - displays server error message
///   PKT_HANDOFF_NOTIFY - signals client to prepare for handoff
///
/// Regulatory compliance:
///   - CARs SOR/96-433 Part V - all ATC instructions must reach the pilot
///   - DO-178C DAL-D - non-blocking receive with deterministic error handling
///
/// REQ-CLT-070, REQ-SVR-070
use std::io::Read;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::logger::Logger;
use crate::packet::{
    HEADER_SIZE, PKT_ACK, PKT_ERROR, PKT_HANDOFF_NOTIFY, PKT_LARGE_DATA, PacketHeader,
};

/// Spawns the background receiver thread.
///
/// `stream`      - a cloned TcpStream (from conn.stream.try_clone())
/// `logger`      - shared logger Arc
/// `alive`       - set to false by this thread when connection is lost
/// `handoff_flag`- set to true when server sends PKT_HANDOFF_NOTIFY
pub fn spawn_receiver(
    mut stream: TcpStream,
    logger: Arc<Logger>,
    alive: Arc<AtomicBool>,
    handoff_flag: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            // Read fixed 54-byte header
            let mut hdr_buf = [0u8; HEADER_SIZE];
            match read_exact_or_err(&mut stream, &mut hdr_buf) {
                Ok(false) => {
                    // Connection closed cleanly
                    alive.store(false, Ordering::SeqCst);
                    logger.log_connection("Receiver: server closed connection");
                    break;
                }
                Err(e) => {
                    alive.store(false, Ordering::SeqCst);
                    logger.log_error(&format!("Receiver: read error: {}", e));
                    break;
                }
                Ok(true) => {}
            }

            let header = PacketHeader::from_bytes(&hdr_buf);
            let packet_type = header.packet_type;
            let seq_num = header.seq_num;
            let payload_length = header.payload_length;
            let payload_len = payload_length as usize;

            // Read payload
            let mut payload = vec![0u8; payload_len];
            if payload_len > 0 {
                if let Err(e) = read_exact(&mut stream, &mut payload) {
                    alive.store(false, Ordering::SeqCst);
                    logger.log_error(&format!("Receiver: payload read error: {}", e));
                    break;
                }
            }

            // Dispatch by packet type
            match packet_type {
                PKT_ACK => {
                    if payload_len == 0 {
                        // Plain ACK - sequence confirmation
                        logger.log_rx("ACK", seq_num, 0, "server acknowledged packet");
                        println!("\n[ATC] ACK received (seq={})", seq_num);
                    } else {
                        // Non-empty payload = ATC clearance text (REQ-CLT-070, REQ-SVR-070)
                        let text = String::from_utf8_lossy(&payload).to_string();
                        logger.log_rx(
                            "ACK/CLEARANCE",
                            seq_num,
                            payload_length,
                            &text,
                        );
                        println!("\n--------------------------------------------------");
                        println!("  [ATC CLEARANCE] {}", text);
                        println!("--------------------------------------------------");
                    }
                }

                PKT_LARGE_DATA => {
                    // REQ-SYS-070: confirm large data received
                    logger.log_rx(
                        "LARGE_DATA",
                        seq_num,
                        payload_length,
                        &format!("{} bytes received", payload_len),
                    );
                    println!(
                        "\n[ATC] Weather data received: {} bytes ({:.2} KB)",
                        payload_len,
                        payload_len as f64 / 1024.0
                    );
                }

                PKT_ERROR => {
                    let msg = String::from_utf8_lossy(&payload).to_string();
                    logger.log_rx("ERROR", seq_num, payload_length, &msg);
                    println!("\n[!] ATC ERROR: {}", msg);
                }

                PKT_HANDOFF_NOTIFY => {
                    // REQ-CLT-060: server is handing us off to a new ATC
                    logger.log_rx(
                        "HANDOFF_NOTIFY",
                        seq_num,
                        0,
                        "server initiated handoff",
                    );
                    println!("\n[ATC] Handoff notification received - prepare to reconnect.");
                    handoff_flag.store(true, Ordering::SeqCst);
                }

                other => {
                    logger.log_rx(
                        &format!("UNKNOWN(0x{:02X})", other),
                        seq_num,
                        payload_length,
                        "unexpected packet type",
                    );
                }
            }

            // Re-print prompt so user knows they can still type
            print!("Select option: ");
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
    })
}

// I/O helpers

/// Read exactly `buf.len()` bytes. Returns Ok(true) on success,
/// Ok(false) if connection closed before first byte, Err on I/O error.
fn read_exact_or_err(stream: &mut TcpStream, buf: &mut [u8]) -> Result<bool, std::io::Error> {
    use std::io::ErrorKind;
    let mut total = 0;
    while total < buf.len() {
        match stream.read(&mut buf[total..]) {
            Ok(0) => return Ok(false),
            Ok(n) => total += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

fn read_exact(stream: &mut TcpStream, buf: &mut [u8]) -> std::io::Result<()> {
    use std::io::ErrorKind;
    let mut total = 0;
    while total < buf.len() {
        match stream.read(&mut buf[total..]) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "connection closed",
                ));
            }
            Ok(n) => total += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
