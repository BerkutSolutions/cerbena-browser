param(
    [string]$Version = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "release-signing.ps1")

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

function Write-Utf8NoBomFile([string]$Path, [string]$Content) {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function New-ReleaseManifestEntry(
    [string]$Name,
    [string]$Target,
    [string]$Source,
    [string]$Platform,
    [string]$Kind,
    [string]$InstallerKind,
    [string]$UpdaterStrategy,
    [bool]$Primary
) {
    $hash = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash.ToLowerInvariant()
    $size = (Get-Item -LiteralPath $Source).Length
    return @{
        name = $Name
        path = $Target
        sha256 = $hash
        size_bytes = $size
        platform = $Platform
        kind = $Kind
        installer_kind = $InstallerKind
        updater_strategy = $UpdaterStrategy
        primary = $Primary
    }
}

function Resolve-OptionalLinuxReleaseArtifacts([string]$RepoRoot, [string]$Version) {
    $linuxDropRoot = Join-Path $RepoRoot ("build\linux\" + $Version)
    if (-not (Test-Path -LiteralPath $linuxDropRoot)) {
        return @()
    }

    $artifacts = New-Object System.Collections.Generic.List[hashtable]
    $debFiles = Get-ChildItem -LiteralPath $linuxDropRoot -File -Filter "*.deb" -ErrorAction SilentlyContinue |
        Sort-Object Name
    foreach ($debFile in $debFiles) {
        if ($debFile.Name -notlike ("cerbena-browser_" + $Version + "_*.deb")) {
            throw "unexpected Debian artifact name '$($debFile.Name)'; expected cerbena-browser_${Version}_<arch>.deb under $linuxDropRoot"
        }
        if ($debFile.Name -notlike "*_amd64.deb") {
            throw "unsupported Debian artifact architecture '$($debFile.Name)'; first Linux slice only supports amd64/x86_64 packages"
        }

        [void]$artifacts.Add(@{
            name = $debFile.Name
            source = $debFile.FullName
            target = $debFile.Name
            platform = "linux-x64"
            kind = "installer"
            installer_kind = "deb"
            updater_strategy = "manual_download"
            primary = $false
        })
    }

    return @($artifacts)
}

function Resolve-BuiltBinaryPath([string]$RepoRoot, [string]$PreferredPath, [string]$BinaryName) {
    if (Test-Path -LiteralPath $PreferredPath) {
        return $PreferredPath
    }

    $candidateRoots = @(
        (Join-Path $RepoRoot "target\release"),
        (Join-Path $RepoRoot "cmd\launcher\target\release"),
        (Join-Path $RepoRoot "ui\desktop\src-tauri\target\release")
    ) | Where-Object { Test-Path -LiteralPath $_ }

    foreach ($root in $candidateRoots) {
        $candidate = Join-Path $root $BinaryName
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }

    $fallback = Get-ChildItem -LiteralPath $RepoRoot -Recurse -File -Filter $BinaryName -ErrorAction SilentlyContinue |
        Sort-Object FullName |
        Select-Object -First 1 -ExpandProperty FullName
    if (-not [string]::IsNullOrWhiteSpace($fallback)) {
        return $fallback
    }

    return $PreferredPath
}

function Resolve-LocalCargoTargetDir([string]$RepoRoot) {
    return (Join-Path $RepoRoot "target")
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
$checksumsSignaturePath = Join-Path $releaseRoot "checksums.sig"
$cargoTargetDir = Resolve-LocalCargoTargetDir $repoRoot
$desktopBinary = Join-Path $cargoTargetDir ("release\" + $desktopBinaryName)
$launcherBinary = Join-Path $cargoTargetDir ("release\" + $launcherBinaryName)

if (Test-Path $releaseRoot) {
    Remove-Item -LiteralPath $releaseRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $bundleRoot -Force | Out-Null

Invoke-Native "cargo" @("build", "-p", "cerbena-launcher", "--release", "--target-dir", $cargoTargetDir)
Push-Location (Join-Path $repoRoot "ui\desktop")
try {
    Invoke-Native "npm.cmd" @("run", "style:sync")
    Invoke-Native "npm.cmd" @("run", "i18n:check")
    Push-Location "src-tauri"
    try {
        Invoke-Native "cargo" @("build", "--release", "--target-dir", $cargoTargetDir)
    } finally {
        Pop-Location
    }
} finally {
    Pop-Location
}

$desktopBinary = Resolve-BuiltBinaryPath $repoRoot $desktopBinary $desktopBinaryName
$launcherBinary = Resolve-BuiltBinaryPath $repoRoot $launcherBinary $launcherBinaryName

foreach ($requiredFile in @($desktopBinary, $launcherBinary)) {
    if (-not (Test-Path $requiredFile)) {
        throw "expected release binary not found: $requiredFile"
    }
}

Copy-Item -LiteralPath $desktopBinary -Destination (Join-Path $bundleRoot "cerbena.exe") -Force
Copy-Item -LiteralPath $desktopBinary -Destination (Join-Path $bundleRoot "cerbena-updater.exe") -Force
Copy-Item -LiteralPath $launcherBinary -Destination (Join-Path $bundleRoot $launcherBinaryName) -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "README.md") -Destination (Join-Path $bundleRoot "README.md") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "README.en.md") -Destination (Join-Path $bundleRoot "README.en.md") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "CHANGELOG.md") -Destination (Join-Path $bundleRoot "CHANGELOG.md") -Force

Sign-WindowsArtifacts @($bundleRoot)

Compress-Archive -Path (Join-Path $bundleRoot "*") -DestinationPath $archivePath -CompressionLevel Optimal -Force

$commit = Try-GetGitCommit $repoRoot

$artifacts = @(
    @{
        name = "cerbena.exe"
        source = $desktopBinary
        target = "cerbena-windows-x64/cerbena.exe"
        platform = "windows-x64"
        kind = "bundle_binary"
        installer_kind = "none"
        updater_strategy = "embedded_runtime"
        primary = $false
    },
    @{
        name = "cerbena-updater.exe"
        source = $desktopBinary
        target = "cerbena-windows-x64/cerbena-updater.exe"
        platform = "windows-x64"
        kind = "bundle_binary"
        installer_kind = "none"
        updater_strategy = "standalone_updater"
        primary = $false
    },
    @{
        name = $launcherBinaryName
        source = $launcherBinary
        target = "cerbena-windows-x64/$launcherBinaryName"
        platform = "windows-x64"
        kind = "bundle_binary"
        installer_kind = "none"
        updater_strategy = "launcher_runtime"
        primary = $false
    },
    @{
        name = "cerbena-windows-x64.zip"
        source = $archivePath
        target = "cerbena-windows-x64.zip"
        platform = "windows-x64"
        kind = "bundle"
        installer_kind = "portable_zip"
        updater_strategy = "portable_zip"
        primary = $false
    }
)
$artifacts += @(Resolve-OptionalLinuxReleaseArtifacts -RepoRoot $repoRoot -Version $resolvedVersion)

$manifestArtifacts = @()
$checksumLines = New-Object System.Collections.Generic.List[string]
foreach ($artifact in $artifacts) {
    $entry = New-ReleaseManifestEntry `
        -Name $artifact.name `
        -Target $artifact.target `
        -Source $artifact.source `
        -Platform $artifact.platform `
        -Kind $artifact.kind `
        -InstallerKind $artifact.installer_kind `
        -UpdaterStrategy $artifact.updater_strategy `
        -Primary ([bool]$artifact.primary)
    $manifestArtifacts += $entry
    $checksumLines.Add("$($entry.sha256)  $($artifact.target)")
}

$manifest = @{
    product = "Cerbena Browser"
    version = $resolvedVersion
    git_commit = $commit
    generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    repository_url = "https://github.com/BerkutSolutions/cerbena-browser"
    artifacts = $manifestArtifacts
}

$manifestJson = $manifest | ConvertTo-Json -Depth 6
$checksumsText = [string]::Join("`n", $checksumLines)
$checksumsBytes = [System.Text.Encoding]::UTF8.GetBytes($checksumsText)
$checksumsSignature = New-ReleaseChecksumSignature $checksumsBytes

Write-Utf8NoBomFile $manifestPath $manifestJson
Write-Utf8NoBomFile $checksumsPath $checksumsText
Write-Utf8NoBomFile $checksumsSignaturePath $checksumsSignature

Write-Host "Release artifacts generated at $releaseRoot" -ForegroundColor Green
