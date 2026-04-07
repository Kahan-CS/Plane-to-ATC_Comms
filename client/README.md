# ATC Client Build and Run Guide

This document covers build, test, and run steps for the Rust aircraft client (`atc-client`) on Windows.

## Prerequisites

- Windows 10/11
- Rust toolchain installed (`rustup`, `cargo`)
- ATC server available and listening on a TCP port

Install or update Rust:

```powershell
rustup update
```

## Build

From this folder (`client`):

```powershell
cargo build
```

Release build:

```powershell
cargo build --release
```

## Run Tests

```powershell
cargo test
```

## Run the Client

Usage:

```powershell
cargo run -- <SERVER_IP> <PORT> <CALLSIGN> <TYPE> <MODEL> <ORIGIN> <DEST>
```

Example:

```powershell
cargo run -- 127.0.0.1 9000 AC8821 B737 737-800 CYYZ CYVR
```

You can also run the built binary directly:

```powershell
.\target\debug\atc-client.exe 127.0.0.1 9000 AC8821 B737 737-800 CYYZ CYVR
```

## Typical Workflow

1. Start the ATC server.
2. Start the client with route and aircraft arguments.
3. Confirm handshake success in console output.
4. Use the cockpit menu to send phase data, MAYDAY, weather request, or handoff simulation.

## Logs

The client writes timestamped logs in the current working directory.

- Session log: `atc_client_YYYYMMDD_HHMMSS.log`
- Heartbeat log: `atc_heartbeat_YYYYMMDD_HHMMSS.log`

## Common Issues

- Connection refused:
	- Ensure server is running first.
	- Verify IP and port are correct.
- Build errors:
	- Run `rustup update`.
	- Run `cargo clean` then `cargo build`.
- No incoming responses:
	- Confirm firewall allows local TCP traffic for client/server executables.
