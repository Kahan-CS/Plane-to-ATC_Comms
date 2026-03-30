/**
 * @file packet.h
 * @brief Shared ATC Comms packet structure definition.
 *
 * This file defines the binary-compatible packet format used by both the
 * C Server and Rust Client. ALL fields are in network byte order (big-endian).
 * The header is exactly 54 bytes with packed alignment (no padding).
 *
 * Regulatory compliance:
 *   - CARs SOR/96-433 Part V (Airworthiness) - data integrity in safety comms
 *   - DO-178C DAL-D guidance - deterministic data structures, no hidden padding
 *
 * REQ-PKT-030, REQ-PKT-031, REQ-PKT-032
 */

#ifndef PACKET_H
#define PACKET_H

#include <stdint.h>


#endif
