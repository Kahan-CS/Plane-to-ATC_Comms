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
pub const PKT_HANDSHAKE: u8 = 0x01;
pub const PKT_ACK: u8 = 0x02;
pub const PKT_ERROR: u8 = 0x03;
pub const PKT_TAKEOFF: u8 = 0x04;
pub const PKT_TRANSIT: u8 = 0x05;
pub const PKT_LANDING: u8 = 0x06;
pub const PKT_MAYDAY: u8 = 0x07;
pub const PKT_LARGE_DATA_REQUEST: u8 = 0x08;
pub const PKT_LARGE_DATA: u8 = 0x09;
pub const PKT_HANDOFF_NOTIFY: u8 = 0x0A;
pub const PKT_DISCONNECT: u8 = 0x0B;

pub const HEADER_SIZE: usize = 54;

// Fixed 54-byte header (REQ-PKT-031)
/// Binary layout matches PacketHeader in shared/packet.h exactly.
/// Fields are in network byte order: call `.to_be_bytes()` when serializing.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PacketHeader {
    pub packet_type: u8,       //  1 byte
    pub seq_num: u32,          //  4 bytes : big-endian on wire
    pub timestamp: i64,        //  8 bytes : Unix epoch UTC, big-endian
    pub payload_length: u32,   //  4 bytes : big-endian
    pub origin_atc_id: u32,    //  4 bytes : 0=live, non-zero=buffered handoff
    pub aircraft_id: [u8; 32], // 32 bytes : null-padded UTF-8 callsign
    pub emergency_flag: u8,    //  1 byte  : 0=normal, 1=MAYDAY
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
            packet_type: buf[0],
            seq_num: u32::from_be_bytes(buf[1..5].try_into().unwrap()),
            timestamp: i64::from_be_bytes(buf[5..13].try_into().unwrap()),
            payload_length: u32::from_be_bytes(buf[13..17].try_into().unwrap()),
            origin_atc_id: u32::from_be_bytes(buf[17..21].try_into().unwrap()),
            aircraft_id,
            emergency_flag: buf[53],
        }
    }
}

//  Full packet with heap-allocated payload (REQ-PKT-033, REQ-SYS-030)
#[derive(Debug)]
pub struct Packet {
    pub header: PacketHeader,
    pub payload: Vec<u8>, // size == header.payload_length
}

//  Payload structs (serialized into payload bytes)

/// REQ-PKT-061: Handshake payload (53 bytes)
/// initial_phase tells the server which state to enter after verification.
/// 0 = TAKEOFF (default), 2 = TRANSIT, 3 = LANDING.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct HandshakePayload {
    pub callsign: [u8; 12],
    pub aircraft_type: [u8; 16],
    pub aircraft_model: [u8; 16],
    pub origin: [u8; 4],
    pub destination: [u8; 4],
    pub initial_phase: u8,
}

pub const PHASE_TAKEOFF: u8 = 0;
pub const PHASE_TRANSIT: u8 = 2;
pub const PHASE_LANDING: u8 = 3;

