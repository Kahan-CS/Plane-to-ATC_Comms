/**
 * @file main.c
 * @brief ATC Ground Control Server
 *
 * State flow:
 *   IDLE -> HANDSHAKE -> TAKEOFF -> TRANSIT -> LANDING -> DISCONNECTED -> IDLE
 *   - HANDSHAKE->TAKEOFF: when client sends PKT_TAKEOFF
 *   - TAKEOFF->TRANSIT:   when client sends PKT_TRANSIT
 *   - TRANSIT->LANDING:   when client sends PKT_LANDING
 *   - MAYDAY:             from any active state (TAKEOFF/TRANSIT/LANDING)
 *
 * Usage: gcc main.c -o main -lws2_32 && .\main.exe <PORT>
 *
 * Regulatory: CARs SOR/96-433 Part V · DO-178C DAL-D
 *
 *  Requirements covered in this file:
 *      REQ-SYS-010  Server is one of the two required applications
 *      REQ-SYS-050  All TX/RX packets are logged to file
 *      REQ-SYS-060  Server implements an operational state machine
 *      REQ-SYS-070  Server supports large data transfer command
 *      REQ-SYS-080  Handshake required before accepting any commands
 *      REQ-SVR-010  Listens on configurable port, accepts verified conns
 *      REQ-SVR-020  Parses structured packets and displays info
 *      REQ-SVR-030  Detects and flags MAYDAY packets
 *      REQ-SVR-040  Accepts buffered handoff packets via origin_atc_id
 *      REQ-SVR-050  Transmits >= 1 MB telemetry object on request
 *      REQ-SVR-060  Displays connected session log with phase
 *      REQ-SVR-070  Sends phase-appropriate ATC clearances
 *      REQ-STM-010  Defined states
 *      REQ-STM-020  Only sequential transitions permitted
 *      REQ-STM-030  Reject invalid commands with ERROR packet
 *      REQ-STM-040  MAYDAY sub-state from active flight phases
 *      REQ-LOG-010  Log created at startup, flushed per event
 *      REQ-LOG-020  State transitions logged as distinct entries
 *      REQ-LOG-050  MAYDAY packets prefixed with [MAYDAY] in log
 *      REQ-LOG-060  Session summary at shutdown
 *      REQ-COM-010  TCP/IP transport
 *      REQ-COM-060  ACK transmission after receiving a data packet
 *      REQ-PKT-020  Buffered handoff packets identified by origin_atc_id
 *      REQ-PKT-031  54-byte packet header layout
 *      REQ-PKT-034  aircraft_id and emergency_flag readable from header
 */

/* Server keepalive interval. The Rust client's receiver thread has a
 * 10-second read timeout. We must send *something* to the client more
 * often than that to prevent the receiver from declaring connection lost. */
#define SERVER_KEEPALIVE_INTERVAL_MS 8000LL

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <conio.h>

#include "../../shared/packet.h"
#include "include/server_config.h"
#include "include/state_machine.h"
#include "include/network.h"

/* Runtime ATC identity - set from command line.
 * Usage: atc-server <PORT> [ATC_NAME] [ATC_ID] */
char     g_atc_name[32] = "ATC-1";
uint32_t g_atc_id       = 1;


/* ================================================================
 *  Payload byte-order helpers
 *  (Rust client sends all multi-byte payload fields in big-endian)
 * ================================================================ */

static inline float swap_float(float f) {
    uint32_t tmp;
    memcpy(&tmp, &f, 4);
    tmp = ntohl(tmp);
    float result;
    memcpy(&result, &tmp, 4);
    return result;
}

static inline uint16_t swap_u16(uint16_t v) {
    return ntohs(v);
}

static inline int64_t swap_i64(int64_t v) {
    return (int64_t)swap64((uint64_t)v);
}


/* ================================================================
 *  Keepalive — empty PKT_ACK to prevent client receiver timeout
    REQ-COM-030 (keep-alive mechanism) companion on the server side.
 * ================================================================ */

static int send_server_keepalive(SOCKET sockfd, const char *aircraft_id) {
    PacketHeader hdr;
    memset(&hdr, 0, sizeof(PacketHeader));
    hdr.packet_type = PKT_ACK;
    hdr.seq_num = 0;                /* Reserved keepalive sequence */
    hdr.timestamp = now_ms();
    hdr.payload_length = 0;
    hdr.origin_atc_id = g_atc_id;
    hdr.emergency_flag = 0;
    if (aircraft_id) {
        strncpy(hdr.aircraft_id, aircraft_id, 31);
        hdr.aircraft_id[31] = '\0';
    }
    return send_packet(sockfd, &hdr, NULL);
}


