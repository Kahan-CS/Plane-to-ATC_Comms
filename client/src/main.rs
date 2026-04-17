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
/// REQ-CLT-010/020/030/040/050/060/070/080
/// REQ-SYS-040/070/080/090
/// REQ-LOG-010/030/040/050/060
/// REQ-COM-010/020/040/060

mod buffer;
mod heartbeat;
mod logger;
mod network;
mod packet;
mod phases;
mod receiver;

use std::io::{self, BufRead, Write};
use std::process;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

use buffer::{flush_buffer, HandoffBuffer};
use heartbeat::spawn_heartbeat;
use logger::Logger;
use network::Connection;
use packet::{
    HandshakePayload, Packet, PacketHeader, PKT_ACK, PKT_DISCONNECT, PKT_ERROR, PKT_HANDSHAKE,
    PKT_HANDOFF_NOTIFY, PKT_LARGE_DATA_REQUEST, PKT_MAYDAY, PHASE_LANDING, PHASE_TAKEOFF,
    PHASE_TRANSIT,
};
use phases::{build_landing_packet, build_takeoff_packet, build_transit_packet};
use receiver::spawn_receiver;

/// Previous server ATC ID used on buffered handoff retransmission.
const PREV_ATC_ID: u32 = 1;

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
    let mut handoff_buffer = HandoffBuffer::new(PREV_ATC_ID);
    run_session(
        &server_addr,
        &callsign,
        &ac_type,
        &ac_model,
        &origin,
        &dest,
        &logger,
        &mut handoff_buffer,
        PHASE_TAKEOFF,
    );

    // REQ-LOG-060: session summary written on clean exit
    logger.write_summary();
}