impl HandshakePayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(53);
        v.extend_from_slice(&self.callsign);
        v.extend_from_slice(&self.aircraft_type);
        v.extend_from_slice(&self.aircraft_model);
        v.extend_from_slice(&self.origin);
        v.extend_from_slice(&self.destination);
        v.push(self.initial_phase);
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
    pub departure_time: i64,    // Unix epoch
    pub clearance_type: u8,     // 0=IFR, 1=VFR
    pub assigned_heading: f32,  // degrees
    pub assigned_altitude: f32, // feet MSL
    pub squawk_code: u16,
    pub wind_speed: f32,         // knots
    pub wind_direction: f32,     // degrees
    pub speed_off_runway: f32,   // knots
    pub initial_climb_rate: f32, // feet/min
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
    pub speed: f32,
    pub altitude: f32,
    pub heading: f32,
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
    pub approach_speed: f32,
    pub current_altitude: f32,
    pub heading: f32,
    pub assigned_runway: [u8; 4],
    pub approach_clearance: u8,
    pub wind_shear: f32,
    pub visibility: f32,
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

    // CLT-001
    // REQ-PKT-031
    // DO-178C DAL-D: packed struct must have no hidden
    //   compiler padding — size must be deterministic
    // Verifies PacketHeader is exactly 54 bytes
    // size_of::<PacketHeader>() equals 54
    #[test]
    fn header_is_54_bytes() {
        assert_eq!(std::mem::size_of::<PacketHeader>(), 54);
    }

    // CLT-006
    // REQ-PKT-030, REQ-SYS-020
    // DO-178C DAL-D: serialization round-trip must
    //   produce identical values — no data corruption
    // Verifies to_bytes then from_bytes restores all fields
    // All field values match after round-trip
    #[test]
    fn header_round_trips() {
        let original = PacketHeader {
            packet_type: PKT_HANDSHAKE,
            seq_num: 42,
            timestamp: 1_700_000_000,
            payload_length: 52,
            origin_atc_id: 0,
            aircraft_id: *b"AC1234\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
            emergency_flag: 0,
        };
        let bytes = original.to_bytes();
        let decoded = PacketHeader::from_bytes(&bytes);
        
        // Copy packed struct fields to local variables to avoid alignment issues
        let orig_type = original.packet_type;
        let orig_seq = original.seq_num;
        let orig_ts = original.timestamp;
        let orig_len = original.payload_length;
        let orig_atc = original.origin_atc_id;
        let orig_id = original.aircraft_id;
        let orig_flag = original.emergency_flag;
        
        let dec_type = decoded.packet_type;
        let dec_seq = decoded.seq_num;
        let dec_ts = decoded.timestamp;
        let dec_len = decoded.payload_length;
        let dec_atc = decoded.origin_atc_id;
        let dec_id = decoded.aircraft_id;
        let dec_flag = decoded.emergency_flag;
        
        assert_eq!(dec_type, orig_type);
        assert_eq!(dec_seq, orig_seq);
        assert_eq!(dec_ts, orig_ts);
        assert_eq!(dec_len, orig_len);
        assert_eq!(dec_atc, orig_atc);
        assert_eq!(dec_id, orig_id);
        assert_eq!(dec_flag, orig_flag);
    }

    // CLT-003
    // REQ-PKT-034
    // CARs SOR/96-433 Part V: MAYDAY flag must be
    //   readable before payload deserialization
    // Verifies emergency_flag is at byte offset 53
    // bytes[53] equals 1 when flag is set
    #[test]
    fn emergency_flag_detectable_in_header() {
        // REQ-PKT-034: emergency_flag readable without deserializing payload
        let mut header = PacketHeader {
            packet_type: PKT_TRANSIT,
            seq_num: 1,
            timestamp: 0,
            payload_length: 0,
            origin_atc_id: 0,
            aircraft_id: [0u8; 32],
            emergency_flag: 1,
        };
        let bytes = header.to_bytes();
        // emergency_flag is always at byte offset 53
        assert_eq!(bytes[53], 1, "MAYDAY flag must be readable at byte 53");
        header.emergency_flag = 0;
        let bytes2 = header.to_bytes();
        assert_eq!(bytes2[53], 0);
    }

    // CLT-002
    // REQ-PKT-010
    // DO-178C DAL-D: all packet type identifiers must
    //   match agreed wire values for interoperability
    // Verifies all 11 constants match shared/packet.h
    // All 11 assert_eq comparisons succeed
    #[test]
    fn test_clt002_all_packet_type_constants() {
        assert_eq!(PKT_HANDSHAKE,          0x01u8);
        assert_eq!(PKT_ACK,                0x02u8);
        assert_eq!(PKT_ERROR,              0x03u8);
        assert_eq!(PKT_TAKEOFF,            0x04u8);
        assert_eq!(PKT_TRANSIT,            0x05u8);
        assert_eq!(PKT_LANDING,            0x06u8);
        assert_eq!(PKT_MAYDAY,             0x07u8);
        assert_eq!(PKT_LARGE_DATA_REQUEST, 0x08u8);
        assert_eq!(PKT_LARGE_DATA,         0x09u8);
        assert_eq!(PKT_HANDOFF_NOTIFY,     0x0Au8);
        assert_eq!(PKT_DISCONNECT,         0x0Bu8);
    }

    // CLT-004
    // REQ-PKT-020
    // CARs SOR/96-433 Part V: buffered handoff packets
    //   must carry non-zero ATC origin identifier
    // Verifies origin_atc_id is non-zero for buffered packet
    // Copied origin_atc_id is non-zero and equals 1001
    #[test]
    fn test_clt004_buffered_packet_nonzero_atc_id() {
        let header = PacketHeader {
            packet_type:    PKT_TRANSIT,
            seq_num:        5,
            timestamp:      0,
            payload_length: 0,
            origin_atc_id:  1001,
            aircraft_id:    [0u8; 32],
            emergency_flag: 0,
        };
        let atc_id = header.origin_atc_id;
        assert_ne!(atc_id, 0,
            "buffered packet must have non-zero origin_atc_id");
        assert_eq!(atc_id, 1001,
            "origin_atc_id must equal the value set");
    }

    // CLT-005
    // REQ-PKT-020
    // CARs SOR/96-433 Part V: live packets must be
    //   distinguishable from buffered handoff packets
    // Verifies live packet has origin_atc_id of zero
    // Copied origin_atc_id equals zero
    #[test]
    fn test_clt005_live_packet_zero_atc_id() {
        let header = PacketHeader {
            packet_type:    PKT_TRANSIT,
            seq_num:        1,
            timestamp:      0,
            payload_length: 0,
            origin_atc_id:  0,
            aircraft_id:    [0u8; 32],
            emergency_flag: 0,
        };
        let atc_id = header.origin_atc_id;
        assert_eq!(atc_id, 0,
            "live packet must have origin_atc_id == 0");
    }

    // CLT-007
    // REQ-PKT-060, REQ-PKT-061
    // DO-178C DAL-D: payload structs must match C server
    //   exactly for binary compatibility
    // Verifies HandshakePayload is exactly 53 bytes
    // size_of::<HandshakePayload>() equals 53
    #[test]
    fn test_clt007_handshake_payload_53_bytes() {
        let size = std::mem::size_of::<HandshakePayload>();
        assert_eq!(size, 53,
            "HandshakePayload must be 53 bytes to match C server");
    }

    // CLT-008
    // REQ-CLT-040, REQ-PKT-060
    // DO-178C DAL-D: takeoff telemetry struct must be
    //   binary-compatible with C server struct
    // Verifies TakeoffPayload is exactly 35 bytes
    // size_of::<TakeoffPayload>() equals 35
    #[test]
    fn test_clt008_takeoff_payload_35_bytes() {
        let size = std::mem::size_of::<TakeoffPayload>();
        assert_eq!(size, 35,
            "TakeoffPayload must be 35 bytes — confirmed \
             consistent with C server after comment correction");
    }

    // CLT-009
    // REQ-CLT-040, REQ-PKT-060
    // DO-178C DAL-D: transit telemetry struct must be
    //   binary-compatible with C server struct
    // Verifies TransitPayload is exactly 14 bytes
    // size_of::<TransitPayload>() equals 14
    #[test]
    fn test_clt009_transit_payload_14_bytes() {
        let size = std::mem::size_of::<TransitPayload>();
        assert_eq!(size, 14,
            "TransitPayload must be 14 bytes to match C server");
    }

    // CLT-010
    // REQ-CLT-040, REQ-PKT-060
    // DO-178C DAL-D: landing telemetry struct must be
    //   binary-compatible with C server struct
    // Verifies LandingPayload is exactly 25 bytes
    // size_of::<LandingPayload>() equals 25
    #[test]
    fn test_clt010_landing_payload_25_bytes() {
        let size = std::mem::size_of::<LandingPayload>();
        assert_eq!(size, 25,
            "LandingPayload must be 25 bytes — confirmed \
             consistent with C server after comment correction");
    }

    // CLT-019
    // REQ-SYS-030, REQ-PKT-033
    // DO-178C DAL-D: dynamic allocation must be driven
    //   by header field value deterministically
    // Verifies payload Vec size matches payload_length
    // payload.len() equals header.payload_length
    #[test]
    fn test_clt019_dynamic_payload_matches_header() {
        let payload_data = vec![0xABu8; 52];
        let header = PacketHeader {
            packet_type:    PKT_HANDSHAKE,
            seq_num:        1,
            timestamp:      0,
            payload_length: 52,
            origin_atc_id:  0,
            aircraft_id:    [0u8; 32],
            emergency_flag: 0,
        };
        let pkt = Packet {
            header,
            payload: payload_data,
        };
        let plen = pkt.header.payload_length;
        assert_eq!(pkt.payload.len(), plen as usize,
            "payload Vec size must match header.payload_length");
    }

    // CLT-020
    // REQ-PKT-062, REQ-CLT-050
    // CARs SOR/96-433 Part V: MAYDAY distress signal must be constructable and correctly encoded in any active flight phase
    // Verifies building a MAYDAY packet correctly sets packet_type to PKT_MAYDAY and emergency_flag to 1
    // Both field values confirmed via local variable copy
    #[test]
    fn test_clt020_mayday_packet_sets_emergency_flag() {
        let header = PacketHeader {
            packet_type:    PKT_MAYDAY,
            seq_num:        1,
            timestamp:      0,
            payload_length: 0,
            origin_atc_id:  0,
            aircraft_id:    [0u8; 32],
            emergency_flag: 1,
        };
        let ptype = header.packet_type;
        let flag  = header.emergency_flag;
        assert_eq!(ptype, PKT_MAYDAY,
            "packet_type must be PKT_MAYDAY");
        assert_eq!(flag, 1,
            "emergency_flag must be 1 for MAYDAY");
        assert_ne!(flag, 0,
            "emergency_flag must be non-zero for MAYDAY");
    }

    // CLT-021
    // REQ-PKT-061, REQ-CLT-030
    // DO-178C DAL-D: aircraft identification data must\ be correctly serialized into handshake payload with no field corruption or offset error
    // Verifies HandshakePayload::to_bytes() produces
    //   53 bytes, callsign appears at offset 0,
    //   and initial_phase is serialized as the final byte
    // bytes.len() == 53 and first 6 bytes match callsign
    #[test]
    fn test_clt021_handshake_payload_serializes_callsign() {
        let mut p = HandshakePayload::default();
        let callsign = b"AC1234";
        p.callsign[..callsign.len()].copy_from_slice(callsign);
        p.initial_phase = PHASE_TRANSIT;
        let bytes = p.to_bytes();
        assert_eq!(bytes.len(), 53,
            "HandshakePayload::to_bytes must produce 53 bytes");
        assert_eq!(&bytes[0..6], callsign,
            "callsign bytes must appear at offset 0 in payload");
        assert_eq!(bytes[52], PHASE_TRANSIT,
            "initial_phase must be serialized as the last byte");
    }

    // CLT-022
    // REQ-CLT-040
    // DO-178C DAL-D: serialized takeoff telemetry must produce the exact byte count expected by the C server, size mismatch causes misread telemetry
    // Verifies TakeoffPayload::to_bytes() output is exactly 35 bytes tests serialization not just
    //   compile-time struct size
    // bytes.len() equals 35
    #[test]
    fn test_clt022_takeoff_payload_serializes_35_bytes() {
        let p = TakeoffPayload {
            departure_time:     0,
            clearance_type:     0,
            assigned_heading:   0.0,
            assigned_altitude:  0.0,
            squawk_code:        0,
            wind_speed:         0.0,
            wind_direction:     0.0,
            speed_off_runway:   0.0,
            initial_climb_rate: 0.0,
        };
        let bytes = p.to_bytes();
        assert_eq!(bytes.len(), 35,
            "TakeoffPayload::to_bytes must produce 35 bytes");
    }
}
