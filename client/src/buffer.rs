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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{Packet, PacketHeader, PKT_TRANSIT};

    fn make_packet(seq: u32) -> Packet {
        Packet {
            header: PacketHeader {
                packet_type:    PKT_TRANSIT,
                seq_num:        seq,
                timestamp:      0,
                payload_length: 0,
                origin_atc_id:  0,
                aircraft_id:    [0u8; 32],
                emergency_flag: 0,
            },
            payload: vec![],
        }
    }

// test  CLT-013
    // req   REQ-SYS-090, REQ-CLT-060
    //   DO-178C DAL-D — buffer behaviour must bedeterministic for safety-critical handoff
    // Verifies HandoffBuffer stores packets on push and returns all on drain
    // @pass  drained.len() equals number of pushes
    #[test]
    fn test_clt013_buffer_push_and_drain() {
        let mut buf = HandoffBuffer::new(1);
        buf.push(make_packet(1));
        buf.push(make_packet(2));
        buf.push(make_packet(3));
        let drained = buf.drain();
        assert_eq!(drained.len(), 3,
            "drain must return all pushed packets");
    }

    // CLT-014
    //REQ-SYS-090, REQ-CLT-060
    // DO-178C DAL-D — empty state must be deterministic after drain
    //Verifies HandoffBuffer is empty after drain
    // @pass  is_empty() returns true after drain
    #[test]
    fn test_clt014_buffer_empty_after_drain() {
        let mut buf = HandoffBuffer::new(1);
        buf.push(make_packet(1));
        buf.drain();
        assert!(buf.is_empty(),
            "buffer must be empty after drain");
    }

    //CLT-015
    //REQ-SYS-090, REQ-CLT-060
    //DO-178C DAL-D — length must be traceable
    //Verifies len() increments with each push
    // @pass  len() equals push count at each step
    #[test]
    fn test_clt015_buffer_len_after_push() {
        let mut buf = HandoffBuffer::new(1);
        assert_eq!(buf.len(), 0);
        buf.push(make_packet(1)); assert_eq!(buf.len(), 1);
        buf.push(make_packet(2)); assert_eq!(buf.len(), 2);
        buf.push(make_packet(3)); assert_eq!(buf.len(), 3);
        buf.push(make_packet(4)); assert_eq!(buf.len(), 4);
        buf.push(make_packet(5)); assert_eq!(buf.len(), 5);
    }
}
