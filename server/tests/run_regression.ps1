$ErrorActionPreference = "Stop"

Write-Host "[1/4] Build server"
mingw32-make build

Write-Host "[2/4] Run unit tests"
mingw32-make unit

Write-Host "[3/4] Run integration tests"
mingw32-make integration

Write-Host "[4/4] Regression run complete"
