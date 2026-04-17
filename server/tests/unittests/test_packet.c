#include "../../include/unity.h"
#include "../../../shared/packet.h"
#include <stddef.h>
#include <string.h>

void test_header_size_54_bytes(void) {
    TEST_ASSERT_EQUAL_INT(54, (int)sizeof(PacketHeader));
}

void test_handshake_payload_size_53_bytes(void) {
    TEST_ASSERT_EQUAL_INT(53, (int)sizeof(HandshakePayload));
}

void test_handshake_initial_phase_is_last_byte(void) {
    TEST_ASSERT_EQUAL_INT(52, (int)offsetof(HandshakePayload, initial_phase));
}

void test_all_packet_constants(void) {
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

void test_header_field_offsets(void) {
    TEST_ASSERT_EQUAL_INT(0, (int)offsetof(PacketHeader, packet_type));
    TEST_ASSERT_EQUAL_INT(1, (int)offsetof(PacketHeader, seq_num));
    TEST_ASSERT_EQUAL_INT(5, (int)offsetof(PacketHeader, timestamp));
    TEST_ASSERT_EQUAL_INT(13, (int)offsetof(PacketHeader, payload_length));
    TEST_ASSERT_EQUAL_INT(17, (int)offsetof(PacketHeader, origin_atc_id));
    TEST_ASSERT_EQUAL_INT(21, (int)offsetof(PacketHeader, aircraft_id));
    TEST_ASSERT_EQUAL_INT(53, (int)offsetof(PacketHeader, emergency_flag));
}

void test_emergency_flag_readable(void) {
    PacketHeader h;
    memset(&h, 0, sizeof(h));
    h.emergency_flag = 1;
    TEST_ASSERT_EQUAL_UINT8(1, h.emergency_flag);
    h.emergency_flag = 0;
    TEST_ASSERT_EQUAL_UINT8(0, h.emergency_flag);
}

void test_origin_atc_id_identifies_buffered_packet(void) {
    PacketHeader h;
    memset(&h, 0, sizeof(h));
    h.origin_atc_id = 1001;
    TEST_ASSERT(h.origin_atc_id != 0);
    TEST_ASSERT_EQUAL_UINT32(1001, h.origin_atc_id);
}

int main(void) {
    UNITY_BEGIN("server/unittests/packet");
    RUN_TEST(test_header_size_54_bytes);
    RUN_TEST(test_handshake_payload_size_53_bytes);
    RUN_TEST(test_handshake_initial_phase_is_last_byte);
    RUN_TEST(test_all_packet_constants);
    RUN_TEST(test_header_field_offsets);
    RUN_TEST(test_emergency_flag_readable);
    RUN_TEST(test_origin_atc_id_identifies_buffered_packet);
    return UNITY_END();
}
