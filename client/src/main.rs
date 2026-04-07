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
/// REQ-CLT-010, REQ-CLT-030, REQ-SYS-080, REQ-PKT-061, REQ-LOG-010, REQ-LOG-030
mod logger;
mod network;
mod packet;
mod phases;
mod receiver;

use logger::Logger;
use network::Connection;
use packet::{
    HandshakePayload, PKT_ACK, PKT_DISCONNECT, PKT_ERROR, PKT_HANDSHAKE,
    PKT_LARGE_DATA_REQUEST, PKT_MAYDAY, Packet, PacketHeader,
};
use phases::{build_landing_packet, build_takeoff_packet, build_transit_packet};
use receiver::spawn_receiver;

use std::io::{self, BufRead, Write};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 8 {
        eprintln!(
            "Usage: {} <SERVER_IP> <PORT> <CALLSIGN> <TYPE> <MODEL> <ORIGIN> <DEST>",
            args[0]
        );
        eprintln!(
            "Example: {} 127.0.0.1 9000 AC8821 B737 737-800 CYYZ CYVR",
            args[0]
        );
        process::exit(1);
    }

    let server_addr = format!("{}:{}", args[1], args[2]);
    let callsign = args[3].clone();
    let ac_type = args[4].clone();
    let ac_model = args[5].clone();
    let origin = args[6].clone();
    let dest = args[7].clone();

    // REQ-LOG-010, REQ-LOG-040: create timestamped log file at startup
    let logger = Arc::new(Logger::new("atc_client").unwrap_or_else(|e| {
        eprintln!("[FATAL] Cannot create log file: {}", e);
        process::exit(2);
    }));
    logger.log_connection(&format!(
        "Session started — {} ({}/{}) {} -> {}",
        callsign, ac_type, ac_model, origin, dest
    ));

    // Connect (REQ-CLT-010, REQ-COM-020)
    println!("=== ATC Communications Client ===");
    println!("Aircraft : {} ({}/{})", callsign, ac_type, ac_model);
    println!("Route    : {} -> {}", origin, dest);
    println!("Server   : {}", server_addr);
    println!();
    println!("Connecting to ATC server...");

    let mut conn = Connection::connect(&server_addr).unwrap_or_else(|e| {
        logger.log_error(&format!("Terminal connection failure: {}", e));
        logger.write_summary();
        eprintln!("[FATAL] {}", e);
        process::exit(3);
    });

    println!("[OK] Connected.");
    logger.log_connection(&format!("Connected to {}", server_addr));

    // Handshake (REQ-SYS-080, REQ-PKT-061, REQ-CLT-030)
    let mut seq: u32 = 1;

    let hs_payload = HandshakePayload {
        callsign: HandshakePayload::str_to_fixed(&callsign),
        aircraft_type: HandshakePayload::str_to_fixed(&ac_type),
        aircraft_model: HandshakePayload::str_to_fixed(&ac_model),
        origin: HandshakePayload::str_to_fixed(&origin),
        destination: HandshakePayload::str_to_fixed(&dest),
    };

    // Capture length before move into Packet (avoids use-after-move)
    let payload_bytes = hs_payload.to_bytes();
    let payload_len = payload_bytes.len() as u32;

    let handshake = Packet {
        header: PacketHeader {
            packet_type: PKT_HANDSHAKE,
            seq_num: seq,
            timestamp: current_timestamp(),
            payload_length: payload_len,
            origin_atc_id: 0,
            aircraft_id: HandshakePayload::str_to_fixed(&callsign),
            emergency_flag: 0,
        },
        payload: payload_bytes,
    };

    print!("Sending handshake... ");
    io::stdout().flush().ok();

    conn.send_packet(&handshake).unwrap_or_else(|e| {
        logger.log_error(&format!("Handshake send failed: {}", e));
        logger.write_summary();
        eprintln!("[FATAL] Handshake send failed: {}", e);
        process::exit(4);
    });

    // Log TX using payload_len captured before the move (REQ-LOG-030)
    logger.log_tx(
        "HANDSHAKE",
        seq,
        payload_len,
        &format!(
            "{} {}/{} {} -> {}",
            callsign, ac_type, ac_model, origin, dest
        ),
    );
    seq += 1;

    // Wait for ACK (REQ-SYS-080)
    match conn.recv_packet() {
        Ok(pkt) if pkt.header.packet_type == PKT_ACK => {
            println!("[OK] Connection verified by ATC.");
            logger.log_rx(
                "ACK",
                pkt.header.seq_num,
                pkt.header.payload_length,
                "handshake acknowledged",
            );
        }
        Ok(pkt) if pkt.header.packet_type == PKT_ERROR => {
            logger.log_error("ATC rejected handshake — PKT_ERROR received");
            logger.write_summary();
            eprintln!("[FATAL] ATC rejected handshake.");
            process::exit(5);
        }
        Ok(pkt) => {
            logger.log_error(&format!(
                "Unexpected packet type during handshake: 0x{:02X}",
                pkt.header.packet_type
            ));
            logger.write_summary();
            eprintln!(
                "[FATAL] Unexpected response: 0x{:02X}",
                pkt.header.packet_type
            );
            process::exit(6);
        }
        Err(e) => {
            logger.log_error(&format!("No ACK received: {}", e));
            logger.write_summary();
            eprintln!("[FATAL] No ACK received: {}", e);
            process::exit(7);
        }
    }

    // Start background receiver so incoming ATC messages never block menu input.
    let alive = Arc::new(AtomicBool::new(true));
    let handoff_flag = Arc::new(AtomicBool::new(false));
    let recv_stream = conn.stream.try_clone().unwrap_or_else(|e| {
        logger.log_error(&format!("Failed to clone stream for receiver: {}", e));
        logger.write_summary();
        eprintln!("[FATAL] Cannot start receiver thread: {}", e);
        process::exit(8);
    });
    let _recv_handle = spawn_receiver(
        recv_stream,
        Arc::clone(&logger),
        Arc::clone(&alive),
        Arc::clone(&handoff_flag),
    );

    // Interactive menu (REQ-SYS-040)
    run_menu(
        &mut conn,
        &mut seq,
        &callsign,
        &logger,
        &alive,
        &handoff_flag,
    );

    // REQ-LOG-060: session summary written on clean exit
    logger.write_summary();
}

