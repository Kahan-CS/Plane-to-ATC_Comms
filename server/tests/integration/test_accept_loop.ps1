param(
    [string]$ServerExe = "./atc-server.exe",
    [int]$Port = 19090
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $ServerExe)) {
    throw "Server executable not found: $ServerExe"
}

$server = Start-Process -FilePath $ServerExe -ArgumentList "$Port" -PassThru -WindowStyle Hidden

try {
    Start-Sleep -Milliseconds 500

    $client1 = New-Object System.Net.Sockets.TcpClient
    $client1.Connect("127.0.0.1", $Port)
    $client1.Close()

    Start-Sleep -Milliseconds 200

    if ($server.HasExited) {
        throw "Server exited unexpectedly after first client disconnect"
    }

    $client2 = New-Object System.Net.Sockets.TcpClient
    $client2.Connect("127.0.0.1", $Port)
    $client2.Close()

    Start-Sleep -Milliseconds 200

    if ($server.HasExited) {
        throw "Server exited unexpectedly after second client disconnect"
    }

    Write-Host "[PASS] Integration accept-loop smoke test"
}
finally {
    if ($server -and -not $server.HasExited) {
        Stop-Process -Id $server.Id -Force
    }
}
