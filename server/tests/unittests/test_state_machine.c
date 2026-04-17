#include "../../include/unity.h"
#include "../../src/include/state_machine.h"

void test_idle_to_handshake_valid(void) {
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_IDLE, STATE_HANDSHAKE));
}

void test_handshake_to_takeoff_valid(void) {
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_HANDSHAKE, STATE_TAKEOFF));
}

void test_handshake_to_transit_valid_for_reconnect(void) {
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_HANDSHAKE, STATE_TRANSIT));
}

void test_handshake_to_landing_valid_for_reconnect(void) {
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_HANDSHAKE, STATE_LANDING));
}

void test_takeoff_to_transit_valid(void) {
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_TAKEOFF, STATE_TRANSIT));
}

void test_transit_to_landing_valid(void) {
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_TRANSIT, STATE_LANDING));
}

void test_landing_to_disconnected_valid(void) {
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_LANDING, STATE_DISCONNECTED));
}

void test_disconnected_to_idle_valid(void) {
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_DISCONNECTED, STATE_IDLE));
}

void test_mayday_reachable_from_active_phases(void) {
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_TAKEOFF, STATE_MAYDAY));
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_TRANSIT, STATE_MAYDAY));
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_LANDING, STATE_MAYDAY));
}

void test_skip_state_rejected(void) {
    TEST_ASSERT_EQUAL_INT(0, is_valid_transition(STATE_IDLE, STATE_TAKEOFF));
}

void test_backwards_transition_rejected(void) {
    TEST_ASSERT_EQUAL_INT(0, is_valid_transition(STATE_TRANSIT, STATE_HANDSHAKE));
}

void test_state_to_str_all_states(void) {
    TEST_ASSERT_EQUAL_STRING("IDLE", state_to_str(STATE_IDLE));
    TEST_ASSERT_EQUAL_STRING("HANDSHAKE", state_to_str(STATE_HANDSHAKE));
    TEST_ASSERT_EQUAL_STRING("TAKEOFF", state_to_str(STATE_TAKEOFF));
    TEST_ASSERT_EQUAL_STRING("TRANSIT", state_to_str(STATE_TRANSIT));
    TEST_ASSERT_EQUAL_STRING("LANDING", state_to_str(STATE_LANDING));
    TEST_ASSERT_EQUAL_STRING("MAYDAY", state_to_str(STATE_MAYDAY));
    TEST_ASSERT_EQUAL_STRING("DISCONNECTED", state_to_str(STATE_DISCONNECTED));
}

int main(void) {
    UNITY_BEGIN("server/unittests/state_machine");
    RUN_TEST(test_idle_to_handshake_valid);
    RUN_TEST(test_handshake_to_takeoff_valid);
    RUN_TEST(test_handshake_to_transit_valid_for_reconnect);
    RUN_TEST(test_handshake_to_landing_valid_for_reconnect);
    RUN_TEST(test_takeoff_to_transit_valid);
    RUN_TEST(test_transit_to_landing_valid);
    RUN_TEST(test_landing_to_disconnected_valid);
    RUN_TEST(test_disconnected_to_idle_valid);
    RUN_TEST(test_mayday_reachable_from_active_phases);
    RUN_TEST(test_skip_state_rejected);
    RUN_TEST(test_backwards_transition_rejected);
    RUN_TEST(test_state_to_str_all_states);
    return UNITY_END();
}
