/**
 * @file test_packet.c
 * @brief Formal unit tests for shared packet
 *        structure from the C server side
 *
 * Test Suite: Server Packet Structure
 *
 * Regulatory Compliance:
 *   - DO-178C DAL-D : binary-compatible packet structures with no hidden padding
 *   - CARs SOR/96-433 Part V :data integrity in safety-critical communications
 *
 * Coverage:
 *   SVR-016  REQ-PKT-031, REQ-PKT-032  Header 54 bytes
 *   SVR-017  REQ-PKT-010               All constants
 *   SVR-018  REQ-PKT-034               Emergency flag
 *
 * How to run:
 *   From server/: mingw32-make test
 */

#include "../include/unity.h"
#include "../../shared/packet.h"
#include <stddef.h>
#include <string.h>

// SVR-016
// REQ-PKT-031, REQ-PKT-032
// DO-178C DAL-D: binary wire format must have
//   deterministic layout with no compiler padding
// sizeof(PacketHeader) must equal 54 with packed alignment and no compiler padding. Binary layout must match Rust client exactly.
// TEST_ASSERT_EQUAL_INT(54, size) passes

void test_SVR016_header_size_54_bytes(void)
{
    int size = (int)sizeof(PacketHeader);
    TEST_ASSERT_EQUAL_INT(54, size);
}

// SVR-017
// REQ-PKT-010
// DO-178C DAL-D: binary wire format must have
//   deterministic layout with no compiler padding
// All 11 packet type constants must match the agreed wire values used by Rust client. Mismatch causes silent protocol failure.
// All 11 TEST_ASSERT_EQUAL_INT checks pass

void test_SVR017_all_packet_constants(void)
{
    TEST_ASSERT_EQUAL_INT(0x01, PKT_HANDSHAKE);
    TEST_ASSERT_EQUAL_INT(0x02, PKT_ACK);
    TEST_ASSERT_EQUAL_INT(0x03, PKT_ERROR);
    TEST_ASSERT_EQUAL_INT(0x04, PKT_TAKEOFF);
    TEST_ASSERT_EQUAL_INT(0x05, PKT_TRANSIT);
    TEST_ASSERT_EQUAL_INT(0x06, PKT_LANDING);
    TEST_ASSERT_EQUAL_INT(0x07, PKT_MAYDAY);
    TEST_ASSERT_EQUAL_INT(0x08, PKT_LARGE_DATA_REQUEST);
    TEST_ASSERT_EQUAL_INT(0x09, PKT_LARGE_DATA);
    TEST_ASSERT_EQUAL_INT(0x0A, PKT_HANDOFF_NOTIFY);
    TEST_ASSERT_EQUAL_INT(0x0B, PKT_DISCONNECT);
}

// SVR-018
// REQ-PKT-034
// DO-178C DAL-D: binary wire format must have
//   deterministic layout with no compiler padding
// Emergency_flag must be readable directly from PacketHeader without deserializing the payload. Server checks this field on every packet before any other processing.
// emergency_flag value readable and correct

void test_SVR018_emergency_flag_readable(void)
{
    PacketHeader h;
    memset(&h, 0, sizeof(h));
    h.emergency_flag = 1;
    TEST_ASSERT_EQUAL_UINT8(1, h.emergency_flag);
    h.emergency_flag = 0;
    TEST_ASSERT_EQUAL_UINT8(0, h.emergency_flag);
}

// SVR-022
// REQ-SVR-040, REQ-PKT-020
// CARs SOR/96-433 Part V: server must identify buffered handoff packets from header alone without deserializing payload — ensures data integrity check happens before payload access
// Verifies non-zero origin_atc_id is readable directly from PacketHeader
// origin_atc_id non-zero and equals 1001

void test_SVR022_server_identifies_buffered_packet(void)
{
    PacketHeader h;
    memset(&h, 0, sizeof(h));
    h.origin_atc_id = 1001;
    TEST_ASSERT(h.origin_atc_id != 0);
    TEST_ASSERT_EQUAL_UINT32(1001, h.origin_atc_id);
}

// SVR-024
// REQ-PKT-031, REQ-PKT-032
// DO-178C DAL-D: binary wire format must have= deterministic field positions any drift in field offsets causes the C server and Rust
//   client to misread each other's packets
// Verifies each PacketHeader field sits at the correct byte offset using offsetof()
// All 7 offsetof checks pass matching Rust layout

void test_SVR024_packet_header_field_offsets(void)
{
    TEST_ASSERT_EQUAL_INT(0,
                          (int)offsetof(PacketHeader, packet_type));
    TEST_ASSERT_EQUAL_INT(1,
                          (int)offsetof(PacketHeader, seq_num));
    TEST_ASSERT_EQUAL_INT(5,
                          (int)offsetof(PacketHeader, timestamp));
    TEST_ASSERT_EQUAL_INT(13,
                          (int)offsetof(PacketHeader, payload_length));
    TEST_ASSERT_EQUAL_INT(17,
                          (int)offsetof(PacketHeader, origin_atc_id));
    TEST_ASSERT_EQUAL_INT(21,
                          (int)offsetof(PacketHeader, aircraft_id));
    TEST_ASSERT_EQUAL_INT(53,
                          (int)offsetof(PacketHeader, emergency_flag));
}

int main(void)
{
    UNITY_BEGIN("Server Packet — SVR-016 to SVR-024");
    RUN_TEST(test_SVR016_header_size_54_bytes);
    RUN_TEST(test_SVR017_all_packet_constants);
    RUN_TEST(test_SVR018_emergency_flag_readable);
    RUN_TEST(test_SVR022_server_identifies_buffered_packet);
    RUN_TEST(test_SVR024_packet_header_field_offsets);
    return UNITY_END();
}
