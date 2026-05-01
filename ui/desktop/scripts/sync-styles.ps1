$ErrorActionPreference = "Stop"

$source = Join-Path $PSScriptRoot "..\..\..\styles\base.css"
$destinationDir = Join-Path $PSScriptRoot "..\web\styles"
$destination = Join-Path $destinationDir "base.css"

if (-not (Test-Path $source)) {
  throw "Global styles file not found: $source"
}

New-Item -ItemType Directory -Path $destinationDir -Force | Out-Null
Copy-Item -Path $source -Destination $destination -Force
Write-Host "Synchronized styles: $source -> $destination"
