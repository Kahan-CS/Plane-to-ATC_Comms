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