/// Connect, handshake, run menu, and reconnect when handoff is triggered.
fn run_session(
    server_addr: &str,
    callsign: &str,
    ac_type: &str,
    ac_model: &str,
    origin: &str,
    dest: &str,
    logger: &Arc<Logger>,
    handoff_buffer: &mut HandoffBuffer,
    initial_phase: u8,
) {
    println!("Connecting to ATC server at {}...", server_addr);

    let mut conn = Connection::connect(server_addr).unwrap_or_else(|e| {
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
        callsign: HandshakePayload::str_to_fixed(callsign),
        aircraft_type: HandshakePayload::str_to_fixed(ac_type),
        aircraft_model: HandshakePayload::str_to_fixed(ac_model),
        origin: HandshakePayload::str_to_fixed(origin),
        destination: HandshakePayload::str_to_fixed(dest),
        initial_phase,
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
            aircraft_id: HandshakePayload::str_to_fixed(callsign),
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
        &format!("{} {}/{} {} -> {}", callsign, ac_type, ac_model, origin, dest),
    );
    seq += 1;

    // Wait for ACK (REQ-SYS-080)
    match conn.recv_packet() {
        Ok(pkt) if pkt.header.packet_type == PKT_ACK => {
            let ack_seq = pkt.header.seq_num;
            let ack_len = pkt.header.payload_length;
            println!("[OK] Connection verified by ATC.");
            logger.log_rx("ACK", ack_seq, ack_len, "handshake acknowledged");
        }
        Ok(pkt) if pkt.header.packet_type == PKT_ERROR => {
            logger.log_error("ATC rejected handshake - PKT_ERROR received");
            logger.write_summary();
            eprintln!("[FATAL] ATC rejected handshake.");
            process::exit(5);
        }
        Ok(pkt) => {
            let ptype = pkt.header.packet_type;
            logger.log_error(&format!(
                "Unexpected packet type during handshake: 0x{:02X}",
                ptype
            ));
            logger.write_summary();
            eprintln!("[FATAL] Unexpected response: 0x{:02X}", ptype);
            process::exit(6);
        }
        Err(e) => {
            logger.log_error(&format!("No ACK received: {}", e));
            logger.write_summary();
            eprintln!("[FATAL] No ACK received: {}", e);
            process::exit(7);
        }
    }

    if !handoff_buffer.is_empty() {
        flush_buffer(handoff_buffer, &mut conn, logger.as_ref());
    }

    let alive = Arc::new(AtomicBool::new(true));
    let handoff_flag = Arc::new(AtomicBool::new(false));
    let phase_state = Arc::new(AtomicU8::new(initial_phase));

    let recv_stream = conn.stream.try_clone().unwrap_or_else(|e| {
        logger.log_error(&format!("Failed to clone stream for receiver: {}", e));
        logger.write_summary();
        eprintln!("[FATAL] Cannot start receiver thread: {}", e);
        process::exit(8);
    });
    let _recv_handle = spawn_receiver(
        recv_stream,
        Arc::clone(logger),
        Arc::clone(&alive),
        Arc::clone(&handoff_flag),
        Arc::clone(&phase_state),
    );

    // active keepalive thread for early connection-loss detection.
    let hb_stream = conn.stream.try_clone().unwrap_or_else(|e| {
        logger.log_error(&format!("Failed to clone stream for heartbeat: {}", e));
        logger.write_summary();
        eprintln!("[FATAL] Cannot start heartbeat thread: {}", e);
        process::exit(9);
    });
    let _hb_handle = spawn_heartbeat(hb_stream, Arc::clone(&alive), callsign.to_string());

    // Interactive menu (REQ-SYS-040)

    let (should_reconnect, reconnect_phase) = run_menu(
        &mut conn,
        &mut seq,
        callsign,
        logger,
        &alive,
        &handoff_flag,
        handoff_buffer,
        &phase_state,
    );

    if should_reconnect {
        println!();
        println!(
            "[Reconnect] Enter next ATC server (ip:port), or press Enter to reuse {}",
            server_addr
        );
        print!("New server: ");
        io::stdout().flush().ok();

        let mut next = String::new();
        io::stdin().read_line(&mut next).ok();
        let next_server = if next.trim().is_empty() {
            server_addr.to_string()
        } else {
            next.trim().to_string()
        };

        logger.log_connection(&format!("Reconnect to {}", next_server));
        run_session(
            &next_server,
            callsign,
            ac_type,
            ac_model,
            origin,
            dest,
            logger,
            handoff_buffer,
            reconnect_phase,
        );
    }
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
    handoff_buffer: &mut HandoffBuffer,
    phase_state: &Arc<AtomicU8>,
) -> (bool, u8) {
    let stdin = io::stdin();
    loop {
        if !alive.load(Ordering::SeqCst) {
            println!("\n[NET] Connection lost. Switching to reconnect prompt.");
            logger.log_connection("Connection lost detected by receiver thread; requesting reconnect");
            break (true, phase_state.load(Ordering::SeqCst));
        }

        if handoff_flag.load(Ordering::SeqCst) {
            println!("\n[ATC] Handoff notice active. Reconnecting to next ATC...");
            logger.log_connection("Handoff notify flag raised by receiver thread");
            handoff_flag.store(false, Ordering::SeqCst);
            return (true, phase_state.load(Ordering::SeqCst));
        }

        println!();
        println!("┌─ATC Cockpit Menu ─────────┐");
        println!("│  1. Takeoff                                     │");
        println!("│  2. Transiting / Airborne                       │");
        println!("│  3. Landing                                     │");
        println!("│  4. MAYDAY  !!!!!                               │");
        println!("│  5. Disconnect / Inactive                       │");
        println!("│  6. Request Weather Data (large transfer)       │");
        println!("│  7. Trigger ATC Handoff (simulate)              │");
        println!("└─────────────┘");
        print!("Select option: ");
        io::stdout().flush().ok();

        let mut input = String::new();
        if stdin.lock().read_line(&mut input).is_err() {
            logger.log_error("Failed to read user input — exiting menu");
            break (false, phase_state.load(Ordering::SeqCst));
        }

        match input.trim() {
            "1" => {
                let pkt = build_takeoff_packet(*seq, callsign, current_timestamp());
                send_or_buffer(conn, pkt, seq, "TAKEOFF", logger, handoff_buffer, alive);
            }
            "2" => {
                phase_state.store(PHASE_TRANSIT, Ordering::SeqCst);
                let pkt = build_transit_packet(*seq, callsign, current_timestamp());
                send_or_buffer(conn, pkt, seq, "TRANSIT", logger, handoff_buffer, alive);
            }
            "3" => {
                phase_state.store(PHASE_LANDING, Ordering::SeqCst);
                let pkt = build_landing_packet(*seq, callsign, current_timestamp());
                send_or_buffer(conn, pkt, seq, "LANDING", logger, handoff_buffer, alive);
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
                send_or_buffer(conn, pkt, seq, "MAYDAY", logger, handoff_buffer, alive);
            }
            "5" => {
                send_disconnect(conn, seq, callsign, logger.as_ref());
                println!("Disconnected. Goodbye.");
                break (false, phase_state.load(Ordering::SeqCst));
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
                send_or_buffer(
                    conn,
                    pkt,
                    seq,
                    "LARGE_DATA_REQUEST",
                    logger,
                    handoff_buffer,
                    alive,
                );
            }
            "7" => {
                println!("[Handoff] Triggering ATC handoff...");
                logger.log_connection("Handoff triggered by pilot (menu option 7)");
                let pkt = Packet {
                    header: PacketHeader {
                        packet_type: PKT_HANDOFF_NOTIFY,
                        seq_num: *seq,
                        timestamp: current_timestamp(),
                        payload_length: 0,
                        origin_atc_id: PREV_ATC_ID,
                        aircraft_id: HandshakePayload::str_to_fixed(callsign),
                        emergency_flag: 0,
                    },
                    payload: vec![],
                };
                conn.send_packet(&pkt).ok();
                logger.log_tx("HANDOFF_NOTIFY", *seq, 0, "pilot-initiated handoff");
                *seq += 1;
                return (true, phase_state.load(Ordering::SeqCst));
            }
            other => {
                let msg = format!("Invalid menu input: '{}'", other.trim());
                println!("Unknown option '{}'. Please enter 1-6.", other.trim());
                logger.log_error(&msg);
            }
        }
    }
}

/// Send packet, or buffer it for handoff retransmission on failure.
fn send_or_buffer(
    conn: &mut Connection,
    pkt: Packet,
    seq: &mut u32,
    ptype_str: &str,
    logger: &Arc<Logger>,
    buffer: &mut HandoffBuffer,
    alive: &Arc<AtomicBool>,
) {
    let plen = pkt.header.payload_length;
    match conn.send_packet(&pkt) {
        Ok(_) => {
            if ptype_str == "MAYDAY" {
                logger.log_mayday(*seq);
            } else {
                logger.log_tx(ptype_str, *seq, plen, &format!("{} sent", ptype_str));
            }
        }
        Err(e) => {
            logger.log_error(&format!("{} send failed: {}", ptype_str, e));
            buffer.push(pkt);
            logger.log_connection(&format!(
                "Buffered failed packet for handoff: type={} seq={}",
                ptype_str, *seq
            ));
            alive.store(false, Ordering::SeqCst);
        }
    }
    *seq += 1;
}

/// Send PKT_DISCONNECT cleanly.
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
