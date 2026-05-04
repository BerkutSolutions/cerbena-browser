param(
    [string]$Version = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Utf8NoBomFile([string]$Path, [string]$Content) {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Read-JsonFile([string]$Path) {
    if (-not (Test-Path $Path)) {
        throw "missing JSON file: $Path"
    }
    return Get-Content $Path -Raw | ConvertFrom-Json
}

function Normalize-Version([string]$Value) {
    $normalized = [string]$Value
    $normalized = $normalized.Trim()
    if ($normalized.StartsWith("v")) {
        $normalized = $normalized.Substring(1)
    }
    if ([string]::IsNullOrWhiteSpace($normalized)) {
        throw "version must not be empty"
    }
    if ($normalized -notmatch '^\d+\.\d+\.\d+$') {
        throw "version must use semantic format X.Y.Z"
    }
    return $normalized
}

function Apply-ReplaceAll([string]$Content, [string]$CurrentVersion, [string]$NextVersion, [string]$Path) {
    if ($CurrentVersion -eq $NextVersion) {
        return $Content
    }
    if (-not $Content.Contains($CurrentVersion)) {
        throw "version string '$CurrentVersion' was not found in $Path"
    }
    return $Content.Replace($CurrentVersion, $NextVersion)
}

function Apply-ChangelogHeading([string]$Content, [string]$CurrentVersion, [string]$NextVersion, [string]$Path) {
    if ($CurrentVersion -eq $NextVersion) {
        return $Content
    }
    $pattern = "(?m)^##\s+" + [regex]::Escape($CurrentVersion) + "(\s*)$"
    $replaced = [regex]::Replace($Content, $pattern, ("## " + $NextVersion + '$1'), 1)
    if ($replaced -eq $Content) {
        throw "release heading for $CurrentVersion was not found in $Path"
    }
    return $replaced
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$manifestPath = Join-Path $repoRoot "scripts\version-sync-targets.json"
$tauriConfigPath = Join-Path $repoRoot "ui\desktop\src-tauri\tauri.conf.json"
$targetsManifest = Read-JsonFile $manifestPath
$tauriConfig = Read-JsonFile $tauriConfigPath
$currentVersion = Normalize-Version ([string]$tauriConfig.version)

if ([string]::IsNullOrWhiteSpace($Version)) {
    Write-Host ""
    Write-Host ("Current version: " + $currentVersion) -ForegroundColor Cyan
    $Version = Read-Host "Enter a new version"
}

$nextVersion = Normalize-Version $Version
if ($nextVersion -eq $currentVersion) {
    Write-Host ("Version is already " + $nextVersion + ". Nothing to update.") -ForegroundColor Yellow
    exit 0
}

$updatedFiles = New-Object System.Collections.Generic.List[string]
foreach ($target in $targetsManifest.targets) {
    $relativePath = [string]$target.path
    $strategy = [string]$target.strategy
    $path = Join-Path $repoRoot $relativePath
    if (-not (Test-Path $path)) {
        throw "version target is missing: $relativePath"
    }
    $original = [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)
    $updated = switch ($strategy) {
        "replace_all" { Apply-ReplaceAll $original $currentVersion $nextVersion $relativePath }
        "changelog_heading" { Apply-ChangelogHeading $original $currentVersion $nextVersion $relativePath }
        default { throw "unknown version sync strategy '$strategy' for $relativePath" }
    }
    if ($updated -ne $original) {
        Write-Utf8NoBomFile $path $updated
        $updatedFiles.Add($relativePath) | Out-Null
    }
}

Write-Host ""
Write-Host ("Updated version: " + $currentVersion + " -> " + $nextVersion) -ForegroundColor Green
foreach ($item in $updatedFiles) {
    Write-Host (" - " + $item) -ForegroundColor DarkGray
}
