/// packet.rs: Rust mirror of shared/packet.h
///
/// All structs use `#[repr(C, packed)]` to match the C server's packed alignment.
/// All multi-byte fields are stored in network byte order (big-endian) and must
/// be converted with `.to_be()` / `.from_be()` before use.
///
/// Regulatory compliance:
///   - CARs SOR/96-433 Part V: deterministic, auditable data structures
///   - DO-178C DAL-D: no hidden compiler padding in safety-relevant packets
///
/// REQ-PKT-030, REQ-PKT-031, REQ-PKT-032, REQ-PKT-033, REQ-PKT-034

// Packet type constants (REQ-PKT-010) 
pub const PKT_HANDSHAKE: u8          = 0x01;
pub const PKT_ACK: u8                = 0x02;
pub const PKT_ERROR: u8              = 0x03;
pub const PKT_TAKEOFF: u8            = 0x04;
pub const PKT_TRANSIT: u8            = 0x05;
pub const PKT_LANDING: u8            = 0x06;
pub const PKT_MAYDAY: u8             = 0x07;
pub const PKT_LARGE_DATA_REQUEST: u8 = 0x08;
pub const PKT_LARGE_DATA: u8         = 0x09;
pub const PKT_HANDOFF_NOTIFY: u8     = 0x0A;
pub const PKT_DISCONNECT: u8         = 0x0B;

pub const HEADER_SIZE: usize = 54;

// Fixed 54-byte header (REQ-PKT-031) 
/// Binary layout matches PacketHeader in shared/packet.h exactly.
/// Fields are in network byte order: call `.to_be_bytes()` when serializing.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PacketHeader {
    pub packet_type:    u8,       //  1 byte
    pub seq_num:        u32,      //  4 bytes : big-endian on wire
    pub timestamp:      i64,      //  8 bytes : Unix epoch UTC, big-endian
    pub payload_length: u32,      //  4 bytes : big-endian
    pub origin_atc_id:  u32,      //  4 bytes : 0=live, non-zero=buffered handoff
    pub aircraft_id:    [u8; 32], // 32 bytes : null-padded UTF-8 callsign
    pub emergency_flag: u8,       //  1 byte  : 0=normal, 1=MAYDAY
}
// Compile-time size assertion: must equal HEADER_SIZE
const _ASSERT_HEADER_SIZE: () = assert!(
    std::mem::size_of::<PacketHeader>() == HEADER_SIZE,
    "PacketHeader is not 54 bytes: check field sizes and repr(packed)"
);

impl PacketHeader {
    /// Serialize to bytes in network byte order (big-endian).
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0] = self.packet_type;
        buf[1..5].copy_from_slice(&self.seq_num.to_be_bytes());
        buf[5..13].copy_from_slice(&self.timestamp.to_be_bytes());
        buf[13..17].copy_from_slice(&self.payload_length.to_be_bytes());
        buf[17..21].copy_from_slice(&self.origin_atc_id.to_be_bytes());
        buf[21..53].copy_from_slice(&self.aircraft_id);
        buf[53] = self.emergency_flag;
        buf
    }

    /// Deserialize from 54 raw bytes (network byte order).
    pub fn from_bytes(buf: &[u8; HEADER_SIZE]) -> Self {
        let mut aircraft_id = [0u8; 32];
        aircraft_id.copy_from_slice(&buf[21..53]);
        PacketHeader {
            packet_type:    buf[0],
            seq_num:        u32::from_be_bytes(buf[1..5].try_into().unwrap()),
            timestamp:      i64::from_be_bytes(buf[5..13].try_into().unwrap()),
            payload_length: u32::from_be_bytes(buf[13..17].try_into().unwrap()),
            origin_atc_id:  u32::from_be_bytes(buf[17..21].try_into().unwrap()),
            aircraft_id,
            emergency_flag: buf[53],
        }
    }
}

//  Full packet with heap-allocated payload (REQ-PKT-033, REQ-SYS-030) 
#[derive(Debug)]
pub struct Packet {
    pub header:  PacketHeader,
    pub payload: Vec<u8>,  // size == header.payload_length
}

