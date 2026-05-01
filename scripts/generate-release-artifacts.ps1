param(
    [string]$Version = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Invoke-Native([string]$FilePath, [string[]]$Arguments = @(), [switch]$Quiet) {
    $prevErrorAction = $ErrorActionPreference
    $hasNativePref = $null -ne (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)
    if ($hasNativePref) {
        $prevNativePref = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
    }
    $ErrorActionPreference = "Continue"
    try {
        if ($Quiet) {
            $output = & $FilePath @Arguments 2>&1
            $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
            if ($exitCode -ne 0) {
                $argsText = ($Arguments -join " ")
                $tail = ($output | Select-Object -Last 40) -join [Environment]::NewLine
                throw "command failed ($exitCode): $FilePath $argsText`n$tail"
            }
            return
        }
        & $FilePath @Arguments
        $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
        if ($exitCode -ne 0) {
            $argsText = ($Arguments -join " ")
            throw "command failed ($exitCode): $FilePath $argsText"
        }
    } finally {
        $ErrorActionPreference = $prevErrorAction
        if ($hasNativePref) {
            $PSNativeCommandUseErrorActionPreference = $prevNativePref
        }
    }
}

function Read-JsonFile([string]$Path) {
    if (-not (Test-Path $Path)) {
        throw "missing JSON file: $Path"
    }
    return Get-Content $Path -Raw | ConvertFrom-Json
}

function Try-GetGitCommit([string]$Root) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = "git"
    $psi.Arguments = "-C `"$Root`" rev-parse HEAD"
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $psi
    [void]$process.Start()
    $stdout = $process.StandardOutput.ReadToEnd()
    $null = $process.StandardError.ReadToEnd()
    $process.WaitForExit()

    if ($process.ExitCode -ne 0) {
        return "unknown"
    }

    $commit = ($stdout | Select-Object -First 1).Trim()
    if ([string]::IsNullOrWhiteSpace($commit)) {
        return "unknown"
    }

    return $commit
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$tauriConfig = Read-JsonFile (Join-Path $repoRoot "ui\desktop\src-tauri\tauri.conf.json")
$rootPackage = Read-JsonFile (Join-Path $repoRoot "package.json")
$desktopPackage = Read-JsonFile (Join-Path $repoRoot "ui\desktop\package.json")

$resolvedVersion = $Version
if ([string]::IsNullOrWhiteSpace($resolvedVersion)) {
    $resolvedVersion = [string]$tauriConfig.version
}
if ([string]::IsNullOrWhiteSpace($resolvedVersion)) {
    throw "unable to resolve release version"
}
if ([string]$rootPackage.version -ne $resolvedVersion) {
    throw "root package.json version mismatch: $($rootPackage.version) != $resolvedVersion"
}
if ([string]$desktopPackage.version -ne $resolvedVersion) {
    throw "ui/desktop package.json version mismatch: $($desktopPackage.version) != $resolvedVersion"
}

$desktopBinaryName = "browser-desktop-ui.exe"
$launcherBinaryName = "cerbena-launcher.exe"
$releaseRoot = Join-Path $repoRoot ("build\release\" + $resolvedVersion)
$stagingRoot = Join-Path $releaseRoot "staging"
$bundleRoot = Join-Path $stagingRoot "cerbena-windows-x64"
$archivePath = Join-Path $releaseRoot "cerbena-windows-x64.zip"
$manifestPath = Join-Path $releaseRoot "release-manifest.json"
$checksumsPath = Join-Path $releaseRoot "checksums.txt"
$desktopBinary = Join-Path $repoRoot ("ui\desktop\src-tauri\target\release\" + $desktopBinaryName)
$launcherBinary = Join-Path $repoRoot ("target\release\" + $launcherBinaryName)

if (Test-Path $releaseRoot) {
    Remove-Item -LiteralPath $releaseRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $bundleRoot -Force | Out-Null

Invoke-Native "cargo" @("build", "-p", "cerbena-launcher", "--release")
Push-Location (Join-Path $repoRoot "ui\desktop")
try {
    Invoke-Native "npm.cmd" @("run", "style:sync")
    Invoke-Native "npm.cmd" @("run", "i18n:check")
    Push-Location "src-tauri"
    try {
        Invoke-Native "cargo" @("build", "--release")
    } finally {
        Pop-Location
    }
} finally {
    Pop-Location
}

foreach ($requiredFile in @($desktopBinary, $launcherBinary)) {
    if (-not (Test-Path $requiredFile)) {
        throw "expected release binary not found: $requiredFile"
    }
}

Copy-Item -LiteralPath $desktopBinary -Destination (Join-Path $bundleRoot "cerbena.exe") -Force
Copy-Item -LiteralPath $launcherBinary -Destination (Join-Path $bundleRoot $launcherBinaryName) -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "README.md") -Destination (Join-Path $bundleRoot "README.md") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "README.en.md") -Destination (Join-Path $bundleRoot "README.en.md") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "CHANGELOG.md") -Destination (Join-Path $bundleRoot "CHANGELOG.md") -Force

Compress-Archive -Path (Join-Path $bundleRoot "*") -DestinationPath $archivePath -CompressionLevel Optimal -Force

$commit = Try-GetGitCommit $repoRoot

$artifacts = @(
    @{
        name = "cerbena.exe"
        source = $desktopBinary
        target = "cerbena-windows-x64/cerbena.exe"
    },
    @{
        name = $launcherBinaryName
        source = $launcherBinary
        target = "cerbena-windows-x64/$launcherBinaryName"
    },
    @{
        name = "cerbena-windows-x64.zip"
        source = $archivePath
        target = "cerbena-windows-x64.zip"
    }
)

$manifestArtifacts = @()
$checksumLines = New-Object System.Collections.Generic.List[string]
foreach ($artifact in $artifacts) {
    $hash = (Get-FileHash -LiteralPath $artifact.source -Algorithm SHA256).Hash.ToLowerInvariant()
    $size = (Get-Item -LiteralPath $artifact.source).Length
    $manifestArtifacts += @{
        name = $artifact.name
        path = $artifact.target
        sha256 = $hash
        size_bytes = $size
    }
    $checksumLines.Add("$hash  $($artifact.target)")
}

$manifest = @{
    product = "Cerbena Browser"
    version = $resolvedVersion
    git_commit = $commit
    generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    repository_url = "https://github.com/BerkutSolutions/cerbena-browser"
    artifacts = $manifestArtifacts
}

$manifest | ConvertTo-Json -Depth 6 | Set-Content -Path $manifestPath -Encoding utf8
$checksumLines | Set-Content -Path $checksumsPath -Encoding utf8

Write-Host "Release artifacts generated at $releaseRoot" -ForegroundColor Green
