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
//  REQ-PKT-031, REQ-PKT-032
// sizeof(PacketHeader) must equal 54 with packed alignment and no compiler padding.
//  Binary layout must match Rust client exactly.
//  TEST_ASSERT_EQUAL_INT(54, size) passes

void test_SVR016_header_size_54_bytes(void)
{
    int size = (int)sizeof(PacketHeader);
    TEST_ASSERT_EQUAL_INT(54, size);
}

// SVR-017
// REQ-PKT-010
//  All 11 packet type constants must match the agreed wire values used by Rust client.
// Mismatch causes silent protocol failure.
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

int main(void)
{
    UNITY_BEGIN("Server Packet — SVR-016 to SVR-018");
    RUN_TEST(test_SVR016_header_size_54_bytes);
    RUN_TEST(test_SVR017_all_packet_constants);
    RUN_TEST(test_SVR018_emergency_flag_readable);
    return UNITY_END();
}
