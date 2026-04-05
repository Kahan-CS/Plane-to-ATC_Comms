/**
 * @file test_state_machine.c
 * @brief Formal unit tests for ATCState state machine
 *
 * Test Suite: Server State Machine
 * GitHub Issue: #14

 *
 * Regulatory Compliance:
 *   - CARs SOR/96-433 Part V (Airworthiness): deterministic state behaviour in safety-critical communications software
 *   - DO-178C DAL-D : traceable unit-level verification of every defined state and permitted transition
 *
 * Coverage:
 *   SVR-001  REQ-STM-010  Initial state is IDLE
 *   SVR-002  REQ-STM-020  IDLE -> HANDSHAKE valid
 *   SVR-003  REQ-STM-020  HANDSHAKE -> TAKEOFF valid
 *   SVR-004  REQ-STM-020  TAKEOFF -> TRANSIT valid
 *   SVR-005  REQ-STM-020  TRANSIT -> LANDING valid
 *   SVR-006  REQ-STM-020  LANDING -> DISCONNECTED valid
 *   SVR-007  REQ-STM-020  DISCONNECTED -> IDLE valid
 *   SVR-008  REQ-STM-020  Skip state rejected
 *   SVR-009  REQ-STM-020  Backwards transition rejected
 *   SVR-010  REQ-STM-010  state_to_str all 6 states
 *
 * How to run:
 *   From repo root: mingw32-make test
 *   From server/:   mingw32-make test
 */

#include "../include/unity.h"
#include "../src/include/state_machine.h"

/* ── Test Functions ─────────────────────────────────────────────────────────── */

/**
 * @test SVR-001
 * @req  REQ-STM-010
 * @desc Initial ATCState variable must equal STATE_IDLE
 *       before any transition is triggered.
 * @pass TEST_ASSERT_EQUAL_INT confirms value is STATE_IDLE
 */
void test_SVR001_initial_state_is_idle(void)
{
    ATCState state = STATE_IDLE;
    TEST_ASSERT_EQUAL_INT(STATE_IDLE, state);
}

/**
 * @test SVR-002
 * @req  REQ-STM-020
 * @desc IDLE -> HANDSHAKE must be a permitted transition.
 *       This is the first required step when a client connects.
 * @pass is_valid_transition returns 1
 */
void test_SVR002_idle_to_handshake_valid(void)
{
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_IDLE, STATE_HANDSHAKE));
}

/**
 * @test SVR-003
 * @req  REQ-STM-020
 * @desc HANDSHAKE -> TAKEOFF must be a permitted transition.
 *       Occurs after the server verifies the handshake packet.
 * @pass is_valid_transition returns 1
 */
void test_SVR003_handshake_to_takeoff_valid(void)
{
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_HANDSHAKE, STATE_TAKEOFF));
}

/**
 * @test SVR-004
 * @req  REQ-STM-020
 * @desc TAKEOFF -> TRANSIT must be a permitted transition.
 *       Occurs when the aircraft reports airborne telemetry.
 * @pass is_valid_transition returns 1
 */
void test_SVR004_takeoff_to_transit_valid(void)
{
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_TAKEOFF, STATE_TRANSIT));
}

/**
 * @test SVR-005
 * @req  REQ-STM-020
 * @desc TRANSIT -> LANDING must be a permitted transition.
 *       Occurs when the aircraft begins approach phase.
 * @pass is_valid_transition returns 1
 */
void test_SVR005_transit_to_landing_valid(void)
{
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_TRANSIT, STATE_LANDING));
}

/**
 * @test SVR-006
 * @req  REQ-STM-020
 * @desc LANDING -> DISCONNECTED must be a permitted transition.
 *       Occurs when the session ends after landing.
 * @pass is_valid_transition returns 1
 */
void test_SVR006_landing_to_disconnected_valid(void)
{
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_LANDING, STATE_DISCONNECTED));
}

/**
 * @test SVR-007
 * @req  REQ-STM-020
 * @desc DISCONNECTED -> IDLE must be a permitted transition.
 *       Resets the server to accept a new client connection.
 * @pass is_valid_transition returns 1
 */
void test_SVR007_disconnected_to_idle_valid(void)
{
    TEST_ASSERT_EQUAL_INT(1, is_valid_transition(STATE_DISCONNECTED, STATE_IDLE));
}

/**
 * @test SVR-008
 * @req  REQ-STM-020
 * @desc IDLE -> TAKEOFF must be rejected (skips HANDSHAKE).
 *       No state may be skipped; transitions must be sequential.
 * @pass is_valid_transition returns 0
 */
void test_SVR008_skip_state_rejected(void)
{
    TEST_ASSERT_EQUAL_INT(0, is_valid_transition(STATE_IDLE, STATE_TAKEOFF));
}

/**
 * @test SVR-009
 * @req  REQ-STM-020
 * @desc TRANSIT -> HANDSHAKE must be rejected (backwards transition).
 *       The state machine must never move backwards.
 * @pass is_valid_transition returns 0
 */
void test_SVR009_backwards_transition_rejected(void)
{
    TEST_ASSERT_EQUAL_INT(0, is_valid_transition(STATE_TRANSIT, STATE_HANDSHAKE));
}

/**
 * @test SVR-010
 * @req  REQ-STM-010
 * @desc state_to_str must return the correct string label for all
 *       6 defined states. Used in log output and debug messages.
 * @pass TEST_ASSERT_EQUAL_STRING confirms each label matches exactly
 */
void test_SVR010_state_to_str_all_states(void)
{
    TEST_ASSERT_EQUAL_STRING("IDLE", state_to_str(STATE_IDLE));
    TEST_ASSERT_EQUAL_STRING("HANDSHAKE", state_to_str(STATE_HANDSHAKE));
    TEST_ASSERT_EQUAL_STRING("TAKEOFF", state_to_str(STATE_TAKEOFF));
    TEST_ASSERT_EQUAL_STRING("TRANSIT", state_to_str(STATE_TRANSIT));
    TEST_ASSERT_EQUAL_STRING("LANDING", state_to_str(STATE_LANDING));
    TEST_ASSERT_EQUAL_STRING("DISCONNECTED", state_to_str(STATE_DISCONNECTED));
}

/* ── Entry Point ────────────────────────────────────────────────────────────── */

int main(void)
{
    UNITY_BEGIN("Server State Machine — SVR-001 to SVR-010");
    RUN_TEST(test_SVR001_initial_state_is_idle);
    RUN_TEST(test_SVR002_idle_to_handshake_valid);
    RUN_TEST(test_SVR003_handshake_to_takeoff_valid);
    RUN_TEST(test_SVR004_takeoff_to_transit_valid);
    RUN_TEST(test_SVR005_transit_to_landing_valid);
    RUN_TEST(test_SVR006_landing_to_disconnected_valid);
    RUN_TEST(test_SVR007_disconnected_to_idle_valid);
    RUN_TEST(test_SVR008_skip_state_rejected);
    RUN_TEST(test_SVR009_backwards_transition_rejected);
    RUN_TEST(test_SVR010_state_to_str_all_states);
    return UNITY_END();
}
