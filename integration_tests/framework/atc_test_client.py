# atc_test_client.py
# Integration test helper — reusable TCP client
#
# INT-001 to INT-017
# REQ-SYS-080, REQ-COM-010, REQ-PKT-030
# DO-178C DAL-D: integration test framework must
#   send and receive binary-compatible packets
#   matching the production wire format exactly
# CARs SOR/96-433 Part V: data integrity verified
#   end-to-end across distributed application

import socket
import struct
import time

# Packet type constants — must match shared/packet.h exactly
PKT_HANDSHAKE          = 0x01
PKT_ACK                = 0x02
PKT_ERROR              = 0x03
PKT_TAKEOFF            = 0x04
PKT_TRANSIT            = 0x05
PKT_LANDING            = 0x06
PKT_MAYDAY             = 0x07
PKT_LARGE_DATA_REQUEST = 0x08
PKT_LARGE_DATA         = 0x09
PKT_HANDOFF_NOTIFY     = 0x0A
PKT_DISCONNECT         = 0x0B

HEADER_SIZE  = 54
SERVER_HOST  = "127.0.0.1"
SERVER_PORT  = 9000

# PacketHeader wire layout (network byte order / big-endian):
#   packet_type    : B  (1 byte)
#   seq_num        : I  (4 bytes)
#   timestamp      : q  (8 bytes)
#   payload_length : I  (4 bytes)
#   origin_atc_id  : I  (4 bytes)
#   aircraft_id    : 32s (32 bytes)
#   emergency_flag : B  (1 byte)
#   Total          : 54 bytes
HEADER_FMT = "!BIqII32sB"


class ATCTestClient:
    def __init__(self, host=SERVER_HOST, port=SERVER_PORT):
        self.host = host
        self.port = port
        self.sock = None
        self._seq = 0

    def connect(self):
        """Open TCP socket to the ATC server with a 5-second timeout."""
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.settimeout(5.0)
        self.sock.connect((self.host, self.port))

    def disconnect(self):
        """Close the TCP socket."""
        if self.sock:
            try:
                self.sock.close()
            except Exception:
                pass
            self.sock = None

    def build_header(self, packet_type, seq_num, aircraft_id,
                     atc_id, emergency_flag, payload_length):
        """
        Build a 54-byte packed header in network byte order.

        aircraft_id is a str or bytes — zero-padded to 32 bytes.
        Returns raw bytes.
        """
        if isinstance(aircraft_id, str):
            aircraft_id = aircraft_id.encode("utf-8")
        aircraft_id = aircraft_id[:32].ljust(32, b"\x00")

        ts = int(time.time())
        return struct.pack(
            HEADER_FMT,
            packet_type,
            seq_num,
            ts,
            payload_length,
            atc_id,
            aircraft_id,
            emergency_flag,
        )

    def send_packet(self, packet_type, seq_num, aircraft_id,
                    atc_id, emergency_flag, payload=b""):
        """Build and send header + payload over the TCP socket."""
        header = self.build_header(
            packet_type, seq_num, aircraft_id,
            atc_id, emergency_flag, len(payload)
        )
        self.sock.sendall(header)
        if payload:
            self.sock.sendall(payload)

    def _recv_exact(self, n):
        """Read exactly n bytes from the socket."""
        buf = b""
        while len(buf) < n:
            chunk = self.sock.recv(n - len(buf))
            if not chunk:
                raise ConnectionError("Server closed connection")
            buf += chunk
        return buf

    def receive_packet(self):
        """
        Read one full packet from the server.
        Returns (header_dict, payload_bytes).
        header_dict keys: packet_type, seq_num, timestamp,
                          payload_length, origin_atc_id,
                          aircraft_id, emergency_flag
        """
        raw_header = self._recv_exact(HEADER_SIZE)
        fields = struct.unpack(HEADER_FMT, raw_header)
        header = {
            "packet_type":    fields[0],
            "seq_num":        fields[1],
            "timestamp":      fields[2],
            "payload_length": fields[3],
            "origin_atc_id":  fields[4],
            "aircraft_id":    fields[5].rstrip(b"\x00").decode("utf-8", errors="replace"),
            "emergency_flag": fields[6],
        }
        payload = b""
        if header["payload_length"] > 0:
            payload = self._recv_exact(header["payload_length"])
        return header, payload

    def send_handshake(self, callsign, aircraft_type, aircraft_model,
                       origin, dest, seq_num=1):
        """
        Build and send a PKT_HANDSHAKE with a 52-byte HandshakePayload.

        HandshakePayload layout (packed, no padding):
          callsign[12] + aircraft_type[16] + aircraft_model[16]
          + origin[4] + dest[4]  = 52 bytes
        """
        def pad(s, n):
            b = s.encode("utf-8") if isinstance(s, str) else s
            return b[:n].ljust(n, b"\x00")

        payload = (
            pad(callsign, 12)
            + pad(aircraft_type, 16)
            + pad(aircraft_model, 16)
            + pad(origin, 4)
            + pad(dest, 4)
        )  # 52 bytes total

        self.send_packet(
            PKT_HANDSHAKE, seq_num,
            aircraft_id=callsign,
            atc_id=0,
            emergency_flag=0,
            payload=payload,
        )

    def do_full_handshake(self, callsign="AC8821", aircraft_type="B737",
                          aircraft_model="737-800", origin="CYYZ",
                          dest="CYVR"):
        """
        Send handshake and wait for ACK.
        Returns True if server responds with PKT_ACK, False otherwise.
        """
        self.send_handshake(callsign, aircraft_type, aircraft_model,
                            origin, dest)
        try:
            header, _ = self.receive_packet()
            return header["packet_type"] == PKT_ACK
        except Exception:
            return False