/* ================================================================
 *  Session registry  (REQ-SVR-060)
    for the client reference, we just
 *  show the current session phase on the ATC console.
 * ================================================================ */

static int g_session_count = 0;

static void display_session_status(const char *aircraft_id, ATCState phase) {
    printf("--- [SESSION STATUS] Sessions: %d | Aircraft: %-12s | Phase: %s ---\n",
           g_session_count,
           (aircraft_id && aircraft_id[0]) ? aircraft_id : "(none)",
           state_to_str(phase));
}


/* ================================================================
 *  Client Session Handler
 * ================================================================ */

static void handle_client(SOCKET client_fd) {
    ATCState state = STATE_IDLE;
    ATCState prev  = STATE_IDLE;
    char     aircraft_id[32] = {0};

    int handshake_verified = 0;   /* Set true after PKT_HANDSHAKE verified */

    /* Tracks the last time the server sent ANY packet to the client.
     * Used to decide when a keepalive is needed.
     * IMPORTANT: only reset this when we actually SEND to the client,
     * never when we RECEIVE from it — the client's receiver thread
     * only cares about server->client traffic for its timeout. */
    int64_t last_send_ms = now_ms();

    g_session_count++;

    /* IDLE -> HANDSHAKE */
    prev = state; state = STATE_HANDSHAKE;
    log_state_transition(prev, state, "Client connected, awaiting handshake");
    printf("[ATC] State: %s -> %s\n", state_to_str(prev), state_to_str(state));
    display_session_status("(connecting...)", state);

    printf("\n[ATC] Press 'D' at any time to disconnect the aircraft.\n\n");

    while (1) {
        /* Check if ATC controller pressed a key. */
        if (_kbhit()) {
            int ch = _getch();
            if (ch == 'd' || ch == 'D') {
                printf("[ATC] Disconnect initiated by ATC controller.\n");
                send_ack(client_fd, 0, aircraft_id);
                log_info("ATC-initiated disconnect");
                goto session_end;
            }
        }

        /* Wait up to 1 second for client data (non-blocking). */
        fd_set rfds;
        struct timeval tv = { 1, 0 };
        FD_ZERO(&rfds);
        FD_SET(client_fd, &rfds);
        int ready = select(0, &rfds, NULL, NULL, &tv);

        if (ready <= 0) {
            /* No data from client. Check if we need to send a keepalive. */
            int64_t elapsed = now_ms() - last_send_ms;
            if (elapsed >= SERVER_KEEPALIVE_INTERVAL_MS) {
                if (send_server_keepalive(client_fd, aircraft_id) != 0) {
                    printf("[ATC] Connection lost (keepalive send failed).\n");
                    log_info("Connection lost - keepalive send failed");
                    goto session_end;
                }
                last_send_ms = now_ms();
            }
            continue;
        }

        PacketHeader hdr;
        uint8_t     *payload = NULL;

        if (recv_packet(client_fd, &hdr, &payload) != 0) {
            printf("[ATC] Connection closed or error.\n");
            log_info("Connection lost");
            break;
        }

        /* REQ-PKT-034: Capture aircraft_id from header before payload. */
        if (aircraft_id[0] == '\0') {
            strncpy(aircraft_id, hdr.aircraft_id, 31);
            aircraft_id[31] = '\0';
        }

        const char *type_str = packet_type_str(hdr.packet_type);

        /* REQ-SVR-030 / REQ-STM-040: Emergency flag check before dispatch. */
        if (hdr.emergency_flag == 1) {
            printf("\n!!! [MAYDAY] EMERGENCY from %s !!!\n\n", hdr.aircraft_id);
            char s[128]; snprintf(s, sizeof(s), "MAYDAY from %s", hdr.aircraft_id);
            log_packet("FROM", type_str, hdr.seq_num, hdr.payload_length, hdr.emergency_flag, s);

            if (state == STATE_TAKEOFF || state == STATE_TRANSIT || state == STATE_LANDING) {
                prev = state; state = STATE_MAYDAY;
                log_state_transition(prev, state, "Emergency flag set in packet header");
                printf("[ATC] State: %s -> %s\n", state_to_str(prev), state_to_str(state));
                display_session_status(aircraft_id, state);
            }

            send_ack(client_fd, hdr.seq_num, aircraft_id);
            last_send_ms = now_ms();
            free(payload); continue;
        }

     /* REQ-SVR-040 / REQ-PKT-020: Detect buffered handoff packets. */
        if (hdr.origin_atc_id != 0 && hdr.origin_atc_id != SERVER_ATC_ID) {
            printf("[ATC] Buffered handoff packet from ATC #%u for %s\n",
                   hdr.origin_atc_id, hdr.aircraft_id);
            char m[128]; snprintf(m, sizeof(m), "Buffered handoff packet from ATC #%u", hdr.origin_atc_id);
            log_info(m);

            /* Accept and ACK buffered packets without state machine enforcement.
             * These are historical retransmissions, not new commands. */
            send_ack(client_fd, hdr.seq_num, aircraft_id);
            last_send_ms = now_ms();
            free(payload); continue;
        }

        /* REQ-SYS-080: Reject any command received before handshake is verified. */
        if (!handshake_verified &&
            !is_allowed_before_handshake(hdr.packet_type)) {
            char err[160];
            snprintf(err, sizeof(err),
                    "Packet %s rejected: handshake not yet completed (REQ-SYS-080)",
                    type_str);
            send_error_pkt(client_fd, err, aircraft_id);
            last_send_ms = now_ms();
            free(payload);
            continue;
        }

        /* Log incoming packet (skip logging heartbeats to reduce noise). */
        if (hdr.packet_type != PKT_ACK) {
            char summary[256] = "(empty)";
            if (payload && hdr.payload_length > 0) {
                size_t n = hdr.payload_length < 128 ? hdr.payload_length : 128;
                snprintf(summary, sizeof(summary), "%.*s", (int)n, (char *)payload);
            }
            log_packet("FROM", type_str, hdr.seq_num, hdr.payload_length, hdr.emergency_flag, summary);
        }

        /* ---- Dispatch ---- */
        switch (hdr.packet_type) {

        /* ---- HANDSHAKE ---- */
        case PKT_HANDSHAKE: {
            if (state != STATE_HANDSHAKE) {
                send_error_pkt(client_fd, "HANDSHAKE not expected in current state", aircraft_id);
                last_send_ms = now_ms();
                break;
            }
            printf("[ATC] HANDSHAKE from: %s\n", hdr.aircraft_id);

            if (payload && hdr.payload_length >= sizeof(HandshakePayload)) {
                const HandshakePayload *hs = (const HandshakePayload *)payload;
                printf("[ATC]   Callsign : %.12s\n",  hs->callsign);
                printf("[ATC]   Type     : %.16s\n",  hs->aircraft_type);
                printf("[ATC]   Model    : %.16s\n",  hs->aircraft_model);
                printf("[ATC]   Origin   : %.4s\n",   hs->origin);
                printf("[ATC]   Dest     : %.4s\n",   hs->destination);
            } else {
                printf("[ATC]   (payload too small for HandshakePayload)\n");
            }

            if (hdr.aircraft_id[0] == '\0') {
                send_error_pkt(client_fd, "aircraft_id is empty - handshake rejected", aircraft_id);
                log_error("Handshake rejected: empty aircraft_id");
                last_send_ms = now_ms();
                break;
            }

           /* REQ-SYS-080: Handshake verified, but remain in STATE_HANDSHAKE
            * until client explicitly sends PKT_TAKEOFF. This enforces an
            * explicit "ready-to-depart" command from the pilot rather than
            * auto-advancing on handshake success. */
            
            handshake_verified = 1;

            /* Determine which state to enter based on plane's declared phase.
             * initial_phase==0 stays in HANDSHAKE; 2/3 support mid-flight reconnect. */
            uint8_t initial_phase = 0;
            if (payload && hdr.payload_length >= sizeof(HandshakePayload)) {
                const HandshakePayload *hs = (const HandshakePayload *)payload;
                initial_phase = hs->initial_phase;
            }

            if (initial_phase == 2) {
                prev = state; state = STATE_TRANSIT;
                log_state_transition(prev, state, "Handshake: initial_phase=TRANSIT");
                printf("[ATC] State: %s -> %s (mid-flight reconnect)\n",
                       state_to_str(prev), state_to_str(state));
            } else if (initial_phase == 3) {
                prev = state; state = STATE_LANDING;
                log_state_transition(prev, state, "Handshake: initial_phase=LANDING");
                printf("[ATC] State: %s -> %s (mid-flight reconnect)\n",
                       state_to_str(prev), state_to_str(state));
            } else {
                printf("[ATC] Handshake VERIFIED. Waiting for takeoff.\n");
            }

            display_session_status(aircraft_id, state);
            send_ack(client_fd, hdr.seq_num, aircraft_id);           //REQ-COM-060: ACK handshake packet to confirm receipt and verification
            //send_server_keepalive(client_fd, aircraft_id);
            last_send_ms = now_ms();
            break;
        }

        /* ---- TAKEOFF ---- */
        case PKT_TAKEOFF: {
            /* REQ-STM-020: TAKEOFF is valid only after handshake verification.
            * First TAKEOFF packet triggers HANDSHAKE -> TAKEOFF transition. */
            if (state != STATE_HANDSHAKE && state != STATE_TAKEOFF) {
                send_error_pkt(client_fd,
                            "TAKEOFF not valid in current state",
                            aircraft_id);
                last_send_ms = now_ms();
                break;
            }

            if (state == STATE_HANDSHAKE) {
                prev = state; state = STATE_TAKEOFF;
                log_state_transition(prev, state, "Takeoff command received");
                printf("[ATC] State: %s -> %s\n",
                    state_to_str(prev), state_to_str(state));
                display_session_status(aircraft_id, state);
            }

            float    heading = 0.0f, altitude = 0.0f, spd = 0.0f, clmb = 0.0f;
            uint16_t squawk  = 0;
            uint8_t  clr_type = 0;

            printf("[ATC] TAKEOFF telemetry from: %s\n", aircraft_id);
            if (payload && hdr.payload_length >= sizeof(TakeoffPayload)) {
                TakeoffPayload tk_host;
                memcpy(&tk_host, payload, sizeof(TakeoffPayload));

                tk_host.departure_time     = swap_i64(tk_host.departure_time);
                tk_host.assigned_heading   = swap_float(tk_host.assigned_heading);
                tk_host.assigned_altitude  = swap_float(tk_host.assigned_altitude);
                tk_host.squawk_code        = swap_u16(tk_host.squawk_code);
                tk_host.wind_speed         = swap_float(tk_host.wind_speed);
                tk_host.wind_direction     = swap_float(tk_host.wind_direction);
                tk_host.speed_off_runway   = swap_float(tk_host.speed_off_runway);
                tk_host.initial_climb_rate = swap_float(tk_host.initial_climb_rate);

                heading  = tk_host.assigned_heading;
                altitude = tk_host.assigned_altitude;
                spd      = tk_host.speed_off_runway;
                clmb     = tk_host.initial_climb_rate;
                squawk   = tk_host.squawk_code;
                clr_type = tk_host.clearance_type;

                printf("[ATC]   Clearance    : %s\n",       clr_type == 0 ? "IFR" : "VFR");
                printf("[ATC]   Squawk       : %04o\n",     squawk);
                printf("[ATC]   Heading      : %.1f deg\n", heading);
                printf("[ATC]   Altitude     : %.0f ft MSL\n", altitude);
                printf("[ATC]   Wind Speed   : %.1f kts\n", tk_host.wind_speed);
                printf("[ATC]   Wind Dir     : %.1f deg\n", tk_host.wind_direction);
                printf("[ATC]   Runway Speed : %.1f kts\n", spd);
                printf("[ATC]   Climb Rate   : %.0f fpm\n", clmb);
            } else {
                printf("[ATC]   (payload too small for TakeoffPayload)\n");
            }

            /* REQ-SVR-070: Send departure clearance to client as ACK payload. */
            {
                char clearance[200];
                snprintf(clearance, sizeof(clearance),
                         "[DEPARTURE CLEARANCE] %s CLEARED %s. HDG %.0f, ALT %.0f FT, SQUAWK %04o.",
                         aircraft_id, clr_type == 0 ? "IFR" : "VFR",
                         heading, altitude, squawk);

                PacketHeader ack_hdr;
                build_header(&ack_hdr, PKT_ACK, (uint32_t)strlen(clearance), aircraft_id);
                send_packet(client_fd, &ack_hdr, clearance);

                printf("[ATC] CLEARANCE: %s\n", clearance);
                char log_sum[256];
                snprintf(log_sum, sizeof(log_sum), "ACK+CLEARANCE for seq=%u: %.200s", hdr.seq_num, clearance);
                log_packet("TO", "ACK", ack_hdr.seq_num, ack_hdr.payload_length, 0, log_sum);
                last_send_ms = now_ms();
            }
            break;
        }

        /* ---- TRANSIT ---- */
        case PKT_TRANSIT: {
            if (state != STATE_TAKEOFF && state != STATE_TRANSIT) {
                send_error_pkt(client_fd, "TRANSIT not valid in current state", aircraft_id);
                last_send_ms = now_ms();
                break;
            }

            if (state == STATE_TAKEOFF) {
                prev = state; state = STATE_TRANSIT;
                log_state_transition(prev, state, "Transit packet received");
                printf("[ATC] State: %s -> %s\n", state_to_str(prev), state_to_str(state));
                display_session_status(aircraft_id, state);
            }

            printf("[ATC] TRANSIT telemetry from: %s\n", aircraft_id);
            if (payload && hdr.payload_length >= sizeof(TransitPayload)) {
                TransitPayload tr_host;
                memcpy(&tr_host, payload, sizeof(TransitPayload));

                tr_host.speed       = swap_float(tr_host.speed);
                tr_host.altitude    = swap_float(tr_host.altitude);
                tr_host.heading     = swap_float(tr_host.heading);
                tr_host.squawk_code = swap_u16(tr_host.squawk_code);

                printf("[ATC]   Speed    : %.1f kts\n",    tr_host.speed);
                printf("[ATC]   Altitude : %.0f ft MSL\n", tr_host.altitude);
                printf("[ATC]   Heading  : %.1f deg\n",    tr_host.heading);
                printf("[ATC]   Squawk   : %04o\n",        tr_host.squawk_code);
            } else {
                printf("[ATC]   (payload too small for TransitPayload)\n");
            }

                    send_ack(client_fd, hdr.seq_num, aircraft_id);
                    last_send_ms = now_ms();
                    break;
        }

        /* ---- LANDING ---- */
        case PKT_LANDING: {
            if (state != STATE_TRANSIT && state != STATE_LANDING) {
                send_error_pkt(client_fd, "LANDING not valid in current state", aircraft_id);
                last_send_ms = now_ms();
                break;
            }

            if (state == STATE_TRANSIT) {
                prev = state; state = STATE_LANDING;
                log_state_transition(prev, state, "Landing packet received");
                printf("[ATC] State: %s -> %s\n", state_to_str(prev), state_to_str(state));
                display_session_status(aircraft_id, state);
            }

            char    runway[5]       = "N/A";
            int     approach_clear  = 0;
            float   apspd = 0.0f, alt = 0.0f, hdg = 0.0f;

            printf("[ATC] LANDING telemetry from: %s\n", aircraft_id);
            if (payload && hdr.payload_length >= sizeof(LandingPayload)) {
                LandingPayload ld_host;
                memcpy(&ld_host, payload, sizeof(LandingPayload));

                ld_host.approach_speed   = swap_float(ld_host.approach_speed);
                ld_host.current_altitude = swap_float(ld_host.current_altitude);
                ld_host.heading          = swap_float(ld_host.heading);
                ld_host.wind_shear       = swap_float(ld_host.wind_shear);
                ld_host.visibility       = swap_float(ld_host.visibility);

                apspd          = ld_host.approach_speed;
                alt            = ld_host.current_altitude;
                hdg            = ld_host.heading;
                approach_clear = ld_host.approach_clearance;
                strncpy(runway, ld_host.assigned_runway, 4);
                runway[4] = '\0';

                printf("[ATC]   Approach Speed : %.1f kts\n",    apspd);
                printf("[ATC]   Altitude       : %.0f ft MSL\n", alt);
                printf("[ATC]   Heading        : %.1f deg\n",    hdg);
                printf("[ATC]   Runway         : %s\n",          runway);
                printf("[ATC]   Approach Clear : %s\n",          approach_clear ? "YES" : "NO");
                printf("[ATC]   Wind Shear     : %.1f kts\n",   ld_host.wind_shear);
                printf("[ATC]   Visibility     : %.1f SM\n",    ld_host.visibility);
            } else {
                printf("[ATC]   (payload too small for LandingPayload)\n");
            }

          /* REQ-SVR-070: Send landing clearance to client as ACK payload. */
            {
                char clearance[200];
                snprintf(clearance, sizeof(clearance),
                         "[LANDING CLEARANCE] %s RWY %s %s. GO-AROUND: %s.",
                         aircraft_id, runway,
                         approach_clear ? "CLEARED TO LAND" : "NOT CLEARED - HOLD SHORT",
                         approach_clear ? "NEGATIVE"         : "AFFIRM IF UNABLE TO LAND");

                PacketHeader ack_hdr;
                build_header(&ack_hdr, PKT_ACK, (uint32_t)strlen(clearance), aircraft_id);
                send_packet(client_fd, &ack_hdr, clearance);

                printf("[ATC] CLEARANCE: %s\n", clearance);
                char log_sum[256];
                snprintf(log_sum, sizeof(log_sum), "ACK+CLEARANCE for seq=%u: %.200s", hdr.seq_num, clearance);
                log_packet("TO", "ACK", ack_hdr.seq_num, ack_hdr.payload_length, 0, log_sum);
                last_send_ms = now_ms();
            }
            break;
        }

        /* ---- MAYDAY explicit packet ---- */
        case PKT_MAYDAY: {
            if (state != STATE_TAKEOFF && state != STATE_TRANSIT &&
                state != STATE_LANDING && state != STATE_MAYDAY) {
                send_error_pkt(client_fd, "MAYDAY packet not valid in current state", aircraft_id);
                last_send_ms = now_ms();
                break;
            }
            printf("\n!!! [MAYDAY] PKT_MAYDAY received from %s !!!\n\n", aircraft_id);

            if (state != STATE_MAYDAY) {
                prev = state; state = STATE_MAYDAY;
                log_state_transition(prev, state, "PKT_MAYDAY received");
                printf("[ATC] State: %s -> %s\n", state_to_str(prev), state_to_str(state));
                display_session_status(aircraft_id, state);
            }
            send_ack(client_fd, hdr.seq_num, aircraft_id);
            last_send_ms = now_ms();
            break;
        }

        /* ---- LARGE DATA REQUEST (REQ-SVR-050) ---- */
        case PKT_LARGE_DATA_REQUEST: {
            printf("[ATC] Large data request from: %s\n", aircraft_id);
            log_info("Processing large weather data request");
            send_ack(client_fd, hdr.seq_num, aircraft_id);
            send_large_data(client_fd, aircraft_id);
            last_send_ms = now_ms();
            break;
        }

        /* ---- HANDOFF NOTIFY (REQ-SVR-040) ---- */
        case PKT_HANDOFF_NOTIFY: {
            printf("[ATC] Handoff notification from %s - releasing session.\n", aircraft_id);
            log_info("Handoff notification received - ending session to accept next client");
            send_ack(client_fd, hdr.seq_num, aircraft_id);
            last_send_ms = now_ms();
            free(payload);
            goto session_end;
        }

        /* ---- DISCONNECT ---- */
        case PKT_DISCONNECT: {
            printf("[ATC] %s requesting disconnect\n", aircraft_id);
            send_ack(client_fd, hdr.seq_num, aircraft_id);

            const ATCState path[] = { STATE_TRANSIT, STATE_LANDING, STATE_DISCONNECTED };
            for (int i = 0; i < 3; i++) {
                if (is_valid_transition(state, path[i])) {
                    prev = state; state = path[i];
                    log_state_transition(prev, state, "Disconnect requested");
                }
            }
            if (state == STATE_MAYDAY) {
                prev = state; state = STATE_DISCONNECTED;
                log_state_transition(prev, state, "Disconnect from MAYDAY state");
            }
            free(payload); goto session_end;
        }

        /* ---- ACK / Heartbeat from client — do NOT reset last_send_ms ---- */
        case PKT_ACK: {
            break;
        }

        /* REQ-STM-030: Reject unhandled / out-of-state packets. */
        default: {
            char err[128];
            snprintf(err, sizeof(err), "Packet type %s (0x%02X) not handled in state %s",
                     type_str, hdr.packet_type, state_to_str(state));
            send_error_pkt(client_fd, err, aircraft_id);
            last_send_ms = now_ms();
            break;
        }
        }
        free(payload);
    }

session_end:
    if (state != STATE_DISCONNECTED && state != STATE_IDLE) {
        prev = state; state = STATE_DISCONNECTED;
        log_state_transition(prev, state, "Session ended abnormally");
    }
    if (state == STATE_DISCONNECTED) {
        prev = state; state = STATE_IDLE;
        log_state_transition(prev, state, "Session reset to IDLE");
    }
    display_session_status(NULL, STATE_IDLE);
    printf("[ATC] Session closed for %s. State returned to IDLE.\n",
           aircraft_id[0] ? aircraft_id : "(unknown)");
    log_session_summary();
}


