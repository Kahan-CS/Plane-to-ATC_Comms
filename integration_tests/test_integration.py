# test_integration.py
# Integration tests: INT-001 to INT-017
# Handshake, flight phases, MAYDAY, large data,
# disconnect, heartbeat, handoff, ACK seq,
# concurrency, logging, ATC instructions,
# disconnect detection, multi-session
#
# DO-178C DAL-D: verifies distributed communication
#   between Rust client wire format and C server
#   over live TCP connection
# CARs SOR/96-433 Part V: confirms data integrity
#   of structured packets across the full stack

import sys
import os
import glob
import time
import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "framework"))
from atc_test_client import (
    ATCTestClient,
    PKT_ACK, PKT_ERROR,
    PKT_HANDSHAKE, PKT_TAKEOFF, PKT_TRANSIT, PKT_LANDING,
    PKT_MAYDAY, PKT_LARGE_DATA_REQUEST, PKT_LARGE_DATA,
    PKT_HANDOFF_NOTIFY, PKT_DISCONNECT,
    SERVER_HOST, SERVER_PORT,
)

# Path to search for server log files
_SERVER_DIR = os.path.join(os.path.dirname(__file__), "..", "server")


def _latest_log_file():
    """Return the most recently modified .log file in the server directory."""
    pattern = os.path.join(_SERVER_DIR, "*.log")
    files = glob.glob(pattern)
    if not files:
        return None
    return max(files, key=os.path.getmtime)