/// Main interactive menu loop.
/// Options shown at every prompt: pilot selects by number (REQ-SYS-040).
/// Logger passed in so every menu action is recorded (REQ-LOG-030).
fn run_menu(
    conn: &mut Connection,
    seq: &mut u32,
    callsign: &str,
    logger: &Arc<Logger>,
    alive: &Arc<AtomicBool>,
    handoff_flag: &Arc<AtomicBool>,
) {
    let stdin = io::stdin();
    loop {
        if !alive.load(Ordering::SeqCst) {
            println!("\n[NET] Connection lost. Exiting interactive menu.");
            logger.log_connection("Connection lost detected by receiver thread");
            break;
        }

        if handoff_flag.load(Ordering::SeqCst) {
            println!("\n[ATC] Handoff notice active. Reconnect flow will be added in Commit-11.");
            logger.log_connection("Handoff notify flag raised by receiver thread");
            handoff_flag.store(false, Ordering::SeqCst);
        }

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
            logger.log_error("Failed to read user input — exiting menu");
            break;
        }

        match input.trim() {
            "1" => {
                let pkt = build_takeoff_packet(*seq, callsign, current_timestamp());
                let plen = pkt.header.payload_length;
                match conn.send_packet(&pkt) {
                    Ok(_) => {
                        logger.log_tx("TAKEOFF", *seq, plen, "takeoff telemetry sent");
                        println!("[Takeoff] Telemetry sent.");
                    }
                    Err(e) => {
                        logger.log_error(&format!("TAKEOFF send failed: {}", e));
                        println!("[Takeoff] Send failed: {}", e);
                    }
                }
                *seq += 1;
            }
            "2" => {
                let pkt = build_transit_packet(*seq, callsign, current_timestamp());
                let plen = pkt.header.payload_length;
                match conn.send_packet(&pkt) {
                    Ok(_) => {
                        logger.log_tx("TRANSIT", *seq, plen, "transit telemetry sent");
                        println!("[Transit] Telemetry sent.");
                    }
                    Err(e) => {
                        logger.log_error(&format!("TRANSIT send failed: {}", e));
                        println!("[Transit] Send failed: {}", e);
                    }
                }
                *seq += 1;
            }
            "3" => {
                let pkt = build_landing_packet(*seq, callsign, current_timestamp());
                let plen = pkt.header.payload_length;
                match conn.send_packet(&pkt) {
                    Ok(_) => {
                        logger.log_tx("LANDING", *seq, plen, "landing telemetry sent");
                        println!("[Landing] Telemetry sent.");
                    }
                    Err(e) => {
                        logger.log_error(&format!("LANDING send failed: {}", e));
                        println!("[Landing] Send failed: {}", e);
                    }
                }
                *seq += 1;
            }
            "4" => {
                // REQ-CLT-050, REQ-PKT-062: MAYDAY sets emergency_flag=1
                // REQ-LOG-050: logged with [MAYDAY] prefix
                println!("[MAYDAY] Transmitting emergency signal...");
                let pkt = Packet {
                    header: PacketHeader {
                        packet_type: PKT_MAYDAY,
                        seq_num: *seq,
                        timestamp: current_timestamp(),
                        payload_length: 0,
                        origin_atc_id: 0,
                        aircraft_id: HandshakePayload::str_to_fixed(callsign),
                        emergency_flag: 1,
                    },
                    payload: vec![],
                };
                match conn.send_packet(&pkt) {
                    Ok(_) => {
                        logger.log_mayday(*seq);
                        println!("[MAYDAY] Emergency signal sent.");
                    }
                    Err(e) => {
                        logger.log_error(&format!("MAYDAY send failed: {}", e));
                        println!("[MAYDAY] Send failed: {}", e);
                    }
                }
                *seq += 1;
            }
            "5" => {
                send_disconnect(conn, seq, callsign, logger.as_ref());
                println!("Disconnected. Goodbye.");
                break;
            }
            "6" => {
                // REQ-SYS-070: request >= 1 MB weather/telemetry data from server
                println!("[Weather] Requesting large weather data from ATC...");
                let pkt = Packet {
                    header: PacketHeader {
                        packet_type: PKT_LARGE_DATA_REQUEST,
                        seq_num: *seq,
                        timestamp: current_timestamp(),
                        payload_length: 0,
                        origin_atc_id: 0,
                        aircraft_id: HandshakePayload::str_to_fixed(callsign),
                        emergency_flag: 0,
                    },
                    payload: vec![],
                };
                match conn.send_packet(&pkt) {
                    Ok(_) => {
                        logger.log_tx(
                            "LARGE_DATA_REQUEST",
                            *seq,
                            0,
                            "requested weather/telemetry object >=1MB",
                        );
                        println!("[Weather] Request sent - awaiting data (see receiver)...");
                    }
                    Err(e) => {
                        logger.log_error(&format!("LARGE_DATA_REQUEST send failed: {}", e));
                        println!("[Weather] Request failed: {}", e);
                    }
                }
                *seq += 1;
            }
            other => {
                let msg = format!("Invalid menu input: '{}'", other.trim());
                println!("Unknown option '{}'. Please enter 1-6.", other.trim());
                logger.log_error(&msg);
            }
        }
    }
}

/// Send PKT_DISCONNECT cleanly before exiting.
fn send_disconnect(conn: &mut Connection, seq: &mut u32, callsign: &str, logger: &Logger) {
    let pkt = Packet {
        header: PacketHeader {
            packet_type: PKT_DISCONNECT,
            seq_num: *seq,
            timestamp: current_timestamp(),
            payload_length: 0,
            origin_atc_id: 0,
            aircraft_id: HandshakePayload::str_to_fixed(callsign),
            emergency_flag: 0,
        },
        payload: vec![],
    };

    match conn.send_packet(&pkt) {
        Ok(_) => logger.log_tx("DISCONNECT", *seq, 0, "graceful disconnect"),
        Err(e) => logger.log_error(&format!("DISCONNECT send failed: {}", e)),
    }
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
