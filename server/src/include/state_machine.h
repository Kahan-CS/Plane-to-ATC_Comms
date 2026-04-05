/**
 * @file  state_machine.h
 * @brief ATC server operational state machine.
 *
 * REQ-SYS-060, REQ-STM-010, REQ-STM-020, REQ-STM-030
 */

#ifndef STATE_MACHINE_H
#define STATE_MACHINE_H

/* REQ-STM-010: Defined states. */
typedef enum {
    STATE_IDLE,
    STATE_HANDSHAKE,
    STATE_TAKEOFF,
    STATE_TRANSIT,
    STATE_LANDING,
    STATE_DISCONNECTED
} ATCState;

static inline const char *state_to_str(ATCState s) {
    switch (s) {
        case STATE_IDLE:          return "IDLE";
        case STATE_HANDSHAKE:     return "HANDSHAKE";
        case STATE_TAKEOFF:       return "TAKEOFF";
        case STATE_TRANSIT:       return "TRANSIT";
        case STATE_LANDING:       return "LANDING";
        case STATE_DISCONNECTED:  return "DISCONNECTED";
        default:                  return "UNKNOWN";
    }
}

/* REQ-STM-020: Only sequential transitions permitted.
 * IDLE → HANDSHAKE → TAKEOFF → TRANSIT → LANDING → DISCONNECTED → IDLE */
static inline int is_valid_transition(ATCState from, ATCState to) {
    switch (from) {
        case STATE_IDLE:          return (to == STATE_HANDSHAKE);
        case STATE_HANDSHAKE:     return (to == STATE_TAKEOFF);
        case STATE_TAKEOFF:       return (to == STATE_TRANSIT);
        case STATE_TRANSIT:       return (to == STATE_LANDING);
        case STATE_LANDING:       return (to == STATE_DISCONNECTED);
        case STATE_DISCONNECTED:  return (to == STATE_IDLE);
        default:                  return 0;
    }
}

#endif /* STATE_MACHINE_H */