use crate::packet::{
    HandshakePayload, LandingPayload, Packet, PacketHeader, TakeoffPayload, TransitPayload,
    PKT_LANDING, PKT_TAKEOFF, PKT_TRANSIT,
};
/// phases.rs - Flight phase telemetry input, validation, and packet construction
///
/// Each public function prompts the user for phase-specific fields,
/// validates ranges against the field's binary type bounds, builds
/// the payload struct, and returns a ready-to-send Packet.
///
/// Field ranges are constrained by:
///   - Physical plausibility for the aviation domain
///   - The binary type size (e.g. f32 max, u16 max for squawk 0-7777 octal)
///
/// Regulatory compliance:
///   - CARs SOR/96-433 Part V - data integrity in safety communications
///   - DO-178C DAL-D - input validation, no silent truncation
///
/// REQ-CLT-020, REQ-CLT-040, REQ-PKT-060
use std::io::{self, Write};

// Input validation helpers

/// Prompt until they enter an f32 within [min, max].
fn prompt_f32(label: &str, min: f32, max: f32) -> f32 {
    loop {
        print!("  {} ({:.0} - {:.0}): ", label, min, max);
        io::stdout().flush().ok();
        let mut s = String::new();
        io::stdin().read_line(&mut s).ok();
        match s.trim().parse::<f32>() {
            Ok(v) if v >= min && v <= max => return v,
            _ => println!(
                " [!] Invalid - enter a number between {:.0} and {:.0}",
                min, max
            ),
        }
    }
}

/// Prompt until user enters a u16 within [min, max].
fn prompt_u16(label: &str, min: u16, max: u16) -> u16 {
    loop {
        print!("  {} ({} - {}): ", label, min, max);
        io::stdout().flush().ok();
        let mut s = String::new();
        io::stdin().read_line(&mut s).ok();
        match s.trim().parse::<u16>() {
            Ok(v) if v >= min && v <= max => return v,
            _ => println!(
                "    [!] Invalid - enter a number between {} and {}",
                min, max
            ),
        }
    }
}

/// Prompt until user enters 0 or 1.
fn prompt_bool(label: &str, false_label: &str, true_label: &str) -> u8 {
    loop {
        print!("  {} (0={}, 1={}): ", label, false_label, true_label);
        io::stdout().flush().ok();
        let mut s = String::new();
        io::stdin().read_line(&mut s).ok();
        match s.trim() {
            "0" => return 0,
            "1" => return 1,
            _ => println!("   [!] Enter 0 or 1"),
        }
    }
}

/// Prompt for a runway string like "27L", max 3 chars + null.
fn prompt_runway() -> [u8; 4] {
    loop {
        print!("  Assigned runway (e.g. 27L, 05R, 09): ");
        io::stdout().flush().ok();
        let mut s = String::new();
        io::stdin().read_line(&mut s).ok();
        let trimmed = s.trim();
        if trimmed.len() >= 1 && trimmed.len() <= 3 {
            return HandshakePayload::str_to_fixed(trimmed);
        }
        println!("  [!] Runway must be 1-3 characters (e.g. 27L)");
    }
}

// Phase builders