/* ================================================================
 *  Entry Point
 * ================================================================ */

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <PORT> [ATC_NAME] [ATC_ID]\n", argv[0]);
        fprintf(stderr, "  e.g: %s 9000 CYYZ-NORTH 2\n", argv[0]);
        return 1;
    }

    int port = atoi(argv[1]);
    if (port <= 0 || port > 65535) {
        fprintf(stderr, "Invalid port: %s\n", argv[1]); return 1;
    }

    if (argc >= 3) {
        strncpy(g_atc_name, argv[2], sizeof(g_atc_name) - 1);
        g_atc_name[sizeof(g_atc_name) - 1] = '\0';
    }
    if (argc >= 4) {
        int id = atoi(argv[3]);
        if (id > 0) g_atc_id = (uint32_t)id;
    }

    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        fprintf(stderr, "WSAStartup failed\n"); return 1;
    }
    if (logger_init() != 0) { WSACleanup(); return 1; }

    printf("\n========================================\n");
    printf("   ATC GROUND CONTROL SERVER\n");
    printf("   CSCN74000 - Group 3\n");
    printf("   ATC ID: %u  |  Name: %s\n", g_atc_id, g_atc_name);
    printf("========================================\n");
    printf("[ATC] Starting on port %d...\n\n", port);

    char start_msg[96];
    snprintf(start_msg, sizeof(start_msg), "Server starting - ID=%u Name=%s Port=%d",
             g_atc_id, g_atc_name, port);
    log_info(start_msg);

    SOCKET server_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (server_fd == INVALID_SOCKET) {
        fprintf(stderr, "[ATC] socket() failed: %d\n", WSAGetLastError());
        logger_close(); WSACleanup(); return 1;
    }

    int opt = 1;
    setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, (const char *)&opt, sizeof(opt));

    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family      = AF_INET;
    addr.sin_addr.s_addr = INADDR_ANY;
    addr.sin_port        = htons((uint16_t)port);

    if (bind(server_fd, (struct sockaddr *)&addr, sizeof(addr)) == SOCKET_ERROR) {
        fprintf(stderr, "[ATC] bind() failed: %d\n", WSAGetLastError());
        closesocket(server_fd); logger_close(); WSACleanup(); return 1;
    }
    if (listen(server_fd, 1) == SOCKET_ERROR) {
        fprintf(stderr, "[ATC] listen() failed: %d\n", WSAGetLastError());
        closesocket(server_fd); logger_close(); WSACleanup(); return 1;
    }

    printf("[ATC] Listening on port %d - waiting for aircraft...\n\n", port);
    log_info("Listening for connections");

    while (1) {
        struct sockaddr_in client_addr;
        int addr_len = sizeof(client_addr);
        SOCKET client_fd = accept(server_fd, (struct sockaddr *)&client_addr, &addr_len);
        if (client_fd == INVALID_SOCKET) {
            fprintf(stderr, "[ATC] accept() failed: %d\n", WSAGetLastError());
            log_error("accept() failed");
            continue;
        }

        char client_ip[INET_ADDRSTRLEN] = "unknown";
        inet_ntop(AF_INET, &client_addr.sin_addr, client_ip, sizeof(client_ip));
        printf("[ATC] New connection from %s:%d (session #%d)\n",
               client_ip, ntohs(client_addr.sin_port), g_session_count + 1);
        log_info("Client connected");

        handle_client(client_fd);

        closesocket(client_fd);
        printf("[ATC] Waiting for next aircraft...\n\n");
    }

    closesocket(server_fd);
    logger_close();
    WSACleanup();
    return 0;
}
