/**
 * @file  network.h
 * @brief TCP networking helpers and packet send/receive.
 *
 * REQ-COM-010 (TCP/IP), REQ-COM-060 (ACK), REQ-STM-030 (ERROR),
 * REQ-PKT-034 (header-first receive), REQ-SVR-050 (large data),
 * REQ-SVR-070 (ATC clearance instructions)
 */

#ifndef NET_H
#define NET_H

#ifndef _WIN32_WINNT
    #define _WIN32_WINNT 0x0600
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <winsock2.h>
#include <ws2tcpip.h>
#pragma comment(lib, "ws2_32.lib")

#include "../../../shared/packet.h"
#include "server_config.h"
#include "logger.h"

/* ---- Packet type -> string ------------------------------------- */

static inline const char *packet_type_str(uint8_t type) {
    switch (type) {
        case PKT_HANDSHAKE:          return "HANDSHAKE";
        case PKT_ACK:                return "ACK";
        case PKT_ERROR:              return "ERROR";
        case PKT_TAKEOFF:            return "TAKEOFF";
        case PKT_TRANSIT:            return "TRANSIT";
        case PKT_LANDING:            return "LANDING";
        case PKT_MAYDAY:             return "MAYDAY";
        case PKT_LARGE_DATA_REQUEST: return "LARGE_DATA_REQUEST";
        case PKT_LARGE_DATA:         return "LARGE_DATA";
        case PKT_HANDOFF_NOTIFY:     return "HANDOFF_NOTIFY";
        case PKT_DISCONNECT:         return "DISCONNECT";
        default:                     return "UNKNOWN";
    }
}

/* ---- Time ------------------------------------------------------ */

static inline int64_t now_ms(void) {
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    uint64_t t = ((uint64_t)ft.dwHighDateTime << 32) | ft.dwLowDateTime;
    t -= 116444736000000000ULL;
    return (int64_t)(t / 10000);
}

/* ---- TCP I/O --------------------------------------------------- */

static inline int send_all(SOCKET sockfd, const void *buf, size_t len) {
    const char *ptr = (const char *)buf;
    size_t total = 0;
    while (total < len) {
        int sent = send(sockfd, ptr + total, (int)(len - total), 0);
        if (sent <= 0) return -1;
        total += (size_t)sent;
    }
    return 0;
}

static inline int recv_all(SOCKET sockfd, void *buf, size_t len) {
    char *ptr = (char *)buf;
    size_t total = 0;
    while (total < len) {
        int received = recv(sockfd, ptr + total, (int)(len - total), 0);
        if (received <= 0) return -1;
        total += (size_t)received;
    }
    return 0;
}

/* ---- Byte-order conversion (network BE <-> host LE) ------------ */

static inline uint64_t swap64(uint64_t v) {
    return ((v & 0x00000000000000FFULL) << 56) |
           ((v & 0x000000000000FF00ULL) << 40) |
           ((v & 0x0000000000FF0000ULL) << 24) |
           ((v & 0x00000000FF000000ULL) <<  8) |
           ((v & 0x000000FF00000000ULL) >>  8) |
           ((v & 0x0000FF0000000000ULL) >> 24) |
           ((v & 0x00FF000000000000ULL) >> 40) |
           ((v & 0xFF00000000000000ULL) >> 56);
}

/* Convert received header from network (big-endian) to host order. */
static inline void header_ntoh(PacketHeader *h) {
    h->seq_num        = ntohl(h->seq_num);
    h->timestamp      = (int64_t)swap64((uint64_t)h->timestamp);
    h->payload_length = ntohl(h->payload_length);
    h->origin_atc_id  = ntohl(h->origin_atc_id);
}

/* Convert header from host to network (big-endian) before sending. */
static inline void header_hton(PacketHeader *h) {
    h->seq_num        = htonl(h->seq_num);
    h->timestamp      = (int64_t)swap64((uint64_t)h->timestamp);
    h->payload_length = htonl(h->payload_length);
    h->origin_atc_id  = htonl(h->origin_atc_id);
}

/* ---- Packet-level I/O ------------------------------------------ */

/* Send header in network byte order + payload in raw bytes. */
static inline int send_packet(SOCKET sockfd, const PacketHeader *hdr, const void *payload) {
    uint32_t host_payload_len = hdr->payload_length;
    PacketHeader wire = *hdr;
    header_hton(&wire);
    if (send_all(sockfd, &wire, sizeof(PacketHeader)) != 0) return -1;
    if (host_payload_len > 0 && payload)
        if (send_all(sockfd, payload, host_payload_len) != 0) return -1;
    return 0;
}

/* Receive header (converted to host order) + payload. Caller must free(*out_payload). */
static inline int recv_packet(SOCKET sockfd, PacketHeader *hdr, uint8_t **out_payload) {
    *out_payload = NULL;
    if (recv_all(sockfd, hdr, sizeof(PacketHeader)) != 0) return -1;
    header_ntoh(hdr);
    if (hdr->payload_length > MAX_PAYLOAD_BYTES)           return -2;
    if (hdr->payload_length > 0) {
        *out_payload = (uint8_t *)malloc(hdr->payload_length);
        if (!*out_payload) return -3;
        if (recv_all(sockfd, *out_payload, hdr->payload_length) != 0) {
            free(*out_payload); *out_payload = NULL; return -1;
        }
    }
    return 0;
}

