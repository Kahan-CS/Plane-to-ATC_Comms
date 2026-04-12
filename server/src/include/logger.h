/**
 * @file  logger.h
 * @brief TX/RX packet logger with timestamped session files.
 *
 * REQ-SYS-050, REQ-LOG-010 through REQ-LOG-060
 */

#ifndef LOGGER_H
#define LOGGER_H

#include <stdio.h>
#include <string.h>
#include <windows.h>

#include "state_machine.h"

static FILE    *g_log_file    = NULL;

/* REQ-LOG-060: Session counters for shutdown summary. */
static uint32_t g_packets_rx  = 0;   /* total packets received this session  */
static uint32_t g_packets_tx  = 0;   /* total packets transmitted            */
static uint32_t g_errors      = 0;   /* total error log entries              */
static uint32_t g_state_trans = 0;   /* total state machine transitions      */

/* ---- Timestamp ------------------------------------------------- */

static inline void get_timestamp(char *buf, size_t buf_size) {
    SYSTEMTIME st;
    GetLocalTime(&st);
    snprintf(buf, buf_size, "%04d-%02d-%02d %02d:%02d:%02d.%03d",
             st.wYear, st.wMonth, st.wDay,
             st.wHour, st.wMinute, st.wSecond, st.wMilliseconds);
}

/* ---- Lifecycle ------------------------------------------------- */

/* REQ-LOG-040: Filename uses session start timestamp.
 * REQ-LOG-010: Created at startup, flushed per write. */
static inline int logger_init(void) {
    char ts[64], filename[256];
    get_timestamp(ts, sizeof(ts));

    snprintf(filename, sizeof(filename),
             "ATC_SERVER_%c%c%c%c%c%c%c%c_%c%c%c%c%c%c.log",
             ts[0],ts[1],ts[2],ts[3], ts[5],ts[6], ts[8],ts[9],
             ts[11],ts[12], ts[14],ts[15], ts[17],ts[18]);

    g_log_file = fopen(filename, "a");
    if (!g_log_file) { perror("Failed to create log file"); return -1; }

    /* Reset counters on each logger_init call. */
    g_packets_rx = g_packets_tx = g_errors = g_state_trans = 0;

    fprintf(g_log_file, "=== ATC Server Session Start: %s ===\n", ts);
    fflush(g_log_file);
    printf("[LOG] Log file created: %s\n", filename);
    return 0;
}

/* REQ-LOG-060: Session summary — total packets TX/RX, errors, state transitions. */
static inline void log_session_summary(void) {
    if (!g_log_file) return;
    char ts[64]; get_timestamp(ts, sizeof(ts));
    fprintf(g_log_file,
            "[SUMMARY] ATC_SERVER | %s | Packets RX=%u TX=%u | Errors=%u | State Transitions=%u\n",
            ts, g_packets_rx, g_packets_tx, g_errors, g_state_trans);
    fflush(g_log_file);
    printf("[ATC] Session summary — RX: %u  TX: %u  Errors: %u  Transitions: %u\n",
           g_packets_rx, g_packets_tx, g_errors, g_state_trans);
}

static inline void logger_close(void) {
    if (!g_log_file) return;
    char ts[64]; get_timestamp(ts, sizeof(ts));
    log_session_summary();   /* REQ-LOG-060 */
    fprintf(g_log_file, "[INFO] ATC_SERVER | %s | Server shutting down\n", ts);
    fprintf(g_log_file, "=== Session End ===\n");
    fflush(g_log_file);
    fclose(g_log_file);
    g_log_file = NULL;
}

/* ---- Log functions --------------------------------------------- */

/* REQ-LOG-030: [APP] | [TIMESTAMP] | [DIR] | [TYPE] | [SEQ] | [LEN] | [SUMMARY]
 * REQ-LOG-050: MAYDAY packets prefixed with [MAYDAY]. */
static inline void log_packet(const char *direction, const char *type_str,
                               uint32_t seq_num, uint32_t payload_length,
                               uint8_t emergency_flag, const char *summary) {
    if (!g_log_file) return;

    /* REQ-LOG-060: Increment directional counters. */
    if (direction && direction[0] == 'F') g_packets_rx++;   /* "FROM" */
    else                                  g_packets_tx++;   /* "TO"   */

    char ts[64]; get_timestamp(ts, sizeof(ts));
    const char *pfx = (emergency_flag == 1) ? "[MAYDAY] " : "";
    fprintf(g_log_file, "%sATC_SERVER | %s | %s | %s | SEQ=%u | LEN=%u | %s\n",
            pfx, ts, direction, type_str, seq_num, payload_length,
            summary ? summary : "(none)");
    fflush(g_log_file);
}

/* REQ-LOG-020: State transition entry — captures prev, next, trigger, timestamp. */
static inline void log_state_transition(ATCState prev, ATCState next, const char *trigger) {
    if (!g_log_file) return;
    g_state_trans++;   /* REQ-LOG-060 */
    char ts[64]; get_timestamp(ts, sizeof(ts));
    fprintf(g_log_file, "[STATE] ATC_SERVER | %s | %s -> %s | Trigger: %s\n",
            ts, state_to_str(prev), state_to_str(next), trigger);
    fflush(g_log_file);
}

static inline void log_info(const char *msg) {
    if (!g_log_file) return;
    char ts[64]; get_timestamp(ts, sizeof(ts));
    fprintf(g_log_file, "[INFO] ATC_SERVER | %s | %s\n", ts, msg);
    fflush(g_log_file);
}

static inline void log_error(const char *msg) {
    if (!g_log_file) return;
    g_errors++;   /* REQ-LOG-060 */
    char ts[64]; get_timestamp(ts, sizeof(ts));
    fprintf(g_log_file, "[ERROR] ATC_SERVER | %s | %s\n", ts, msg);
    fflush(g_log_file);
}

#endif /* LOGGER_H */
