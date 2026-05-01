$ErrorActionPreference = "Stop"

$port = 1430
$connections = Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $port -ErrorAction SilentlyContinue

if (-not $connections) {
  Write-Host "No process is listening on 127.0.0.1:$port"
  exit 0
}

$pids = $connections | Select-Object -ExpandProperty OwningProcess -Unique

foreach ($pid in $pids) {
  if (-not $pid) {
    continue
  }

  $process = Get-Process -Id $pid -ErrorAction SilentlyContinue
  if (-not $process) {
    continue
  }

  Write-Host "Stopping PID $pid ($($process.ProcessName)) on 127.0.0.1:$port"
  Stop-Process -Id $pid -Force
}

Write-Host "Dev web server port $port is free."
