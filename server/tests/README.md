# Server Test Organization

Layout:

- `unittests/`: focused Unity unit tests for header/state/logger logic.
- `integration/`: smoke-level process/network behavior checks.
- `run_regression.ps1`: single entrypoint to build + unit + integration.

## Test Structure

- `unittests/test_state_machine.c`
: state transitions including reconnect paths (`HANDSHAKE -> TRANSIT/LANDING`) and MAYDAY coverage.
- `unittests/test_packet.c`
: protocol constants, packet/header offsets, and updated `HandshakePayload` size (`53`) with `initial_phase` offset check.
- `unittests/test_logger.c`
: logger lifecycle and counters.
- `integration/test_accept_loop.ps1`
: launches server, performs two sequential TCP connects, verifies server stays alive between sessions.

## Single Regression Run

From `server/`:

```powershell
powershell -ExecutionPolicy Bypass -File tests/run_regression.ps1
```

Or via Make targets:

```powershell
mingw32-make regression
```

## Unit-Only and Integration-Only

```powershell
mingw32-make unit
mingw32-make integration
```

## Coverage (gcovr)

Prerequisites:
- Python available on PATH
- `gcovr` installed in that Python environment (`pip install gcovr`)
- GCC/MinGW with gcov support

Run from `server/`:

```powershell
mingw32-make coverage
```

Artifacts:
- `coverage_report/summary.txt`
- `coverage_report/index.html`

Notes:
- Coverage excludes test sources (`--exclude tests`) to focus report on server headers/shared protocol code.
- `coverage` target performs a clean of old `.gcda/.gcno` data before execution.

## Cleanup

```powershell
mingw32-make clean
```
