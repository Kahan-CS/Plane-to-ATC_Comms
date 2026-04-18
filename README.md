# Plane-to-ATC_Comms
Client-server application that is developed while accounting for aviation and software safety guidelines. Final course project for Software Safety &amp; Reliability (4th year BCS @Conestoga College)

---
# Testing: ATC Communication Handoff System

**Branch:** testing-environment  
**Standards:** DO-178C DAL-D · CARs SOR/96-433 Part V

## Tools Required

| Tool         | Version | Purpose                      | Install             |
| ------------ | ------- | ---------------------------- | ------------------- |
| Rust / Cargo | 1.94.1  | Client unit tests            | rustup.rs           |
| GCC (MinGW)  | 6.3.0   | Server unit tests            | mingw-w64.org       |
| mingw32-make | 3.82.90 | Server build and test runner | included with MinGW |
| Python       | 3.12.0  | Integration tests            | python.org          |
| pytest       | 9.0.3   | Integration test runner      | pip install pytest  |
| Git          | any     | Clone repo                   | git-scm.com         |

## Setup

**1. Clone and switch to the testing branch**
git clone https://github.com/Kahan-CS/Plane-to-ATC_Comms
cd Plane-to-ATC_Comms
git checkout testing-environment

**2. Install pytest**
python -m pip install pytest

**3. Windows note for make**  
MinGW installs `mingw32-make.exe` not `make.exe`.  
Either use `mingw32-make` directly in all commands below,  
or copy `C:\MinGW\bin\mingw32-make.exe` and rename the  
copy to `make.exe` in the same folder.

## Running Server Unit Tests

No server needs to be running. Tests compile and
run standalone using the Unity C framework.
cd server
mingw32-make test

## Running Client Unit Tests

No server needs to be running.
cd client
cargo test --bin atc-client

## Running Integration Tests

Integration tests require the server to be running.
Open two terminals.

**Terminal 1 — build and start the server:**
cd server
mingw32-make build
.\atc-server.exe 9000

**Terminal 2 — run integration tests:**
cd Plane-to-ATC_Comms
python -m pytest integration_tests/ -v

---

## Code Coverage — Server (C)

**Tool:** gcovr 8.6  
**Install:**

```
python -m pip install gcovr
```

**Run:**

```
cd server
mingw32-make coverage
```

**View HTML report:**

```
start coverage_report/index.html
```

Or open `server/coverage_report/index.html`
in any browser.

**Results (unit tests — DO-178C DAL-D compliance):**

| Metric    | Result | Detail                    |
| --------- | ------ | ------------------------- |
| Lines     | 94.9%  | 74 of 78 lines executed   |
| Functions | 100.0% | 10 of 10 functions called |
| Branches  | 59–70% | See note below            |

**Note on branch coverage:**  
Uncovered branches are defensive null guards
(`if g_log_file == NULL`) and OS-level failure
paths (`fopen` failure). These are infeasible
branches under normal operating conditions and
do not correspond to any testable requirement.
Per DO-178C DAL-D guidance, infeasible branches
are documented and excluded from the coverage
target. Line and function coverage are the
primary compliance metrics.

**Coverage report is gitignored** — regenerate
locally by running `mingw32-make coverage`.  
Clean up with `mingw32-make coverage-clean`.

---

## Platform Notes

**Windows (tested configuration):**  
All tests were developed and verified on Windows
using MinGW GCC 6.3.0. This configuration works
fully for unit tests, integration tests, and
gcovr coverage reporting.

**Linux note:**  
If running on Linux replace `mingw32-make` with
`make` in all commands. GCC on Linux (version 10+)
produces more precise branch coverage data.
The server uses `windows.h` and Winsock2 in
`logger.h` and `network.h` — these headers are
Windows-only. To run on Linux the server would
require platform abstractions for `GetLocalTime()`
and `WSAStartup()`. The test files (`test_logger.c`,
`test_state_machine.c`, `test_packet.c`) do not use
Winsock and would compile on Linux without changes.

**CI/CD note:**  
A GitHub Actions workflow running on Ubuntu could
provide automated coverage on every push with
more accurate branch tracking. This is a future
enhancement: current coverage numbers are
sufficient for DO-178C DAL-D compliance evidence.