/// Collect Takeoff telemetry from the user and build a PKT_TAKEOFF packet.
/// REQ-CLT-040 - Takeoff fields: departure_time, clearance_type, heading,
///     altitude, squawk, wind_speed, wind_direction, speed_off_runway, climb_rate
pub fn build_takeoff_packet(seq: u32, callsign: &str, timestamp: i64) -> Packet {
    println!("\n[Takeoff] Enter telemetry:");

    let clearance_type = prompt_bool("Clearance type", "IFR", "VFR");
    let assigned_heading = prompt_f32("Assigned heading (deg)", 0.0, 360.0);
    let assigned_altitude = prompt_f32("Assigned altitude (ft MSL)", 0.0, 45_000.0);
    // Squawk: octal 0000-7777 = decimal 0-4095
    let squawk_code = prompt_u16("Squawk code (octal 0000-7777, enter decimal)", 0, 4095);
    let wind_speed = prompt_f32("Wind speed (kts)", 0.0, 250.0);
    let wind_direction = prompt_f32("Wind direction (deg)", 0.0, 360.0);
    let speed_off_runway = prompt_f32("Speed off runway (kts)", 0.0, 250.0);
    let initial_climb_rate = prompt_f32("Initial climb rate (fpm)", 0.0, 6_000.0);

    let payload_struct = TakeoffPayload {
        departure_time: timestamp,
        clearance_type,
        assigned_heading,
        assigned_altitude,
        squawk_code,
        wind_speed,
        wind_direction,
        speed_off_runway,
        initial_climb_rate,
    };
    let payload = payload_struct.to_bytes();
    let plen = payload.len() as u32;

    Packet {
        header: PacketHeader {
            packet_type: PKT_TAKEOFF,
            seq_num: seq,
            timestamp,
            payload_length: plen,
            origin_atc_id: 0,
            aircraft_id: HandshakePayload::str_to_fixed(callsign),
            emergency_flag: 0,
        },
        payload,
    }
}

/// Collect Transit telemetry and build a PKT_TRANSIT packet.
/// REQ-CLT-040 - Transit fields: speed, altitude, heading, squawk
pub fn build_transit_packet(seq: u32, callsign: &str, timestamp: i64) -> Packet {
    println!("\n[Transit] Enter telemetry:");

    let speed = prompt_f32("Speed (kts)", 0.0, 650.0);
    let altitude = prompt_f32("Altitude (ft MSL)", 0.0, 45_000.0);
    let heading = prompt_f32("Heading (deg)", 0.0, 360.0);
    let squawk_code = prompt_u16("Squawk code (decimal 0-4095)", 0, 4095);

    let payload_struct = TransitPayload {
        speed,
        altitude,
        heading,
        squawk_code,
    };
    let payload = payload_struct.to_bytes();
    let plen = payload.len() as u32;

    Packet {
        header: PacketHeader {
            packet_type: PKT_TRANSIT,
            seq_num: seq,
            timestamp,
            payload_length: plen,
            origin_atc_id: 0,
            aircraft_id: HandshakePayload::str_to_fixed(callsign),
            emergency_flag: 0,
        },
        payload,
    }
}

/// Collect Landing telemetry and build a PKT_LANDING packet.
/// REQ-CLT-040 - Landing fields: approach_speed, altitude, heading,
///   runway, approach_clearance, wind_shear, visibility
pub fn build_landing_packet(seq: u32, callsign: &str, timestamp: i64) -> Packet {
    println!("\n[Landing] Enter telemetry:");

    let approach_speed = prompt_f32("Approach speed (kts)", 0.0, 250.0);
    let current_altitude = prompt_f32("Current altitude (ft MSL)", 0.0, 45_000.0);
    let heading = prompt_f32("Heading (deg)", 0.0, 360.0);
    let assigned_runway = prompt_runway();
    let approach_clearance = prompt_bool("Approach clearance", "NOT cleared", "Cleared to land");
    let wind_shear = prompt_f32("Wind shear (kts)", 0.0, 100.0);
    let visibility = prompt_f32("Visibility (statute miles)", 0.0, 50.0);

    let payload_struct = LandingPayload {
        approach_speed,
        current_altitude,
        heading,
        assigned_runway,
        approach_clearance,
        wind_shear,
        visibility,
    };
    let payload = payload_struct.to_bytes();
    let plen = payload.len() as u32;

    Packet {
        header: PacketHeader {
            packet_type: PKT_LANDING,
            seq_num: seq,
            timestamp,
            payload_length: plen,
            origin_atc_id: 0,
            aircraft_id: HandshakePayload::str_to_fixed(callsign),
            emergency_flag: 0,
        },
        payload,
    }
}