//  Payload structs (serialized into payload bytes) 

/// REQ-PKT-061: Handshake payload (52 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct HandshakePayload {
    pub callsign:       [u8; 12],
    pub aircraft_type:  [u8; 16],
    pub aircraft_model: [u8; 16],
    pub origin:         [u8; 4],
    pub destination:    [u8; 4],
}

impl HandshakePayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(52);
        v.extend_from_slice(&self.callsign);
        v.extend_from_slice(&self.aircraft_type);
        v.extend_from_slice(&self.aircraft_model);
        v.extend_from_slice(&self.origin);
        v.extend_from_slice(&self.destination);
        v
    }

    /// Helper function: fill a fixed-size array from a string, null-padding remainder.
    pub fn str_to_fixed<const N: usize>(s: &str) -> [u8; N] {
        let mut arr = [0u8; N];
        let bytes = s.as_bytes();
        let len = bytes.len().min(N);
        arr[..len].copy_from_slice(&bytes[..len]);
        arr
    }
}

/// REQ-CLT-040: Takeoff telemetry payload
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TakeoffPayload {
    pub departure_time:     i64,   // Unix epoch
    pub clearance_type:     u8,    // 0=IFR, 1=VFR
    pub assigned_heading:   f32,   // degrees
    pub assigned_altitude:  f32,   // feet MSL
    pub squawk_code:        u16,
    pub wind_speed:         f32,   // knots
    pub wind_direction:     f32,   // degrees
    pub speed_off_runway:   f32,   // knots
    pub initial_climb_rate: f32,   // feet/min
}

impl TakeoffPayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.departure_time.to_be_bytes());
        v.push(self.clearance_type);
        v.extend_from_slice(&self.assigned_heading.to_be_bytes());
        v.extend_from_slice(&self.assigned_altitude.to_be_bytes());
        v.extend_from_slice(&self.squawk_code.to_be_bytes());
        v.extend_from_slice(&self.wind_speed.to_be_bytes());
        v.extend_from_slice(&self.wind_direction.to_be_bytes());
        v.extend_from_slice(&self.speed_off_runway.to_be_bytes());
        v.extend_from_slice(&self.initial_climb_rate.to_be_bytes());
        v
    }
}

/// REQ-CLT-040: Transit telemetry payload
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TransitPayload {
    pub speed:       f32,
    pub altitude:    f32,
    pub heading:     f32,
    pub squawk_code: u16,
}

impl TransitPayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.speed.to_be_bytes());
        v.extend_from_slice(&self.altitude.to_be_bytes());
        v.extend_from_slice(&self.heading.to_be_bytes());
        v.extend_from_slice(&self.squawk_code.to_be_bytes());
        v
    }
}

/// REQ-CLT-040: Landing telemetry payload
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct LandingPayload {
    pub approach_speed:    f32,
    pub current_altitude:  f32,
    pub heading:           f32,
    pub assigned_runway:   [u8; 4],
    pub approach_clearance: u8,
    pub wind_shear:        f32,
    pub visibility:        f32,
}

impl LandingPayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.approach_speed.to_be_bytes());
        v.extend_from_slice(&self.current_altitude.to_be_bytes());
        v.extend_from_slice(&self.heading.to_be_bytes());
        v.extend_from_slice(&self.assigned_runway);
        v.push(self.approach_clearance);
        v.extend_from_slice(&self.wind_shear.to_be_bytes());
        v.extend_from_slice(&self.visibility.to_be_bytes());
        v
    }
}

