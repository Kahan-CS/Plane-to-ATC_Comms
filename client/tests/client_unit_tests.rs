/// client_unit_tests.rs
/// Formal integration-level unit tests for the Rust ATC client.
///
/// Covers:
///   - Packet structure sizes and constants  (maps to issue #7)
///   - Connection retry and error behaviour  (maps to issue #9)
///
/// Regulatory compliance:
///   - CARs SOR/96-433 Part V
///   - DO-178C DAL-D

use std::mem::size_of;

use client::network::Connection;
use client::packet::{
    HandshakePayload, LandingPayload, PacketHeader, TakeoffPayload, TransitPayload,
    PKT_ACK, PKT_DISCONNECT, PKT_ERROR, PKT_HANDOFF_NOTIFY, PKT_HANDSHAKE,
    PKT_LANDING, PKT_LARGE_DATA, PKT_LARGE_DATA_REQUEST, PKT_MAYDAY, PKT_TAKEOFF,
    PKT_TRANSIT,
};

// PACKET TESTS (issue #7)

/// REQ-PKT-031: PacketHeader binary layout must be exactly 54 bytes.
/// Packed struct — used local variable to avoid E0793.
#[test]
fn test_packet_header_is_54_bytes() {
    let size = size_of::<PacketHeader>();
    assert_eq!(size, 54);
}

/// REQ-PKT-010: Every packet type constant must match the agreed wire value.
#[test]
fn test_all_packet_type_constants_defined() {
    assert_eq!(PKT_HANDSHAKE,         0x01);
    assert_eq!(PKT_ACK,               0x02);
    assert_eq!(PKT_ERROR,             0x03);
    assert_eq!(PKT_TAKEOFF,           0x04);
    assert_eq!(PKT_TRANSIT,           0x05);
    assert_eq!(PKT_LANDING,           0x06);
    assert_eq!(PKT_MAYDAY,            0x07);
    assert_eq!(PKT_LARGE_DATA_REQUEST,0x08);
    assert_eq!(PKT_LARGE_DATA,        0x09);
    assert_eq!(PKT_HANDOFF_NOTIFY,    0x0A);
    assert_eq!(PKT_DISCONNECT,        0x0B);
}

/// REQ-PKT-034: emergency_flag must be readable directly from the header
/// without deserializing the payload.
#[test]
fn test_emergency_flag_readable_from_header() {
    let header = PacketHeader {
        packet_type:    PKT_MAYDAY,
        seq_num:        1,
        timestamp:      0,
        payload_length: 0,
        origin_atc_id:  0,
        aircraft_id:    [0u8; 32],
        emergency_flag: 1,
    };
    // Copy to local to avoid packed-struct reference (E0793)
    let flag = header.emergency_flag;
    assert_eq!(flag, 1, "emergency_flag should be 1 for MAYDAY");
}

/// REQ-PKT-020: A buffered handoff packet must have a non-zero origin_atc_id.
#[test]
fn test_buffered_packet_has_nonzero_atc_id() {
    let header = PacketHeader {
        packet_type:    PKT_HANDSHAKE,
        seq_num:        5,
        timestamp:      0,
        payload_length: 0,
        origin_atc_id:  1001,
        aircraft_id:    [0u8; 32],
        emergency_flag: 0,
    };
    let atc_id = header.origin_atc_id;
    assert_ne!(atc_id, 0,    "buffered packet must have non-zero origin_atc_id");
    assert_eq!(atc_id, 1001, "origin_atc_id must equal the value set");
}

/// REQ-PKT-020: A live (non-buffered) packet must have origin_atc_id == 0.
#[test]
fn test_live_packet_has_zero_atc_id() {
    let header = PacketHeader {
        packet_type:    PKT_HANDSHAKE,
        seq_num:        1,
        timestamp:      0,
        payload_length: 0,
        origin_atc_id:  0,
        aircraft_id:    [0u8; 32],
        emergency_flag: 0,
    };
    let atc_id = header.origin_atc_id;
    assert_eq!(atc_id, 0, "live packet must have origin_atc_id == 0");
}

/// REQ-PKT-030, REQ-SYS-020: Serialized header must be exactly 54 bytes
/// no more, no less matching the C server's expected layout.
#[test]
fn test_header_serializes_to_54_bytes() {
    let header = PacketHeader {
        packet_type:    PKT_HANDSHAKE,
        seq_num:        1,
        timestamp:      1_700_000_000,
        payload_length: 52,
        origin_atc_id:  0,
        aircraft_id:    [0u8; 32],
        emergency_flag: 0,
    };
    let bytes = header.to_bytes();
    assert_eq!(bytes.len(), 54, "serialized header must be exactly 54 bytes");
}

/// REQ-PKT-061: HandshakePayload packed size must match C struct (52 bytes).
#[test]
fn test_handshake_payload_fits_in_52_bytes() {
    let size = size_of::<HandshakePayload>();
    assert_eq!(size, 52);
}

/// REQ-CLT-040: TakeoffPayload packed size must match C struct (41 bytes).
#[test]
fn test_takeoff_payload_size() {
    let size = size_of::<TakeoffPayload>();
    assert_eq!(size, 41);
}

/// REQ-CLT-040: TransitPayload packed size must match C struct (14 bytes).
#[test]
fn test_transit_payload_size() {
    let size = size_of::<TransitPayload>();
    assert_eq!(size, 14);
}

/// REQ-CLT-040: LandingPayload packed size must match C struct (27 bytes).
#[test]
fn test_landing_payload_size() {
    let size = size_of::<LandingPayload>();
    assert_eq!(size, 27);
}

// CONNECTION TESTS (issue #9) 

/// REQ-COM-020: Connection::connect must return Err after exhausting
/// MAX_RETRY_ATTEMPTS — it must not loop forever.
#[test]
fn test_connection_fails_after_max_retries() {
    let result = Connection::connect("127.0.0.1:19999");
    assert!(result.is_err(), "expected Err after max retries on unreachable port");
}

/// REQ-CLT-080: A failed connection must return Err, never panic.
/// Verifies the retry loop has deterministic, safe termination.
#[test]
fn test_connection_returns_error_not_panic() {
    let outcome = std::panic::catch_unwind(|| {
        Connection::connect("127.0.0.1:19999")
    });
    assert!(outcome.is_ok(), "Connection::connect must not panic");
    let result = outcome.unwrap();
    assert!(result.is_err(), "Connection::connect must return Err on unreachable address");
}
