#include "../../include/unity.h"
#include "../../src/include/state_machine.h"
#include "../../src/include/logger.h"

void test_logger_creates_file(void) {
    int result = logger_init();
    TEST_ASSERT_EQUAL_INT(0, result);
    TEST_ASSERT_NOT_NULL(g_log_file);
    logger_close();
}

void test_log_info_does_not_crash(void) {
    logger_init();
    log_info("unittest info");
    TEST_ASSERT_NOT_NULL(g_log_file);
    logger_close();
}

void test_log_error_increments_counter(void) {
    logger_init();
    log_error("unittest error");
    TEST_ASSERT_EQUAL_INT(1, g_errors);
    logger_close();
}

void test_log_state_transition_increments_counter(void) {
    logger_init();
    log_state_transition(STATE_IDLE, STATE_HANDSHAKE, "client connected");
    TEST_ASSERT_EQUAL_INT(1, g_state_trans);
    logger_close();
}

void test_log_packet_tx_increments_counter(void) {
    logger_init();
    log_packet("TO", "HANDSHAKE", 1, 53, 0, "unit test packet");
    TEST_ASSERT_EQUAL_INT(1, g_packets_tx);
    logger_close();
}

void test_mayday_prefix_path_increments_rx(void) {
    logger_init();
    log_packet("FROM", "MAYDAY", 5, 0, 1, "emergency");
    TEST_ASSERT_EQUAL_INT(1, g_packets_rx);
    logger_close();
}

void test_logger_close_sets_file_null(void) {
    logger_init();
    logger_close();
    TEST_ASSERT_NULL(g_log_file);
}

int main(void) {
    UNITY_BEGIN("server/unittests/logger");
    RUN_TEST(test_logger_creates_file);
    RUN_TEST(test_log_info_does_not_crash);
    RUN_TEST(test_log_error_increments_counter);
    RUN_TEST(test_log_state_transition_increments_counter);
    RUN_TEST(test_log_packet_tx_increments_counter);
    RUN_TEST(test_mayday_prefix_path_increments_rx);
    RUN_TEST(test_logger_close_sets_file_null);
    return UNITY_END();
}
