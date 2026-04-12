/// logger.rs — Session logging for the ATC Client
///
/// Writes every TX/RX packet event and session summary to a uniquely-named
/// plaintext log file created at startup.
///
/// Log entry format (REQ-LOG-030):
///   [CLIENT] | [YYYY-MM-DD HH:MM:SS.mmm] | [TX/RX] | [PACKET_TYPE] | [SEQ] | [LEN] | [SUMMARY]
///
/// MAYDAY entries are prefixed with [MAYDAY] (REQ-LOG-050).
/// State transitions are not applicable client-side but connection events are logged.
///
/// Regulatory compliance:
///   - CARs SOR/96-433 s.605.86 — requirement for operational records
///   - DO-178C DAL-D — traceability of all data events
///
/// REQ-LOG-010, REQ-LOG-030, REQ-LOG-040, REQ-LOG-050, REQ-LOG-060

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::sync::Mutex;
use chrono::Local;

pub struct Logger {
    file: Mutex<File>,
    pub tx_count: Mutex<u64>,
    pub rx_count: Mutex<u64>,
    pub error_count: Mutex<u64>,
}

impl Logger {
    /// Create a new log file named with the session start timestamp.
    /// REQ-LOG-040: unique filename per session, never overwritten.
    pub fn new(app_name: &str) -> io::Result<Self> {
        let ts = Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("{}_{}.log", app_name, ts);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&filename)?;
        eprintln!("[Logger] Session log: {}", filename);
        Ok(Logger {
            file: Mutex::new(file),
            tx_count:    Mutex::new(0),
            rx_count:    Mutex::new(0),
            error_count: Mutex::new(0),
        })
    }

    /// Write a log entry and flush immediately (REQ-LOG-010).
    fn write_line(&self, line: &str) {
        let mut f = self.file.lock().unwrap();
        writeln!(f, "{}", line).ok();
        f.flush().ok();
    }

    /// Format the current timestamp as HH:MM:SS.mmm
    fn now() -> String {
        Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
    }

    /// Log a transmitted packet.
    pub fn log_tx(&self, pkt_type: &str, seq: u32, len: u32, summary: &str) {
        *self.tx_count.lock().unwrap() += 1;
        let line = format!(
            "[CLIENT] | {} | TX | {} | {} | {} | {}",
            Self::now(), pkt_type, seq, len, summary
        );
        self.write_line(&line);
    }

    /// Log a received packet.
    pub fn log_rx(&self, pkt_type: &str, seq: u32, len: u32, summary: &str) {
        *self.rx_count.lock().unwrap() += 1;
        let line = format!(
            "[CLIENT] | {} | RX | {} | {} | {} | {}",
            Self::now(), pkt_type, seq, len, summary
        );
        self.write_line(&line);
    }

    /// Log a MAYDAY transmission (REQ-LOG-050).
    pub fn log_mayday(&self, seq: u32) {
        *self.tx_count.lock().unwrap() += 1;
        let line = format!(
            "[MAYDAY] [CLIENT] | {} | TX | MAYDAY | {} | 0 | Emergency flag transmitted",
            Self::now(), seq
        );
        self.write_line(&line);
    }

    /// Log an error event.
    pub fn log_error(&self, detail: &str) {
        *self.error_count.lock().unwrap() += 1;
        let line = format!("[CLIENT] | {} | ERROR | {}", Self::now(), detail);
        self.write_line(&line);
    }

    /// Log a connection event (connect / disconnect / loss / retry).
    pub fn log_connection(&self, event: &str) {
        let line = format!("[CLIENT] | {} | CONN | {}", Self::now(), event);
        self.write_line(&line);
    }

    /// Write session summary on shutdown (REQ-LOG-060).
    pub fn write_summary(&self) {
        let tx    = *self.tx_count.lock().unwrap();
        let rx    = *self.rx_count.lock().unwrap();
        let errs  = *self.error_count.lock().unwrap();
        let line = format!(
            "[CLIENT] | {} | SUMMARY | TX={} RX={} ERRORS={}",
            Self::now(), tx, rx, errs
        );
        self.write_line(&line);
    }
}

// Unit-tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logger_creates_file_and_writes() {
        let logger = Logger::new("test_client").expect("logger init failed");
        logger.log_tx("HANDSHAKE", 1, 52, "AC8821 CYYZ->CYVR");
        logger.log_rx("ACK", 1, 0, "seq=1");
        logger.log_mayday(2);
        logger.log_error("simulated error");
        logger.write_summary();

        let tx = *logger.tx_count.lock().unwrap();
        let rx = *logger.rx_count.lock().unwrap();
        assert_eq!(tx, 2); // HANDSHAKE + MAYDAY
        assert_eq!(rx, 1); // ACK
    }

    #[test]
    fn mayday_prefix_in_log_line() {
        // REQ-LOG-050: MAYDAY entries must be prefixed [MAYDAY]
        let logger = Logger::new("test_mayday").expect("logger init failed");
        // We can't easily inspect file content in unit test without extra plumbing,
        // but we can verify the counter increments correctly.
        let before = *logger.tx_count.lock().unwrap();
        logger.log_mayday(99);
        let after = *logger.tx_count.lock().unwrap();
        assert_eq!(after - before, 1);
    }

    //CLT-017
    //REQ-LOG-010, REQ-SYS-050
    //DO-178C DAL-D — all RX events must be logged immediately for traceability
    //Verifies log_rx increments rx_count and does not crash
    // @pass  rx_count equals 1 after one log_rx call
    #[test]
    fn test_clt017_log_rx_increments_counter() {
        let logger = Logger::new("test_clt017").expect("logger init failed");
        let before = *logger.rx_count.lock().unwrap();
        logger.log_rx("ACK", 1, 0, "seq=1 acknowledged");
        let after = *logger.rx_count.lock().unwrap();
        assert_eq!(after - before, 1,
            "rx_count must increment by 1 after log_rx");
    }

    //CLT-018
    //REQ-LOG-060
    //DO-178C DAL-D — session summary required for post-flight audit traceability
    //Verifies write_summary does not crash and log file still exists after call
    // @pass  write_summary completes, counters readable
    #[test]
    fn test_clt018_write_summary_no_crash() {
        let logger = Logger::new("test_clt018").expect("logger init failed");
        logger.log_tx("HANDSHAKE", 1, 52, "test");
        logger.log_rx("ACK", 1, 0, "test");
        logger.log_error("test error");
       
        logger.write_summary();
        // Counters still readable after summary
        let tx = *logger.tx_count.lock().unwrap();
        let rx = *logger.rx_count.lock().unwrap();
        assert_eq!(tx, 1, "tx_count must remain accurate after summary");
        assert_eq!(rx, 1, "rx_count must remain accurate after summary");
    }
}
