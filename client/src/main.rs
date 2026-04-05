/// main.rs: ATC Communications Client (Aircraft endpoint)
///
/// Functionality:
///   - CLI argument parsing (server IP:port, callsign, aircraft info)
///   - TCP connection with retry logic
///   - Handshake packet TX + ACK RX
///   - Structured packet send/receive (no raw strings)
///   - Session logging to timestamped file
///
/// Run:  atc-client <SERVER_IP> <PORT> <CALLSIGN> <TYPE> <MODEL> <ORIGIN> <DEST>
/// e.g.: atc-client 127.0.0.1 9000 AC8821 B737 737-800 CYYZ CYVR
///
/// Regulatory compliance:
///   - CARs SOR/96-433 Part V (Airworthiness): safety-critical communications
///   - DO-178C DAL-D: deterministic startup, traceable operations
///
/// REQ-CLT-010, REQ-CLT-030, REQ-SYS-080, REQ-PKT-061

mod packet;
mod network;
// logger module added in next commit

use packet::{
    HandshakePayload, Packet, PacketHeader,
    PKT_ACK, PKT_DISCONNECT, PKT_ERROR, PKT_HANDSHAKE,
};
use network::Connection;

use std::io::{self, BufRead, Write};
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 8 {
        eprintln!(
            "Usage: {} <SERVER_IP> <PORT> <CALLSIGN> <TYPE> <MODEL> <ORIGIN> <DEST>",
            args[0]
        );
        eprintln!("Example: {} 127.0.0.1 9000 AC8821 B737 737-800 CYYZ CYVR", args[0]);
        process::exit(1);
    }

    let server_addr = format!("{}:{}", args[1], args[2]);
    let callsign  = args[3].clone();
    let ac_type   = args[4].clone();
    let ac_model  = args[5].clone();
    let origin    = args[6].clone();
    let dest      = args[7].clone();

    // Connect (REQ-CLT-010, REQ-COM-020) ────────────────
    println!("=== ATC Communications Client ===");
    println!("Aircraft : {} ({}/{})", callsign, ac_type, ac_model);
    println!("Route    : {} → {}", origin, dest);
    println!("Server   : {}", server_addr);
    println!();
    println!("Connecting to ATC server...");

    let mut conn = Connection::connect(&server_addr).unwrap_or_else(|e| {
        eprintln!("[FATAL] {}", e);
        process::exit(2);
    });

    println!("[OK] Connected.");

    // Handshake (REQ-SYS-080, REQ-PKT-061, REQ-CLT-030) 
    let mut seq: u32 = 1;

    let hs_payload = HandshakePayload {
        callsign:       HandshakePayload::str_to_fixed(&callsign),
        aircraft_type:  HandshakePayload::str_to_fixed(&ac_type),
        aircraft_model: HandshakePayload::str_to_fixed(&ac_model),
        origin:         HandshakePayload::str_to_fixed(&origin),
        destination:    HandshakePayload::str_to_fixed(&dest),
    };
    let payload_bytes = hs_payload.to_bytes();

    let handshake = Packet {
        header: PacketHeader {
            packet_type:    PKT_HANDSHAKE,
            seq_num:        seq,
            timestamp:      current_timestamp(),
            payload_length: payload_bytes.len() as u32,
            origin_atc_id:  0,
            aircraft_id:    HandshakePayload::str_to_fixed(&callsign),
            emergency_flag: 0,
        },
        payload: payload_bytes,
    };

    print!("Sending handshake... ");
    io::stdout().flush().ok();

    conn.send_packet(&handshake).unwrap_or_else(|e| {
        eprintln!("[FATAL] Handshake send failed: {}", e);
        process::exit(3);
    });
    seq += 1;

    // Wait for ACK (REQ-SYS-080)
    match conn.recv_packet() {
        Ok(pkt) if pkt.header.packet_type == PKT_ACK => {
            println!("[OK] Connection verified by ATC.");
        }
        Ok(pkt) if pkt.header.packet_type == PKT_ERROR => {
            eprintln!("[FATAL] ATC rejected handshake.");
            process::exit(4);
        }
        Ok(pkt) => {
            eprintln!("[FATAL] Unexpected response: 0x{:02X}", pkt.header.packet_type);
            process::exit(5);
        }
        Err(e) => {
            eprintln!("[FATAL] No ACK received: {}", e);
            process::exit(6);
        }
    }

    // Interactive menu (REQ-SYS-040) ──
    run_menu(&mut conn, &mut seq, &callsign);
}

/// Main interactive menu loop.
/// Options shown at every prompt: pilot selects by number (REQ-SYS-040).
fn run_menu(conn: &mut Connection, seq: &mut u32, callsign: &str) {
    let stdin = io::stdin();
    loop {
        println!();
        println!("┌─ATC Cockpit Menu ─────────┐");
        println!("│  1. Takeoff                                     │");
        println!("│  2. Transiting / Airborne                       │");
        println!("│  3. Landing                                     │");
        println!("│  4. MAYDAY  !!!!!                               │");
        println!("│  5. Disconnect / Inactive                       │");
        println!("│  6. Request Weather Data (large transfer)       │");
        println!("└─────────────┘");
        print!("Select option: ");
        io::stdout().flush().ok();

        let mut input = String::new();
        if stdin.lock().read_line(&mut input).is_err() {
            break;
        }

        match input.trim() {
            "1" => {
                println!("[Takeoff]:");
                // TODO : collect takeoff telemetry and send PKT_TAKEOFF
            }
            "2" => {
                println!("[Transit]:");
                // TODO : collect transit telemetry and send PKT_TRANSIT
            }
            "3" => {
                println!("[Landing]:");
                // TODO : collect landing telemetry and send PKT_LANDING
            }
            "4" => {
                println!("[MAYDAY]:");
                // TODO : set emergency_flag in header and send PKT_MAYDAY
            }
            "5" => {
                send_disconnect(conn, seq, callsign);
                println!("Disconnected. Goodbye.");
                break;
            }
            "6" => {
                println!("[Weather]:");
                // TODO : send PKT_LARGE_DATA_REQUEST and receive ≥1 MB response
            }
            other => {
                println!("Unknown option '{}'. Please enter 1–6.", other);
            }
        }
    }
}

/// Send PKT_DISCONNECT cleanly before exiting.
fn send_disconnect(conn: &mut Connection, seq: &mut u32, callsign: &str) {
    let pkt = Packet {
        header: PacketHeader {
            packet_type:    PKT_DISCONNECT,
            seq_num:        *seq,
            timestamp:      current_timestamp(),
            payload_length: 0,
            origin_atc_id:  0,
            aircraft_id:    HandshakePayload::str_to_fixed(callsign),
            emergency_flag: 0,
        },
        payload: vec![],
    };
    conn.send_packet(&pkt).ok();
    *seq += 1;
}

/// Current Unix timestamp (UTC).
fn current_timestamp() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
