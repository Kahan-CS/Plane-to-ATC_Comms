/**
 * @file packet.h
 * @brief Shared ATC Comms packet structure definition.
 *
 * This file defines the binary-compatible packet format used by both the
 * C Server and Rust Client. ALL fields are in network byte order (big-endian).
 * The header is exactly 54 bytes with packed alignment (no padding).
 *
 * Regulatory compliance:
 *   - CARs SOR/96-433 Part V (Airworthiness) - data integrity in safety comms
 *   - DO-178C DAL-D guidance - deterministic data structures, no hidden padding
 *
 * REQ-PKT-030, REQ-PKT-031, REQ-PKT-032
 */

#ifndef PACKET_H
#define PACKET_H

#include <stdint.h>

/* Packet type identifiers (REQ-PKT-010) */
#define PKT_HANDSHAKE 0x01
#define PKT_ACK 0x02
#define PKT_ERROR 0x03
#define PKT_TAKEOFF 0x04
#define PKT_TRANSIT 0x05
#define PKT_LANDING 0x06
#define PKT_MAYDAY 0x07
#define PKT_LARGE_DATA_REQUEST 0x08
#define PKT_LARGE_DATA 0x09
#define PKT_HANDOFF_NOTIFY 0x0A
#define PKT_DISCONNECT 0x0B

/* Fixed header of 54 bytes (packed) (REQ-PKT-031) */
#pragma pack(push, 1)
typedef struct
{
    uint8_t packet_type;     /*  1 byte : one of packet type identifiers above            */
    uint32_t seq_num;        /*  4 bytes: monotonically increasing       */
    int64_t timestamp;       /*  8 bytes: Unix epoch seconds (UTC)       */
    uint32_t payload_length; /*  4 bytes: byte count of payload that follows */
    uint32_t origin_atc_id;  /*  4 bytes: 0 = live; non-zero = buffered handoff (REQ-PKT-020) */
    char aircraft_id[32];    /* 32 bytes: null-terminated callsign       */
    uint8_t emergency_flag;  /*  1 byte : (Simple bool) 0 = normal, 1 = MAYDAY (REQ-PKT-034) */
                             /* Total: 1+4+8+4+4+32+1 = 54 bytes */
} PacketHeader;
#pragma pack(pop)

/* Dynamically allocated full packet (REQ-PKT-033, REQ-SYS-030) */
typedef struct
{
    PacketHeader header;
    uint8_t *payload; /* heap-allocated; size = header.payload_length  */
} Packet;

/* Payload structs (serialized into payload field) */

/* REQ-PKT-061: Handshake payload */
#pragma pack(push, 1)
typedef struct
{
    char callsign[12];       /* e.g. "AC1234" */
    char aircraft_type[16];  /* e.g. "B737"  */
    char aircraft_model[16]; /* e.g. "737-800" */
    char origin[4];          /* ICAO, e.g. "CYYZ" */
    char destination[4];     /* ICAO, e.g. "CAMD" */
    uint8_t initial_phase;   /* 0=TAKEOFF(default), 2=TRANSIT, 3=LANDING */
} HandshakePayload;          /* 53 bytes          */
#pragma pack(pop)

/* REQ-CLT-040: Takeoff telemetry payload */
#pragma pack(push, 1)
typedef struct
{
    int64_t departure_time;   /* Unix epoch    */
    uint8_t clearance_type;   /* 0=IFR, 1=VFR  */
    float assigned_heading;   /* degrees 0-360  */
    float assigned_altitude;  /* feet MSL       */
    uint16_t squawk_code;     /* 4-digit octal code */
    float wind_speed;         /* knots   */
    float wind_direction;     /* degrees */
    float speed_off_runway;   /* knots   */
    float initial_climb_rate; /* feet per minute  */
} TakeoffPayload;             /* Packed as 35 bytes due to 0x1 byte alignment         */
#pragma pack(pop)

/* REQ-CLT-040: Transit telemetry payload */
#pragma pack(push, 1)
typedef struct
{
    float speed;    /* knots                   */
    float altitude; /* feet MSL                */
    float heading;  /* degrees 0-360           */
    uint16_t squawk_code;
} TransitPayload; /* 14 bytes                */
#pragma pack(pop)

/* REQ-CLT-040: Landing telemetry payload */
#pragma pack(push, 1)
typedef struct
{
    float approach_speed;       /* knots                 */
    float current_altitude;     /* feet MSL              */
    float heading;              /* degrees               */
    char assigned_runway[4];    /* e.g. "27L"          */
    uint8_t approach_clearance; /* 0=not cleared, 1=cleared */
    float wind_shear;           /* knots                 */
    float visibility;           /* statute miles         */
} LandingPayload;               /* Packed as 25 bytes due to 0x1 byte alignment     */
#pragma pack(pop)

#endif
