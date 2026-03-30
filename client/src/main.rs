/// main.rs — ATC Communications Client (Aircraft endpoint)
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
///   - CARs SOR/96-433 Part V (Airworthiness) — safety-critical communications
///   - DO-178C DAL-D — deterministic startup, traceable operations
///
/// REQ-CLT-010, REQ-CLT-030, REQ-SYS-080, REQ-PKT-061