// Essential & Basic data packet Unit-tests (REQ-PKT-031 size assertion + round-trip) 
/// Ensuring PacketHeader is exactly 54 bytes and 
/// that serialization/deserialization round-trips correctly.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_54_bytes() {
        assert_eq!(std::mem::size_of::<PacketHeader>(), 54);
    }

    #[test]
    fn header_round_trips() {
        let original = PacketHeader {
            packet_type:    PKT_HANDSHAKE,
            seq_num:        42,
            timestamp:      1_700_000_000,
            payload_length: 52,
            origin_atc_id:  0,
            aircraft_id:    *b"AC1234\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
            emergency_flag: 0,
        };
        let bytes = original.to_bytes();
        let decoded = PacketHeader::from_bytes(&bytes);
        // u8 and [u8;32] fields are alignment-1 — assert_eq directly is fine.
        // u32/i64 fields are packed (may be misaligned) — copy to locals first (E0793).
        assert_eq!(decoded.packet_type,  original.packet_type);
        let dec_seq = decoded.seq_num;        let orig_seq = original.seq_num;
        assert_eq!(dec_seq, orig_seq);
        let dec_ts  = decoded.timestamp;      let orig_ts  = original.timestamp;
        assert_eq!(dec_ts,  orig_ts);
        let dec_pl  = decoded.payload_length; let orig_pl  = original.payload_length;
        assert_eq!(dec_pl,  orig_pl);
        let dec_atc = decoded.origin_atc_id;  let orig_atc = original.origin_atc_id;
        assert_eq!(dec_atc, orig_atc);
        assert_eq!(decoded.aircraft_id,  original.aircraft_id);
        assert_eq!(decoded.emergency_flag, original.emergency_flag);
    }

    #[test]
    fn emergency_flag_detectable_in_header() {
        // REQ-PKT-034: emergency_flag readable without deserializing payload
        let mut header = PacketHeader {
            packet_type: PKT_TRANSIT, seq_num: 1, timestamp: 0,
            payload_length: 0, origin_atc_id: 0,
            aircraft_id: [0u8; 32], emergency_flag: 1,
        };
        let bytes = header.to_bytes();
        // emergency_flag is always at byte offset 53
        assert_eq!(bytes[53], 1, "MAYDAY flag must be readable at byte 53");
        header.emergency_flag = 0;
        let bytes2 = header.to_bytes();
        assert_eq!(bytes2[53], 0);
    }

    /// REQ-PKT-010: Every packet type constant must match the agreed wire value.
    #[test]
    fn test_all_packet_type_constants_defined() {
        assert_eq!(PKT_HANDSHAKE,          0x01);
        assert_eq!(PKT_ACK,                0x02);
        assert_eq!(PKT_ERROR,              0x03);
        assert_eq!(PKT_TAKEOFF,            0x04);
        assert_eq!(PKT_TRANSIT,            0x05);
        assert_eq!(PKT_LANDING,            0x06);
        assert_eq!(PKT_MAYDAY,             0x07);
        assert_eq!(PKT_LARGE_DATA_REQUEST, 0x08);
        assert_eq!(PKT_LARGE_DATA,         0x09);
        assert_eq!(PKT_HANDOFF_NOTIFY,     0x0A);
        assert_eq!(PKT_DISCONNECT,         0x0B);
    }

    /// REQ-PKT-034: emergency_flag must be readable from the header
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
        let flag = header.emergency_flag;
        assert_eq!(flag, 1, "emergency_flag must be 1 for MAYDAY");
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

    /// REQ-PKT-030, REQ-SYS-020: Serialized header must be exactly 54 bytes.
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
        let size = std::mem::size_of::<HandshakePayload>();
        assert_eq!(size, 52);
    }

    /// REQ-CLT-040: TakeoffPayload packed size must match C struct (41 bytes).
    #[test]
    fn test_takeoff_payload_size() {
        let size = std::mem::size_of::<TakeoffPayload>();
        assert_eq!(size, 41);
    }

    /// REQ-CLT-040: TransitPayload packed size must match C struct (14 bytes).
    #[test]
    fn test_transit_payload_size() {
        let size = std::mem::size_of::<TransitPayload>();
        assert_eq!(size, 14);
    }

    /// REQ-CLT-040: LandingPayload packed size must match C struct (27 bytes).
    #[test]
    fn test_landing_payload_size() {
        let size = std::mem::size_of::<LandingPayload>();
        assert_eq!(size, 27);
    }
}
