/**
 * @file main.c
 * @brief ATC Ground Control Server
 *
 * Functionality:
 *  - Listen for incoming TCP client connections on a configurable port (REQ-SVR-010)
 *  - Accept and verify connections via handshake exchange
 *  - Receive and parse all structured data packets; display flight information (REQ-SVR-020)
 *  - Detect and visually flag MAYDAY emergency packets (REQ-SVR-030)
 *  - Detect and process buffered handoff packets (REQ-SVR-040)
 *  - Transmit >= 1 MB weather/telemetry data on request (REQ-SVR-050)
 *  - Display session log of connected aircraft and current phase (REQ-SVR-060)
 *  - Send phase-appropriate ATC instructions to the client (REQ-SVR-070)
 *  - Enforce state machine: IDLE->HANDSHAKE->TAKEOFF->TRANSIT->LANDING->DISCONNECTED (REQ-STM-010/020/030)
 *  - Transition to MAYDAY sub-state from any active flight state (REQ-STM-040)
 *  - Log all events with flush-per-write; emit session summary at shutdown (REQ-LOG-010/060)
 *
 * Usage: atc-server <PORT>
 *
 * Regulatory compliance:
 *   - CARs SOR/96-433 Part V (Airworthiness)
 *   - DO-178C DAL-D guidance (deterministic state machine, traceable logging)
 *
 * REQ-SVR-010, REQ-SVR-020, REQ-SVR-030, REQ-SVR-040, REQ-SVR-050,
 * REQ-SVR-060, REQ-SVR-070, REQ-STM-010, REQ-STM-020, REQ-STM-030,
 * REQ-STM-040, REQ-LOG-010, REQ-LOG-020, REQ-LOG-060
 */

 /*Command to run the server:
  gcc main.c -o main -lws2_32
  .\main.exe <PORT_NUMBER>
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../../shared/packet.h"
#include "include/server_config.h"
#include "include/state_machine.h"
#include "include/network.h"


/* ================================================================
 *  Session registry  (REQ-SVR-060)
 * ================================================================ */

static int     g_session_count = 0;