/* ---- Header builder -------------------------------------------- */

static uint32_t g_server_seq = 1;

static inline void build_header(PacketHeader *hdr, uint8_t type,
                                 uint32_t payload_len, const char *aircraft_id) {
    memset(hdr, 0, sizeof(PacketHeader));
    hdr->packet_type    = type;
    hdr->seq_num        = g_server_seq++;
    hdr->timestamp      = now_ms();
    hdr->payload_length = payload_len;
    hdr->origin_atc_id  = SERVER_ATC_ID;
    hdr->emergency_flag = 0;
    if (aircraft_id) {
        strncpy(hdr->aircraft_id, aircraft_id, 31);
        hdr->aircraft_id[31] = '\0';
    }
}

/* ---- Response packets ------------------------------------------ */

/* REQ-COM-060: ACK carrying acknowledged sequence number. */
static inline int send_ack(SOCKET sockfd, uint32_t ack_seq, const char *aircraft_id) {
    char payload[32];
    int plen = snprintf(payload, sizeof(payload), "%u", ack_seq);

    PacketHeader hdr;
    build_header(&hdr, PKT_ACK, (uint32_t)plen, aircraft_id);
    int rc = send_packet(sockfd, &hdr, payload);

    char summary[64];
    snprintf(summary, sizeof(summary), "ACK for seq=%u", ack_seq);
    log_packet("TO", packet_type_str(hdr.packet_type), hdr.seq_num, hdr.payload_length, hdr.emergency_flag, summary);
    if (rc == 0) printf("[ATC] Sent ACK for seq=%u\n", ack_seq);
    else         log_error("Failed to send ACK");
    return rc;
}

/* REQ-STM-030: ERROR with descriptive message. */
static inline int send_error_pkt(SOCKET sockfd, const char *msg, const char *aircraft_id) {
    PacketHeader hdr;
    build_header(&hdr, PKT_ERROR, (uint32_t)strlen(msg), aircraft_id);
    int rc = send_packet(sockfd, &hdr, msg);

    char summary[256];
    snprintf(summary, sizeof(summary), "ERROR: %s", msg);
    log_packet("TO", packet_type_str(hdr.packet_type), hdr.seq_num, hdr.payload_length, hdr.emergency_flag, summary);
    log_error(msg);
    if (rc == 0) printf("[ATC] Sent ERROR: %s\n", msg);
    return rc;
}

/* REQ-SVR-070: Send phase-appropriate ATC instruction text to the client. */
static inline int send_atc_clearance(SOCKET sockfd, const char *instruction,
                                      const char *aircraft_id) {
    uint32_t len = (uint32_t)strlen(instruction);
    PacketHeader hdr;
    build_header(&hdr, PKT_ACK, len, aircraft_id);
    int rc = send_packet(sockfd, &hdr, instruction);

    char summary[256];
    snprintf(summary, sizeof(summary), "ATC CLEARANCE: %.200s", instruction);
    log_packet("TO", packet_type_str(PKT_ACK), hdr.seq_num, len, 0, summary);

    if (rc == 0) printf("[ATC] Clearance: %s\n", instruction);
    else         log_error("send_atc_clearance: send failed");
    return rc;
}

/* REQ-SVR-050: Transmit large weather/telemetry data (>= 1 MB). */
static inline int send_large_data(SOCKET sockfd, const char *aircraft_id) {
    const uint32_t DATA_SIZE = 1024u * 1024u + 512u;

    uint8_t *data = (uint8_t *)malloc(DATA_SIZE);
    if (!data) {
        log_error("send_large_data: malloc failed");
        return -1;
    }

    static const char WEATHER_TEMPLATE[] =
        "ATIS CYYZ INFO ALPHA: WIND 270 AT 15KT, VIS 10SM, FEW CLOUDS 3000FT, "
        "SCT 6000FT, BKN 25000FT, TEMP 12 DEW POINT 08, ALTIMETER 2992, "
        "REMARKS: RWY 23L/05R IN USE, ILS APPROACH, BIRD ACTIVITY MEDIUM. ";
    size_t  tlen   = sizeof(WEATHER_TEMPLATE) - 1;
    uint32_t offset = 0;

    while (offset + (uint32_t)tlen <= DATA_SIZE) {
        memcpy(data + offset, WEATHER_TEMPLATE, tlen);
        offset += (uint32_t)tlen;
    }
    if (offset < DATA_SIZE)
        memset(data + offset, 0x20, DATA_SIZE - offset);

    PacketHeader hdr;
    build_header(&hdr, PKT_LARGE_DATA, DATA_SIZE, aircraft_id);
    int rc = send_packet(sockfd, &hdr, data);
    free(data);

    char info[64];
    snprintf(info, sizeof(info), "Large weather data: %u bytes", DATA_SIZE);
    log_packet("TO", packet_type_str(PKT_LARGE_DATA), hdr.seq_num, DATA_SIZE, 0, info);

    if (rc == 0) printf("[ATC] Sent large data (%u bytes) to %s\n", DATA_SIZE, aircraft_id);
    else         log_error("send_large_data: send failed");
    return rc;
}

#endif /* NET_H */