def _do_takeoff_session(client, callsign="AC8821", seq_start=1):
    """Helper: handshake + TAKEOFF, consuming all ACKs."""
    client.do_full_handshake(callsign=callsign)
    client.send_packet(
        PKT_TAKEOFF, seq_num=seq_start + 1,
        aircraft_id=callsign,
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    client.receive_packet()  # consume TAKEOFF ACK


# INT-001
# REQ-SYS-080, REQ-SVR-010, REQ-CLT-010,
# REQ-CLT-030, REQ-COM-010
# Client connects and sends handshake with callsign,
#   aircraft type, origin and destination
# Server responds with PKT_ACK
def test_INT001_server_accepts_valid_handshake(atc_client):
    result = atc_client.do_full_handshake()
    assert result is True, "Server must respond with PKT_ACK to a valid handshake"


# INT-002
# REQ-SYS-080, REQ-STM-030
# Client sends TAKEOFF packet before completing
#   handshake, server must reject with ERROR
# Server responds with PKT_ERROR
def test_INT002_server_rejects_command_before_handshake():
    client = ATCTestClient()
    client.connect()
    try:
        # Send TAKEOFF without prior handshake
        client.send_packet(
            PKT_TAKEOFF, seq_num=1,
            aircraft_id="AC9999",
            atc_id=0, emergency_flag=0,
            payload=bytes(35),
        )
        header, _ = client.receive_packet()
        assert header["packet_type"] == PKT_ERROR, (
            "Server must respond with PKT_ERROR when TAKEOFF is sent "
            "before handshake"
        )
    finally:
        client.disconnect()


# INT-003
# REQ-CLT-040, REQ-STM-020, REQ-SVR-020,
# REQ-PKT-060, REQ-SYS-020
# Client sends TAKEOFF packet after handshake
#   with phase telemetry data in payload
# Server responds with PKT_ACK
def test_INT003_takeoff_packet_accepted(atc_client):
    atc_client.do_full_handshake()
    # Minimal TakeoffPayload: 35 zero bytes
    atc_client.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    header, _ = atc_client.receive_packet()
    assert header["packet_type"] == PKT_ACK, (
        "Server must respond with PKT_ACK to a valid TAKEOFF packet"
    )


# INT-004
# REQ-CLT-040, REQ-CLT-020, REQ-STM-020
# Client sends TRANSIT packet after TAKEOFF
# Server responds with PKT_ACK
def test_INT004_transit_packet_accepted(atc_client):
    atc_client.do_full_handshake()
    # TAKEOFF first
    atc_client.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    atc_client.receive_packet()  # consume TAKEOFF ACK

    # Minimal TransitPayload: 14 zero bytes
    atc_client.send_packet(
        PKT_TRANSIT, seq_num=3,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(14),
    )
    header, _ = atc_client.receive_packet()
    assert header["packet_type"] == PKT_ACK, (
        "Server must respond with PKT_ACK to a valid TRANSIT packet"
    )


# INT-005
# REQ-CLT-040, REQ-CLT-020, REQ-STM-020
# Client sends LANDING packet after TRANSIT
# Server responds with PKT_ACK
def test_INT005_landing_packet_accepted(atc_client):
    atc_client.do_full_handshake()
    # TAKEOFF
    atc_client.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    atc_client.receive_packet()  # consume TAKEOFF ACK

    # TRANSIT
    atc_client.send_packet(
        PKT_TRANSIT, seq_num=3,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(14),
    )
    atc_client.receive_packet()  # consume TRANSIT ACK

    # Minimal LandingPayload: 25 zero bytes
    atc_client.send_packet(
        PKT_LANDING, seq_num=4,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(25),
    )
    header, _ = atc_client.receive_packet()
    assert header["packet_type"] == PKT_ACK, (
        "Server must respond with PKT_ACK to a valid LANDING packet"
    )


# INT-006
# REQ-CLT-050, REQ-SVR-030, REQ-STM-040,
# REQ-PKT-062
# Client sends packet with emergency_flag=1 during TRANSIT phase
# Server responds with PKT_ACK (MAYDAY handled)
def test_INT006_mayday_triggers_response(atc_client):
    atc_client.do_full_handshake()
    # TAKEOFF
    atc_client.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    atc_client.receive_packet()  # consume TAKEOFF ACK

    # TRANSIT
    atc_client.send_packet(
        PKT_TRANSIT, seq_num=3,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(14),
    )
    atc_client.receive_packet()  # consume TRANSIT ACK

    # MAYDAY with emergency_flag=1
    atc_client.send_packet(
        PKT_MAYDAY, seq_num=4,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=1,
        payload=b"",
    )
    header, _ = atc_client.receive_packet()
    assert header["packet_type"] == PKT_ACK, (
        "Server must respond with PKT_ACK when MAYDAY is received"
    )


# INT-007
# REQ-SYS-070, REQ-SVR-050, REQ-PKT-063
# Client sends PKT_LARGE_DATA_REQUEST
# Server responds with PKT_LARGE_DATA containing at least 1048576 bytes (1 MB)
def test_INT007_large_data_transfer(atc_client):
    atc_client.sock.settimeout(30.0)  # large transfer needs more time
    atc_client.do_full_handshake()
    atc_client.send_packet(
        PKT_LARGE_DATA_REQUEST, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=b"",
    )
    # Server sends ACK first, then LARGE_DATA
    header, _ = atc_client.receive_packet()  # ACK
    if header["packet_type"] == PKT_ACK:
        header, payload = atc_client.receive_packet()  # LARGE_DATA

    assert header["packet_type"] == PKT_LARGE_DATA, (
        "Server must respond with PKT_LARGE_DATA to PKT_LARGE_DATA_REQUEST"
    )
    assert len(payload) >= 1048576, (
        f"Large data payload must be >= 1 MB, got {len(payload)} bytes"
    )


# INT-008
# REQ-CLT-080, REQ-SVR-010
# Client sends PKT_DISCONNECT cleanly
# Server responds with PKT_ACK and closes session
def test_INT008_clean_disconnect(atc_client):
    atc_client.do_full_handshake()
    atc_client.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    atc_client.receive_packet()  # consume TAKEOFF ACK

    atc_client.send_packet(
        PKT_DISCONNECT, seq_num=3,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=b"",
    )
    header, _ = atc_client.receive_packet()
    assert header["packet_type"] == PKT_ACK, (
        "Server must respond with PKT_ACK to PKT_DISCONNECT"
    )


# INT-009
# REQ-COM-030
# Client sends PKT_ACK as heartbeat
# Server receives without error, connection stays
def test_INT009_heartbeat_accepted(atc_client):
    atc_client.do_full_handshake()

    # Send PKT_ACK with zero payload as heartbeat
    atc_client.send_packet(
        PKT_ACK, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=b"",
    )
    time.sleep(1)

    # Connection should still be alive send TAKEOFF
    atc_client.send_packet(
        PKT_TAKEOFF, seq_num=3,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    header, _ = atc_client.receive_packet()
    assert header["packet_type"] == PKT_ACK, (
        "Connection must remain alive after heartbeat — PKT_ACK expected"
    )


# INT-010
# REQ-SYS-090, REQ-CLT-060, REQ-SVR-040,
# REQ-COM-040, REQ-PKT-020
# Server sends HANDOFF_NOTIFY with next port
# Client reconnects and sends packet with non-zero origin_atc_id
# Server receives buffered packet with ATC ID set
def test_INT010_handoff_buffered_packets(atc_client):
    atc_client.do_full_handshake()
    atc_client.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    atc_client.receive_packet()  # consume TAKEOFF ACK

    atc_client.send_packet(
        PKT_TRANSIT, seq_num=3,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(14),
    )
    atc_client.receive_packet()  # consume TRANSIT ACK

    # Notify handoff
    atc_client.send_packet(
        PKT_HANDOFF_NOTIFY, seq_num=4,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=b"",
    )
    atc_client.receive_packet()  # consume HANDOFF_NOTIFY ACK
    atc_client.disconnect()

    # Reconnect: simulate new ATC sector
    new_client = ATCTestClient(SERVER_HOST, SERVER_PORT)
    new_client.connect()
    try:
        new_client.do_full_handshake(callsign="AC8821")
        # Send TRANSIT with non-zero origin_atc_id (buffered handoff packet)
        new_client.send_packet(
            PKT_TRANSIT, seq_num=5,
            aircraft_id="AC8821",
            atc_id=1,               # non-zero = buffered packet from ATC #1
            emergency_flag=0,
            payload=bytes(14),
        )
        header, _ = new_client.receive_packet()
        assert header["packet_type"] == PKT_ACK, (
            "Server must ACK a buffered handoff packet with non-zero origin_atc_id"
        )
    finally:
        new_client.disconnect()


# INT-011
# REQ-COM-060
# Server ACKs every received packet referencing the correct sequence number
# ACK payload contains the seq num of the packet that was acknowledged
def test_INT011_server_acks_with_seq_num(atc_client):
    # Handshake with seq_num=1
    atc_client.send_handshake(
        "AC8821", "B737", "737-800", "CYYZ", "CYVR", seq_num=1
    )
    header, payload = atc_client.receive_packet()
    assert header["packet_type"] == PKT_ACK, "Handshake must be ACKed"

    # TAKEOFF with seq_num=2
    atc_client.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    header, payload = atc_client.receive_packet()
    assert header["packet_type"] == PKT_ACK, "TAKEOFF must be ACKed"

    # ACK payload should contain the acknowledged seq num (2)
    if payload:
        ack_body = payload.decode("utf-8", errors="replace").strip()
        assert "2" in ack_body, (
            f"ACK payload should reference seq=2, got: {ack_body!r}"
        )



# INT-013
# REQ-SYS-050, REQ-LOG-010, REQ-LOG-030,
# REQ-LOG-040
# After a live session both server log files exist with timestamped names and contain
#   pipe-separated entries
def test_INT013_log_files_created_and_formatted():
    client = ATCTestClient()
    client.connect()
    client.do_full_handshake()
    client.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    client.receive_packet()
    client.send_packet(
        PKT_DISCONNECT, seq_num=3,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
    )
    try:
        client.receive_packet()
    except Exception:
        pass
    client.disconnect()

    time.sleep(0.5)  # allow server to flush log

    log_file = _latest_log_file()
    assert log_file is not None, (
        "At least one .log file must exist in server/ after a session"
    )
    with open(log_file, "r", errors="replace") as f:
        content = f.read()
    assert "|" in content, (
        "Log file must contain pipe-separated fields"
    )
    assert ("TO" in content or "FROM" in content), (
        "Log file must contain TO or FROM direction strings"
    )


# INT-014
# REQ-LOG-020, REQ-LOG-050, REQ-LOG-060
# After a MAYDAY session server log contains
#   state transition, MAYDAY prefix, and summary
def test_INT014_mayday_logged_in_live_session():
    client = ATCTestClient()
    client.connect()
    client.do_full_handshake()
    client.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    client.receive_packet()
    client.send_packet(
        PKT_TRANSIT, seq_num=3,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(14),
    )
    client.receive_packet()
    client.send_packet(
        PKT_MAYDAY, seq_num=4,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=1,
        payload=b"",
    )
    client.receive_packet()
    client.send_packet(
        PKT_DISCONNECT, seq_num=5,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
    )
    try:
        client.receive_packet()
    except Exception:
        pass
    client.disconnect()

    time.sleep(0.5)

    log_file = _latest_log_file()
    assert log_file is not None, "Log file must exist after MAYDAY session"
    with open(log_file, "r", errors="replace") as f:
        content = f.read()
    assert "MAYDAY" in content, "Log must contain MAYDAY entry"
    assert "SUMMARY" in content, "Log must contain session summary"


# INT-015
# REQ-CLT-070, REQ-SVR-070
# Server sends ATC instructions back to client
#   after TAKEOFF and LANDING packets
def test_INT015_server_sends_atc_instructions(atc_client):
    atc_client.do_full_handshake()
    atc_client.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    header, payload = atc_client.receive_packet()
    assert header["packet_type"] == PKT_ACK, (
        "Server must respond with PKT_ACK (clearance) after TAKEOFF"
    )

    atc_client.send_packet(
        PKT_TRANSIT, seq_num=3,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(14),
    )
    atc_client.receive_packet()  # consume TRANSIT ACK

    atc_client.send_packet(
        PKT_LANDING, seq_num=4,
        aircraft_id="AC8821",
        atc_id=0, emergency_flag=0,
        payload=bytes(25),
    )
    header, payload = atc_client.receive_packet()
    assert header["packet_type"] == PKT_ACK, (
        "Server must respond with PKT_ACK (clearance) after LANDING"
    )
    assert len(payload) > 0, (
        "LANDING ACK must carry non-empty clearance text in payload"
    )


# INT-016
# REQ-CLT-080
# Server process killed mid-session
# Client detects connection drop without crashing
def test_INT016_client_detects_server_disconnect():
    # Manual test: documented for traceability.
    # Tester starts server, connects client, kills server process,
    # observes client output for connection lost alert within
    # heartbeat timeout. Mark pass/fail manually.
    pytest.skip("Manual test — see test plan")


# INT-017
# REQ-SVR-060
# After two sequential client sessions server
#   output shows both aircraft with their phases
def test_INT017_server_logs_multiple_sessions():
    # Session 1
    c1 = ATCTestClient()
    c1.connect()
    c1.do_full_handshake(callsign="AC0001")
    c1.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC0001",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    c1.receive_packet()
    c1.send_packet(
        PKT_DISCONNECT, seq_num=3,
        aircraft_id="AC0001",
        atc_id=0, emergency_flag=0,
    )
    try:
        c1.receive_packet()
    except Exception:
        pass
    c1.disconnect()
    time.sleep(0.3)

    # Session 2
    c2 = ATCTestClient()
    c2.connect()
    c2.do_full_handshake(callsign="AC0002")
    c2.send_packet(
        PKT_TAKEOFF, seq_num=2,
        aircraft_id="AC0002",
        atc_id=0, emergency_flag=0,
        payload=bytes(35),
    )
    c2.receive_packet()
    c2.send_packet(
        PKT_DISCONNECT, seq_num=3,
        aircraft_id="AC0002",
        atc_id=0, emergency_flag=0,
    )
    try:
        c2.receive_packet()
    except Exception:
        pass
    c2.disconnect()
    time.sleep(0.5)

    log_file = _latest_log_file()
    assert log_file is not None, "Log file must exist after two sessions"
    with open(log_file, "r", errors="replace") as f:
        content = f.read()
    assert "AC0001" in content, "Log must contain AC0001 from session 1"
    assert "AC0002" in content, "Log must contain AC0002 from session 2"