/* Print a one-line session status panel to stdout (REQ-SVR-060). */
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

    g_session_count++;

    /* IDLE -> HANDSHAKE */
    prev = state; state = STATE_HANDSHAKE;
    log_state_transition(prev, state, "Client connected, awaiting handshake");
    printf("[ATC] State: %s -> %s\n", state_to_str(prev), state_to_str(state));
    display_session_status("(connecting...)", state);

    while (1) {
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

        /* REQ-SVR-030 / REQ-LOG-050: Emergency flag — check before switch.
         * REQ-STM-040: Transition to MAYDAY sub-state from any active flight state. */
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
            free(payload); continue;
        }

        /* REQ-SVR-040 / REQ-PKT-020: Detect buffered handoff packets. */
        if (hdr.origin_atc_id != 0 && hdr.origin_atc_id != SERVER_ATC_ID) {
            printf("[ATC] Buffered handoff packet from ATC #%u for %s\n",
                   hdr.origin_atc_id, hdr.aircraft_id);
            char m[128]; snprintf(m, sizeof(m), "Buffered handoff packet from ATC #%u", hdr.origin_atc_id);
            log_info(m);
        }

        /* Log incoming packet. */
        char summary[256] = "(empty)";
        if (payload && hdr.payload_length > 0) {
            size_t n = hdr.payload_length < 128 ? hdr.payload_length : 128;
            snprintf(summary, sizeof(summary), "%.*s", (int)n, (char *)payload);
        }
        log_packet("FROM", type_str, hdr.seq_num, hdr.payload_length, hdr.emergency_flag, summary);

        /* ---- Dispatch ---- */
        switch (hdr.packet_type) {

        /* ---- HANDSHAKE (REQ-SVR-010, REQ-PKT-061) ---- */
        case PKT_HANDSHAKE: {
            if (state != STATE_HANDSHAKE) {
                send_error_pkt(client_fd, "HANDSHAKE not expected in current state", aircraft_id);
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
                break;
            }

            prev = state; state = STATE_TAKEOFF;
            log_state_transition(prev, state, "Handshake verified");
            printf("[ATC] Handshake VERIFIED. State: %s -> %s\n",
                   state_to_str(prev), state_to_str(state));
            display_session_status(aircraft_id, state);
            send_ack(client_fd, hdr.seq_num, aircraft_id);
            break;
        }

        /* ---- TAKEOFF (REQ-SVR-020, REQ-SVR-070) ---- */
        case PKT_TAKEOFF: {
            if (state != STATE_TAKEOFF) {
                send_error_pkt(client_fd, "TAKEOFF not valid in current state", aircraft_id);
                break;
            }

            float    heading = 0.0f, altitude = 0.0f, spd = 0.0f, clmb = 0.0f;
            uint16_t squawk  = 0;
            uint8_t  clr_type = 0;

            printf("[ATC] TAKEOFF telemetry from: %s\n", aircraft_id);
            if (payload && hdr.payload_length >= sizeof(TakeoffPayload)) {
                const TakeoffPayload *tk = (const TakeoffPayload *)payload;
                heading  = tk->assigned_heading;
                altitude = tk->assigned_altitude;
                spd      = tk->speed_off_runway;
                clmb     = tk->initial_climb_rate;
                squawk   = tk->squawk_code;
                clr_type = tk->clearance_type;
                printf("[ATC]   Clearance    : %s\n",   clr_type == 0 ? "IFR" : "VFR");
                printf("[ATC]   Squawk       : %04o\n", squawk);
                printf("[ATC]   Heading      : %.1f deg\n", heading);
                printf("[ATC]   Altitude     : %.0f ft MSL\n", altitude);
                printf("[ATC]   Runway Speed : %.1f kts\n", spd);
                printf("[ATC]   Climb Rate   : %.0f fpm\n", clmb);
            } else {
                printf("[ATC]   (payload too small for TakeoffPayload)\n");
            }

            prev = state; state = STATE_TRANSIT;
            log_state_transition(prev, state, "Takeoff packet received");
            printf("[ATC] State: %s -> %s\n", state_to_str(prev), state_to_str(state));
            display_session_status(aircraft_id, state);
            send_ack(client_fd, hdr.seq_num, aircraft_id);

            /* REQ-SVR-070: Send departure clearance with assigned heading, altitude, squawk. */
            {
                char clearance[160];
                snprintf(clearance, sizeof(clearance),
                         "[DEPARTURE CLEARANCE] %s CLEARED %s. "
                         "MAINTAIN HDG %.0f, CLIMB AND MAINTAIN %.0f FT, SQUAWK %04o.",
                         aircraft_id, clr_type == 0 ? "IFR" : "VFR",
                         heading, altitude, squawk);
                send_atc_clearance(client_fd, clearance, aircraft_id);
            }
            break;
        }

        /* ---- TRANSIT (REQ-SVR-020) ---- */
        case PKT_TRANSIT: {
            if (state != STATE_TRANSIT) {
                send_error_pkt(client_fd, "TRANSIT not valid in current state", aircraft_id);
                break;
            }

            printf("[ATC] TRANSIT telemetry from: %s\n", aircraft_id);
            if (payload && hdr.payload_length >= sizeof(TransitPayload)) {
                const TransitPayload *tr = (const TransitPayload *)payload;
                printf("[ATC]   Speed    : %.1f kts\n",    tr->speed);
                printf("[ATC]   Altitude : %.0f ft MSL\n", tr->altitude);
                printf("[ATC]   Heading  : %.1f deg\n",    tr->heading);
                printf("[ATC]   Squawk   : %04o\n",        tr->squawk_code);
            } else {
                printf("[ATC]   (payload too small for TransitPayload)\n");
            }

            prev = state; state = STATE_LANDING;
            log_state_transition(prev, state, "Transit packet received");
            printf("[ATC] State: %s -> %s\n", state_to_str(prev), state_to_str(state));
            display_session_status(aircraft_id, state);
            send_ack(client_fd, hdr.seq_num, aircraft_id);
            break;
        }

        /* ---- LANDING (REQ-SVR-020, REQ-SVR-070) ---- */
        case PKT_LANDING: {
            if (state != STATE_LANDING) {
                send_error_pkt(client_fd, "LANDING not valid in current state", aircraft_id);
                break;
            }

            char    runway[5]       = "N/A";
            int     approach_clear  = 0;
            float   apspd = 0.0f, alt = 0.0f, hdg = 0.0f;

            printf("[ATC] LANDING telemetry from: %s\n", aircraft_id);
            if (payload && hdr.payload_length >= sizeof(LandingPayload)) {
                const LandingPayload *ld = (const LandingPayload *)payload;
                apspd          = ld->approach_speed;
                alt            = ld->current_altitude;
                hdg            = ld->heading;
                approach_clear = ld->approach_clearance;
                strncpy(runway, ld->assigned_runway, 4);
                runway[4] = '\0';
                printf("[ATC]   Approach Speed : %.1f kts\n",   apspd);
                printf("[ATC]   Altitude       : %.0f ft MSL\n", alt);
                printf("[ATC]   Heading        : %.1f deg\n",    hdg);
                printf("[ATC]   Runway         : %s\n",          runway);
                printf("[ATC]   Approach Clear : %s\n",          approach_clear ? "YES" : "NO");
                printf("[ATC]   Wind Shear     : %.1f kts\n",   ld->wind_shear);
                printf("[ATC]   Visibility     : %.1f SM\n",    ld->visibility);
            } else {
                printf("[ATC]   (payload too small for LandingPayload)\n");
            }

            send_ack(client_fd, hdr.seq_num, aircraft_id);

            /* REQ-SVR-070: Send landing instructions — runway assignment and go-around command. */
            {
                char instr[160];
                snprintf(instr, sizeof(instr),
                         "[LANDING CLEARANCE] %s RWY %s %s. GO-AROUND: %s.",
                         aircraft_id, runway,
                         approach_clear ? "CLEARED TO LAND" : "NOT CLEARED - HOLD SHORT",
                         approach_clear ? "NEGATIVE"         : "AFFIRM IF UNABLE TO LAND");
                send_atc_clearance(client_fd, instr, aircraft_id);
            }
            break;
        }

        /* ---- MAYDAY explicit packet (REQ-SVR-030, REQ-STM-040) ---- */
        case PKT_MAYDAY: {
            /* NOTE: packets with emergency_flag==1 are caught before the switch.
             * This case handles a PKT_MAYDAY type with emergency_flag==0. */
            if (state != STATE_TAKEOFF && state != STATE_TRANSIT &&
                state != STATE_LANDING && state != STATE_MAYDAY) {
                send_error_pkt(client_fd, "MAYDAY packet not valid in current state", aircraft_id);
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
            break;
        }

        /* ---- LARGE DATA REQUEST (REQ-SVR-050) ---- */
        case PKT_LARGE_DATA_REQUEST: {
            printf("[ATC] Large data request from: %s\n", aircraft_id);
            log_info("Processing large weather data request");
            send_ack(client_fd, hdr.seq_num, aircraft_id);
            send_large_data(client_fd, aircraft_id);
            break;
        }

        /* ---- HANDOFF NOTIFY (REQ-SVR-040) ---- */
        case PKT_HANDOFF_NOTIFY: {
            printf("[ATC] Handoff notification from ATC #%u — aircraft: %s\n",
                   hdr.origin_atc_id, aircraft_id);
            log_info("Handoff notification acknowledged");
            send_ack(client_fd, hdr.seq_num, aircraft_id);
            break;
        }

        /* ---- DISCONNECT — graceful teardown ---- */
        case PKT_DISCONNECT: {
            printf("[ATC] %s requesting disconnect\n", aircraft_id);
            send_ack(client_fd, hdr.seq_num, aircraft_id);

            /* Advance through any skipped intermediate states to DISCONNECTED.
             * Also handles MAYDAY -> DISCONNECTED via is_valid_transition(). */
            const ATCState path[] = { STATE_TRANSIT, STATE_LANDING, STATE_DISCONNECTED };
            for (int i = 0; i < 3; i++) {
                if (is_valid_transition(state, path[i])) {
                    prev = state; state = path[i];
                    log_state_transition(prev, state, "Disconnect requested");
                }
            }
            /* MAYDAY -> DISCONNECTED (not covered by path[] above). */
            if (state == STATE_MAYDAY) {
                prev = state; state = STATE_DISCONNECTED;
                log_state_transition(prev, state, "Disconnect from MAYDAY state");
            }
            free(payload); goto session_end;
        }

        /* REQ-STM-030: Reject all unhandled / out-of-state packets. */
        default: {
            char err[128];
            snprintf(err, sizeof(err), "Packet type %s (0x%02X) not handled in state %s",
                     type_str, hdr.packet_type, state_to_str(state));
            send_error_pkt(client_fd, err, aircraft_id);
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
    log_session_summary();   /* REQ-LOG-060 */
}


/* ================================================================
 *  Entry Point  (REQ-SVR-010)
 * ================================================================ */

int main(int argc, char *argv[]) {
    if (argc < 2) { fprintf(stderr, "Usage: %s <PORT>\n", argv[0]); return 1; }

    int port = atoi(argv[1]);
    if (port <= 0 || port > 65535) {
        fprintf(stderr, "Invalid port: %s\n", argv[1]); return 1;
    }

    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        fprintf(stderr, "WSAStartup failed\n"); return 1;
    }
    if (logger_init() != 0) { WSACleanup(); return 1; }

    printf("\n========================================\n");
    printf("   ATC GROUND CONTROL SERVER\n");
    printf("   CSCN74000 - Group 3\n");
    printf("   ATC ID: %u  |  Airport: %s\n", SERVER_ATC_ID, SERVER_AIRPORT_CODE);
    printf("========================================\n");
    printf("[ATC] Starting on port %d...\n\n", port);
    log_info("Server starting");

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

    /* REQ-SVR-010: Accept loop — one verified client at a time. */
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

    /* Clean shutdown path. */
    closesocket(server_fd);
    logger_close();
    WSACleanup();
    return 0;
}
