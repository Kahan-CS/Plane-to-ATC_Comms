# ATC Ground Control Server

A TCP-based Air Traffic Control (ATC) ground server that manages aircraft communication throughout all flight phases — from handshake to landing — with state machine enforcement and compliance-grade audit logging.

---

## Overview

The server listens for incoming aircraft client connections and guides each aircraft through a defined sequence of flight phases using a structured binary packet protocol. It validates state transitions, parses telemetry, issues ATC clearances, handles MAYDAY emergencies, and logs all activity to a per-session log file.

**Airport:** CYYZ (Toronto Pearson International)  
**ATC ID:** 1  
**Protocol:** TCP/IP with 54-byte packed binary headers (network byte order)

---

## Project Structure

```
server/
└── src/
    ├── main.c               # Server entry point, session handler, ATC controller input
    └── include/
        ├── server_config.h   # Configuration constants (ATC ID, airport code, limits)
        ├── state_machine.h   # FSM state definitions and transition validation
        ├── network.h         # TCP I/O, byte-order conversion, packet builders
        └── logger.h          # Per-session audit logging system
shared/
└── packet.h                  # Binary packet format (shared with client)
```

---

## Building

**Prerequisites:**
- GCC (MinGW-w64) on Windows
- Windows Sockets (`ws2_32`)

**Compile:**
```bash
cd server/src
gcc main.c -o main -lws2_32
```

---

## Running

```bash
.\main.exe <PORT>
```

**Example:**
```bash
.\main.exe 5555
```

The server will:
1. Print a startup banner with ATC ID and airport code
2. Create a timestamped session log file
3. Wait for an aircraft client to connect
4. Verify the client via handshake and process flight phases
5. Return to listening after the session ends

The server handles one aircraft connection at a time.

---

## ATC Controller Input

While an aircraft is connected, the ATC controller can press **D** on the server terminal at any time to force-disconnect the aircraft. The server uses a non-blocking input check (`select` with 1-second timeout) so keyboard input does not interfere with packet processing.

---

## State Machine

The server enforces a deterministic Finite State Machine for each aircraft session. The client drives state transitions by sending the appropriate packet type. Invalid or out-of-order packets are rejected with an ERROR response.

```
IDLE ──> HANDSHAKE ──> TAKEOFF ──> TRANSIT ──> LANDING ──> DISCONNECTED ──> IDLE
```

**Transition rules:**
- **IDLE → HANDSHAKE:** Automatic when a client connects
- **HANDSHAKE → TAKEOFF:** Automatic after handshake is verified
- **TAKEOFF → TRANSIT:** When the client sends `PKT_TRANSIT`
- **TRANSIT → LANDING:** When the client sends `PKT_LANDING`
- **LANDING → DISCONNECTED:** When the client sends `PKT_DISCONNECT`
- **DISCONNECTED → IDLE:** Automatic reset, ready for next aircraft

**MAYDAY** can be triggered from any active flight state (TAKEOFF, TRANSIT, or LANDING) via the `emergency_flag` in the packet header or a `PKT_MAYDAY` packet. MAYDAY transitions to DISCONNECTED on disconnect.

The server stays in the current phase until the client explicitly sends the next phase's packet. For example, multiple `PKT_TAKEOFF` packets can be sent while in TAKEOFF — the server only advances to TRANSIT when it receives `PKT_TRANSIT`.

---

## Packet Protocol

### Header Format (54 bytes, packed, network byte order)

| Field            | Type       | Size     | Description                          |
|------------------|------------|----------|--------------------------------------|
| `packet_type`    | `uint8_t`  | 1 byte   | Identifies the packet type           |
| `seq_num`        | `uint32_t` | 4 bytes  | Monotonically increasing sequence    |
| `timestamp`      | `int64_t`  | 8 bytes  | Unix epoch timestamp (ms)            |
| `payload_length` | `uint32_t` | 4 bytes  | Byte length of the payload           |
| `origin_atc_id`  | `uint32_t` | 4 bytes  | 0 = live; >0 = buffered handoff      |
| `aircraft_id`    | `char[32]` | 32 bytes | Null-terminated aircraft callsign    |
| `emergency_flag` | `uint8_t`  | 1 byte   | 0 = normal; 1 = MAYDAY              |

