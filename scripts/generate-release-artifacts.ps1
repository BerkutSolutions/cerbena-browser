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

function Write-Utf8NoBomFile([string]$Path, [string]$Content) {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Get-ReleaseSigningPrivateKeyXml() {
    $envValue = [Environment]::GetEnvironmentVariable("CERBENA_RELEASE_SIGNING_PRIVATE_KEY_XML")
    if (-not [string]::IsNullOrWhiteSpace($envValue)) {
        return $envValue.Trim()
    }
    return @'
<RSAKeyValue><Modulus>sQ/dGNzpHEHiSUvpp8+h4axIghjUrkY9hHX3GNPwS9kGK6FCoc6+DuKSK/u5JwEKk/sjTks2m8ANgCm1ajaEPFE/BQjP1VsqQE3/MGbpRwWXIYUP6qKX2EhMQa5Fg0fywHV5uk7v3x6Q/Yfc4cWVLKNClqpq2hk8CX0NfUjqN1s5CNnNH1zgZPZ45ExXZQBlM5UUhdY/N4LKTFiYjpDMvoW4KSM4j9maUBmoNGVTnnRgfyWm6wM7LCoqSPpYhSb4yE+/HtaBGpePVy21B5Xi1nzPSYfShEdVkmeCJTcTj8gr1o8OcqKEs5V3yQa6MmUhNgYM/uC/lGeqiR+lwiLG4Q==</Modulus><Exponent>AQAB</Exponent><P>0mEi1NpWZ8TZOYRS3hTe5oVwv3ZjczL6RaTTEWBhxge6k8EMdk++xeLlmFVxG2deSn04dYAfgUiZgP1HUM0jV00ddfGE/NibG547a9kCme7jxDDeYm4PLzRMRVkrBNhBX7rZQ37aPu0IjXuCfMmHJQlhVxveZvmwOOxwblRfVv8=</P><Q>13Up65xh9Ny3OY70etSsF65PGcTEkXq1gDhNAeVJTLiKlTdEP+sO/S9cIkNv87dYz5QNe1vYsKLJqnNFKKmc+AuTjdK9epGj0VH5VjJHUCWARWFxHnlviwFMlhZJQpmc8qtlbTXFuSlE5FDcCPv7geSJOvxO1iA2rySd5u9Wwh8=</Q><DP>rjbwQDG6gd4aQK4abXv9BgqUzoh8XIZniEqw2t/kt7fowriH2GW7RmXZ2WdP7fCQvcCqg2shK89yBsY3S2tFC+N5NRVXGodJEvranDmuFMkl5m7Nidc4Tc/SJU9s92sZ3+t8RY+Drb5eacNQ0IOWnY4CBL+4UbANRWZOyJ6oAQM=</DP><DQ>1qb3mLA4N0cdk86Ea0suGHmkfLu4SmfCI3fz4IuaN0Ezb+2bpUJ9sGhalhgxlNF5PXT26Ytbmr7Tw2kL4bL5m3WND6KA+3fViVjt255DxelWnciydfXt1sL4lh6l5iA8aNexONh1oD8pT33veVPyAjq5LXbo5BM758nHNqgD+2k=</DQ><InverseQ>UpocYLRWaSn6H/ztKG3ytyJuFhnkxKEH/ED0flKpBdFdB8rMcDLo08cICqtFVfO/l47sXfKlQ8sckRXGS1KP/8Ygh1mNCciEZFVW8eXXRtu7fcK5HX8lTetBIHYVDXi3ObKPNhjNGr6uJOiQcUVCWRHRfLpq12yOSIwBQMlCc3Y=</InverseQ><D>nHxZurybJYcw9/iok9BU0P+Twa8yYKfhfK1Jal79k/tFkc/e9OSkYsFp0IeT1t37vFeLl4mvxK1TAT9bf3iZHDnuCYQFMxp0WArXC68YYtWVAWH5dDSpINSc2Lut4d33tJLet4NGSppYKEooND2MnrvXgRMyhnkg733fKygDIFH91f6E/pMPhOUpJNdWFkTneBm7YkBjqiWeWLHKUmf+HrBuHQcfewyQ+a5Z6EVYyNC+ayL5Y4gscFyvJRbNuYuQGf9etCn7+igLuBgzoDmArAJ603hp4LgHX1mgSynTfmiL7NCKWQElx2e9KtR6KPVsf0gSMcza92vO85DR5DomxQ==</D></RSAKeyValue>
'@
}

function New-ReleaseChecksumSignature([byte[]]$ChecksumBytes) {
    $privateKeyXml = Get-ReleaseSigningPrivateKeyXml
    $rsa = New-Object System.Security.Cryptography.RSACryptoServiceProvider
    $rsa.PersistKeyInCsp = $false
    $rsa.FromXmlString($privateKeyXml)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $signature = $rsa.SignData($ChecksumBytes, $sha)
        return [Convert]::ToBase64String($signature)
    } finally {
        $rsa.Dispose()
        $sha.Dispose()
    }
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
Copy-Item -LiteralPath $desktopBinary -Destination (Join-Path $bundleRoot "cerbena-updater.exe") -Force
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
        name = "cerbena-updater.exe"
        source = $desktopBinary
        target = "cerbena-windows-x64/cerbena-updater.exe"
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

$manifestJson = $manifest | ConvertTo-Json -Depth 6
$checksumsText = [string]::Join([Environment]::NewLine, $checksumLines)
$checksumsBytes = [System.Text.Encoding]::UTF8.GetBytes($checksumsText)
$checksumsSignature = New-ReleaseChecksumSignature $checksumsBytes

Write-Utf8NoBomFile $manifestPath $manifestJson
Write-Utf8NoBomFile $checksumsPath $checksumsText
Write-Utf8NoBomFile $checksumsSignaturePath $checksumsSignature

Write-Host "Release artifacts generated at $releaseRoot" -ForegroundColor Green
