$ErrorActionPreference = "Stop"

$sourceDir = Join-Path $PSScriptRoot "..\..\..\styles"
$source = Join-Path $sourceDir "base.css"
$destinationDir = Join-Path $PSScriptRoot "..\web\styles"

if (-not (Test-Path $source)) {
  throw "Global styles file not found: $source"
}

New-Item -ItemType Directory -Path $destinationDir -Force | Out-Null
$files = Get-ChildItem -Path $sourceDir -Filter "base*.css" -File
foreach ($file in $files) {
  Copy-Item -Path $file.FullName -Destination (Join-Path $destinationDir $file.Name) -Force
}
Write-Host "Synchronized $($files.Count) base style layers: $sourceDir -> $destinationDir"