All multi-byte fields are transmitted in network byte order (big-endian). The server converts to/from host byte order using `header_ntoh()` and `header_hton()` in `network.h`.

### Packet Types

| Constant                 | Value  | Direction       | Description                      |
|--------------------------|--------|-----------------|----------------------------------|
| `PKT_HANDSHAKE`          | `0x01` | Client → Server | Aircraft registration (52 bytes) |
| `PKT_ACK`                | `0x02` | Server → Client | Acknowledgment                   |
| `PKT_ERROR`              | `0x03` | Server → Client | Error response                   |
| `PKT_TAKEOFF`            | `0x04` | Client → Server | Departure telemetry (41 bytes)   |
| `PKT_TRANSIT`            | `0x05` | Client → Server | En-route telemetry (14 bytes)    |
| `PKT_LANDING`            | `0x06` | Client → Server | Approach telemetry (27 bytes)    |
| `PKT_MAYDAY`             | `0x07` | Client → Server | Emergency declaration            |
| `PKT_LARGE_DATA_REQUEST` | `0x08` | Client → Server | Request weather/ATIS data        |
| `PKT_LARGE_DATA`         | `0x09` | Server → Client | Weather/ATIS response (≥1 MB)    |
| `PKT_HANDOFF_NOTIFY`     | `0x0A` | Either          | Inter-ATC handoff notification   |
| `PKT_DISCONNECT`         | `0x0B` | Client → Server | Graceful session termination     |

---

## Logging

Each session creates a log file named `ATC_SERVER_YYYYMMDD_HHMMSS.log` in the working directory. All writes are flushed immediately to prevent data loss on crash.

### Log Entry Prefixes

| Prefix      | Description                                            |
|-------------|--------------------------------------------------------|
| `[INFO]`    | Server events (startup, shutdown, connections)         |
| `[ERROR]`   | Protocol errors, invalid transitions, parse failures   |
| `[STATE]`   | State transitions with previous state and trigger      |
| `[MAYDAY]`  | Emergency events (prefixed on packet log entries)      |
| `[SUMMARY]` | End-of-session statistics (RX/TX counts, transitions)  |

### Packet Log Format (REQ-LOG-030)

```
[APP] | [TIMESTAMP] | [DIRECTION] | [PACKET_TYPE] | SEQ=[N] | LEN=[N] | [SUMMARY]
```

### Example Session Log

```
=== ATC Server Session Start: 2026-04-06 14:09:58.764 ===
[INFO]  ATC_SERVER | 2026-04-06 14:09:58.772 | Server starting
[STATE] ATC_SERVER | 2026-04-06 14:10:03.808 | IDLE -> HANDSHAKE | Trigger: Client connected, awaiting handshake
ATC_SERVER | 2026-04-06 14:10:03.812 | FROM | HANDSHAKE | SEQ=1 | LEN=52 | AC8821...
[STATE] ATC_SERVER | 2026-04-06 14:10:03.815 | HANDSHAKE -> TAKEOFF | Trigger: Handshake verified
ATC_SERVER | 2026-04-06 14:10:10.200 | FROM | TAKEOFF | SEQ=2 | LEN=41 | ...
[STATE] ATC_SERVER | 2026-04-06 14:10:15.500 | TAKEOFF -> TRANSIT | Trigger: Transit packet received
[MAYDAY] ATC_SERVER | 2026-04-06 14:10:20.000 | FROM | MAYDAY | SEQ=4 | LEN=0 | MAYDAY from AC8821
[STATE] ATC_SERVER | 2026-04-06 14:10:20.001 | TRANSIT -> MAYDAY | Trigger: Emergency flag set in packet header
[INFO]  ATC_SERVER | 2026-04-06 14:10:25.049 | Server shutting down
=== Session Summary ===
Packets RX: 5  TX: 6  Errors: 0  Transitions: 8
=== Session End ===
```

## Building

**Prerequisites:**
- CMake 3.10+
- GCC (MinGW-w64) or MSVC on Windows
- Windows Sockets (`ws2_32`)

### Using CMake

```bash
cd server
mkdir build
cd build
cmake ..
cmake --build .
```

The executable lands in `build/debug/server.exe`. Run it with:

```bash
.\Debug\server.exe 5555
```
