/**
 * @file  server_config.h
 * @brief Server-wide constants and configuration.
 * 
 * Requirements covered:
 *   REQ-SVR-010   Server identity used during listen/accept
 *   REQ-PKT-020   SERVER_ATC_ID is the value a live packet carries
 *                 (or zero, from the client's perspective)
 */

#ifndef SERVER_CONFIG_H
#define SERVER_CONFIG_H

#include <stdint.h>

#define SERVER_ATC_ID       1        /* Unique ATC station identifier  */
#define SERVER_AIRPORT_CODE "CYYZ"   /* ICAO code for this station     */
#define MAX_PAYLOAD_BYTES   (10 * 1024 * 1024)  /* 10 MB safety cap   */

#endif /* SERVER_CONFIG_H */