/**
 * @file main.c
 * @brief ATC Ground Control Server
 * 
 * Functionality:
 *  - Accept one TCP client connection
 *  - Read HANDSHAKE packet and verify it
 *  - Send ACK or ERROR response
 *  - Log all TX/RX events to timestamped file
 *  - Enforce state machine (IDLE -> HANDSHAKE -> ... -> DISCONNECTED)
 *
 *
 * Usage: atc-server <PORT>
 *
 * Regulatory compliance:
 *   - CARs SOR/96-433 Part V (Airworthiness)
 *   - DO-178C DAL-D guidance (deterministic state machine, traceable logging)
 *
 * REQ-SVR-010, REQ-STM-010, REQ-STM-020, REQ-STM-030, REQ-LOG-010
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
 
#include "../../shared/packet.h"
#include "include/server_config.h"
#include "include/state_machine.h"
#include "include/network.h"
 
 
/* ================================================================
 *  Client Session Handler
 * ================================================================ */
 
static void handle_client(SOCKET client_fd) {
    ATCState state = STATE_IDLE;
    ATCState prev  = STATE_IDLE;
    char     aircraft_id[32] = {0};
 
    /* IDLE -> HANDSHAKE */
    prev = state; state = STATE_HANDSHAKE;
    log_state_transition(prev, state, "Client connected, awaiting handshake");
    printf("[ATC] State: %s -> %s\n", state_to_str(prev), state_to_str(state));
 
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
 
        /* REQ-LOG-050 / REQ-SVR-030: Emergency check first. */
        if (hdr.emergency_flag == 1) {
            printf("\n!!! [MAYDAY] EMERGENCY from %s !!!\n\n", hdr.aircraft_id);
            char s[128]; snprintf(s, sizeof(s), "MAYDAY from %s", hdr.aircraft_id);
            log_packet("FROM", type_str, hdr.seq_num, hdr.payload_length, hdr.emergency_flag, s);
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
 
        /* Payload summary for log entry. */
        char summary[256] = "(empty)";
        if (payload && hdr.payload_length > 0) {
            size_t n = hdr.payload_length < 128 ? hdr.payload_length : 128;
            snprintf(summary, sizeof(summary), "%.*s", (int)n, (char *)payload);
        }
        log_packet("FROM", type_str, hdr.seq_num, hdr.payload_length, hdr.emergency_flag, summary);
 
        /* ---- Dispatch ---- */
        switch (hdr.packet_type) {
 
        /* REQ-SYS-080, REQ-SVR-010, REQ-PKT-061 */
        case PKT_HANDSHAKE: {
            if (state != STATE_HANDSHAKE) {
                send_error_pkt(client_fd, "HANDSHAKE not expected in current state", aircraft_id);
                free(payload); continue;
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
                free(payload); continue;
            }
 
            prev = state; state = STATE_TAKEOFF;
            log_state_transition(prev, state, "Handshake verified");
            printf("[ATC] Handshake VERIFIED. State: %s -> %s\n",
                   state_to_str(prev), state_to_str(state));
            send_ack(client_fd, hdr.seq_num, aircraft_id);
            break;
        }
 
        /* Graceful teardown */
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
            free(payload); goto session_end;
        }
 
        /* REQ-STM-030: Reject unhandled / out-of-state packets.*/
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
        log_state_transition(prev, state, "Session ended");
    }
    if (state == STATE_DISCONNECTED) {
        prev = state; state = STATE_IDLE;
        log_state_transition(prev, state, "Session reset");
    }
    printf("[ATC] Session closed for %s. State returned to IDLE.\n",
           aircraft_id[0] ? aircraft_id : "(unknown)");
}
 
 
/* ================================================================
 *  Entry Point
 * ================================================================ */
 

// TODO: Implement main() to set up server socket, accept one client, and call handle_client().

// int main(int argc, char *argv[]) {
//     if (argc < 2) { fprintf(stderr, "Usage: %s <PORT>\n", argv[0]); return 1; }
 
//     int port = atoi(argv[1]);
//     if (port <= 0 || port > 65535) { fprintf(stderr, "Invalid port: %s\n", argv[1]); return 1; }
 
//     WSADATA wsa;
//     if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) { fprintf(stderr, "WSAStartup failed\n"); return 1; }
//     if (logger_init() != 0) { WSACleanup(); return 1; }
 
//     printf("\n========================================\n");
//     printf("   ATC GROUND CONTROL SERVER\n");
//     printf("   CSCN74000 - Group 3\n");
//     printf("   ATC ID: %u  |  Airport: %s\n", SERVER_ATC_ID, SERVER_AIRPORT_CODE);
//     printf("========================================\n");
//     printf("[ATC] Starting on port %d...\n\n", port);
//     log_info("Server starting");
 
//     SOCKET server_fd = socket(AF_INET, SOCK_STREAM, 0);
//     if (server_fd == INVALID_SOCKET) { perror("socket"); logger_close(); WSACleanup(); return 1; }
 
//     int opt = 1;
//     setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, (const char *)&opt, sizeof(opt));
 
//     struct sockaddr_in addr = {0};
//     addr.sin_family      = AF_INET;
//     addr.sin_addr.s_addr = INADDR_ANY;
//     addr.sin_port        = htons((uint16_t)port);
 
//     if (bind(server_fd, (struct sockaddr *)&addr, sizeof(addr)) == SOCKET_ERROR) {
//         perror("bind"); closesocket(server_fd); logger_close(); WSACleanup(); return 1;
//     }
//     if (listen(server_fd, 1) == SOCKET_ERROR) {
//         perror("listen"); closesocket(server_fd); logger_close(); WSACleanup(); return 1;
//     }
 
//     printf("[ATC] Listening on port %d - waiting for aircraft...\n\n", port);
//     log_info("Listening for connections");

// }