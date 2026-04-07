/// heartbeat.rs - Active connection keepalive and loss detection
///
/// Spawns a thread that periodically sends a PKT_ACK (used as keepalive)
/// to the server. If the send fails, the connection is considered lost,
/// the alive flag is cleared, and the pilot is alerted.
///
/// Heartbeat events are written to a SEPARATE log file from the main
/// session log. This separation is intentional for a safety-critical
/// system: the heartbeat log provides an independent record of connection
/// health that is not interleaved with packet data.
///
/// Regulatory compliance:
///   - CARs SOR/96-433 Part VI s.602.137 - communication watchdog for
///     safety-critical ATC links; loss of link must be detected and reported
///   - DO-178C DAL-D - independent monitoring thread with its own audit trail
///
/// REQ-CLT-080, REQ-COM-020
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::Local;

use crate::packet::{HandshakePayload, PKT_ACK, Packet, PacketHeader};

/// Heartbeat interval in seconds.
pub const HEARTBEAT_INTERVAL_SECS: u64 = 5;

/// Spawn the heartbeat thread.
///
/// `stream`   - cloned TcpStream
/// `alive`    - shared flag; set to false on connection loss
/// `callsign` - used to populate aircraft_id in the keepalive packet
pub fn spawn_heartbeat(
    mut stream: TcpStream,
    alive: Arc<AtomicBool>,
    callsign: String,
) -> std::thread::JoinHandle<()> {
    // Open a dedicated heartbeat log file (REQ: separate file for safety audit)
    let hb_log = open_heartbeat_log();

    std::thread::spawn(move || {
        let mut hb_log = hb_log;
        let mut hb_seq: u32 = 0;

        loop {
            std::thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));

            // Stop if connection already known dead (receiver thread may have set this)
            if !alive.load(Ordering::SeqCst) {
                write_hb_log(
                    &mut hb_log,
                    "STOP",
                    "connection already dead - heartbeat exiting",
                );
                break;
            }

            hb_seq += 1;
            let pkt = build_keepalive(hb_seq, &callsign);

            match send_keepalive(&mut stream, &pkt) {
                Ok(_) => {
                    write_hb_log(
                        &mut hb_log,
                        "OK",
                        &format!("keepalive sent (hb_seq={})", hb_seq),
                    );
                }
                Err(e) => {
                    // Connection lost - alert pilot and set flag
                    write_hb_log(
                        &mut hb_log,
                        "LOST",
                        &format!("send failed (hb_seq={}): {}", hb_seq, e),
                    );
                    alive.store(false, Ordering::SeqCst);

                    // Print alert directly as menu thread may be blocked on input
                    println!("\n");
                    println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
                    println!("  [ALERT] CONNECTION TO ATC SERVER LOST");
                    println!("  Heartbeat failed - please reconnect.");
                    println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
                    println!("Select option: ");
                    break;
                }
            }
        }
    })
}

// Internal helpers

fn open_heartbeat_log() -> File {
    let ts = Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("atc_heartbeat_{}.log", ts);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)
        .expect("Cannot create heartbeat log");
    eprintln!("[Heartbeat] Log: {}", filename);
    file
}

fn write_hb_log(file: &mut File, status: &str, detail: &str) {
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[HEARTBEAT] | {} | {} | {}\n", ts, status, detail);
    file.write_all(line.as_bytes()).ok();
    file.flush().ok();
}

fn build_keepalive(seq: u32, callsign: &str) -> Packet {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    Packet {
        header: PacketHeader {
            packet_type: PKT_ACK, // keepalive reuses PKT_ACK with empty payload
            seq_num: seq,
            timestamp: ts,
            payload_length: 0,
            origin_atc_id: 0,
            aircraft_id: HandshakePayload::str_to_fixed(callsign),
            emergency_flag: 0,
        },
        payload: vec![],
    }
}

fn send_keepalive(stream: &mut TcpStream, pkt: &Packet) -> std::io::Result<()> {
    use std::io::Write;
    stream.write_all(&pkt.header.to_bytes())?;
    stream.flush()
}
