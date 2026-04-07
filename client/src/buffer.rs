/// buffer.rs — Packet buffer for ATC handoff retransmission
///
/// During an ATC handoff, the client disconnects from one server and
/// reconnects to another. Any packets that could not be delivered to
/// the first server are held in this buffer. On reconnect, they are
/// retransmitted to the new server with origin_atc_id set to the
/// previous server's ATC ID, so the new ATC knows they originated
/// from the previous sector.
///
/// Regulatory compliance:
///   - CARs SOR/96-433 Part V — no data loss during ATC boundary crossing
///   - DO-178C DAL-D — deterministic retransmission, full audit trail
///
/// REQ-CLT-060, REQ-SYS-090, REQ-PKT-020, REQ-COM-040

use crate::packet::Packet;

/// Holds packets that failed to send during an ATC handoff.
pub struct HandoffBuffer {
    /// The ATC ID of the server the packets were originally intended for.
    /// Set as origin_atc_id on retransmission (REQ-PKT-020).
    pub origin_atc_id: u32,
    packets: Vec<Packet>,
}

impl HandoffBuffer {
    pub fn new(origin_atc_id: u32) -> Self {
        HandoffBuffer { origin_atc_id, packets: Vec::new() }
    }

    /// Store a packet that could not be delivered (REQ-CLT-060).
    pub fn push(&mut self, mut pkt: Packet) {
        pkt.header.origin_atc_id = self.origin_atc_id;
        self.packets.push(pkt);
    }

    pub fn is_empty(&self) -> bool { self.packets.is_empty() }
    pub fn len(&self) -> usize     { self.packets.len() }

    /// Drain the buffer, returning packets in FIFO order.
    pub fn drain(&mut self) -> Vec<Packet> {
        std::mem::take(&mut self.packets)
    }
}

/// Retransmit all buffered packets to the new connection.
/// Returns the number of packets successfully sent.
///
/// REQ-SYS-090: buffered packets sent first, before any new commands.
pub fn flush_buffer(
    buffer: &mut HandoffBuffer,
    conn: &mut crate::network::Connection,
    logger: &crate::logger::Logger,
) -> usize {
    if buffer.is_empty() { return 0; }

    println!("[Handoff] Retransmitting {} buffered packet(s) to new ATC...",
        buffer.len());

    let mut sent = 0;
    for pkt in buffer.drain() {
        let seq = pkt.header.seq_num;
        let ptype = pkt.header.packet_type;
        let plen = pkt.header.payload_length;
        let origin = pkt.header.origin_atc_id;
        match conn.send_packet(&pkt) {
            Ok(_) => {
                logger.log_tx(
                    &format!("BUFFERED(0x{:02X})", ptype),
                    seq,
                    plen,
                    &format!("retransmitted from ATC#{}", origin),
                );
                sent += 1;
            }
            Err(e) => {
                logger.log_error(&format!(
                    "Buffer retransmit failed for seq={}: {}", seq, e
                ));
            }
        }
    }
    println!("[Handoff] {} packet(s) retransmitted.", sent);
    sent
}
