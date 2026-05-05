param(
    [string]$LegacyVersion = "1.0.0",
    [string]$MinimumPublishedVersion = "1.0.11",
    [string]$ExpectedPublishedVersion = "",
    [ValidateSet("msi_only")]
    [string]$ContractMode = "msi_only",
    [int]$TimeoutMinutes = 25,
    [switch]$CompactOutput,
    [switch]$KeepArtifacts
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function ConvertTo-NativeArgument([string]$Value) {
    if ($null -eq $Value) {
        return '""'
    }
    if ($Value -notmatch '[\s"]') {
        return $Value
    }
    $escaped = $Value -replace '(\\*)"', '$1$1\"'
    $escaped = $escaped -replace '(\\+)$', '$1$1'
    return '"' + $escaped + '"'
}

function Invoke-Native([string]$FilePath, [string[]]$Arguments = @(), [string]$WorkingDirectory = "", [hashtable]$Environment = @{}, [switch]$Quiet) {
    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $FilePath
    if (-not [string]::IsNullOrWhiteSpace($WorkingDirectory)) {
        $startInfo.WorkingDirectory = $WorkingDirectory
    }
    $startInfo.Arguments = (($Arguments | ForEach-Object { ConvertTo-NativeArgument $_ }) -join " ")
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($entry in $Environment.GetEnumerator()) {
        $startInfo.EnvironmentVariables[$entry.Key] = [string]$entry.Value
    }
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "failed to start process: $FilePath"
    }
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $stdout.Wait()
    $stderr.Wait()
    if ($process.ExitCode -ne 0) {
        $tail = @($stdout.Result, $stderr.Result) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        throw "command failed ($($process.ExitCode)): $FilePath $($Arguments -join ' ')`n$($tail -join [Environment]::NewLine)"
    }
    if (-not $Quiet) {
        if (-not [string]::IsNullOrWhiteSpace($stdout.Result)) {
            Write-Host $stdout.Result.TrimEnd()
        }
        if (-not [string]::IsNullOrWhiteSpace($stderr.Result)) {
            Write-Host $stderr.Result.TrimEnd()
        }
    }
    return @{
        StdOut = $stdout.Result
        StdErr = $stderr.Result
    }
}

function Write-Utf8NoBomFile([string]$Path, [string]$Content) {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Append-Utf8NoBomLine([string]$Path, [string]$Line) {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    $directory = Split-Path -Parent $Path
    if (-not [string]::IsNullOrWhiteSpace($directory)) {
        [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    }
    [System.IO.File]::AppendAllText($Path, $Line + [Environment]::NewLine, $encoding)
}

function Write-Log([string]$Message) {
    $timestamp = [DateTime]::UtcNow.ToString("o")
    $line = "[$timestamp] [published-updater-e2e] $Message"
    Write-Host $line
    Append-Utf8NoBomLine $script:SessionLogPath $line
}

function Read-JsonFile([string]$Path) {
    if (-not (Test-Path $Path)) {
        throw "missing file: $Path"
    }
    return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Normalize-Version([string]$Value) {
    if ($null -eq $Value) {
        return ""
    }
    return $Value.Trim().TrimStart("v", "V")
}

function Get-VersionCore([string]$Value) {
    $normalized = Normalize-Version $Value
    if ([string]::IsNullOrWhiteSpace($normalized)) {
        return ""
    }
    return ($normalized -split "-", 2)[0]
}

function Compare-SemVer([string]$Left, [string]$Right) {
    $split = {
        param([string]$Version)
        $core = Get-VersionCore $Version
        return $core.Split(".") | ForEach-Object {
            $value = 0
            [void][int]::TryParse($_, [ref]$value)
            $value
        }
    }
    $leftParts = & $split $Left
    $rightParts = & $split $Right
    $count = [Math]::Max($leftParts.Count, $rightParts.Count)
    for ($index = 0; $index -lt $count; $index++) {
        $leftValue = if ($index -lt $leftParts.Count) { $leftParts[$index] } else { 0 }
        $rightValue = if ($index -lt $rightParts.Count) { $rightParts[$index] } else { 0 }
        if ($leftValue -lt $rightValue) { return -1 }
        if ($leftValue -gt $rightValue) { return 1 }
    }
    return 0
}

function Test-VersionEquivalent([string]$Actual, [string]$Expected) {
    $normalizedActual = Normalize-Version $Actual
    $normalizedExpected = Normalize-Version $Expected
    if ($normalizedActual -eq $normalizedExpected) {
        return $true
    }
    return (Get-VersionCore $normalizedActual) -eq (Get-VersionCore $normalizedExpected)
}

function Resolve-WorkspaceVersion([string]$RepoRoot) {
    $tauriConfig = Read-JsonFile (Join-Path $RepoRoot "ui\desktop\src-tauri\tauri.conf.json")
    $rootPackage = Read-JsonFile (Join-Path $RepoRoot "package.json")
    $desktopPackage = Read-JsonFile (Join-Path $RepoRoot "ui\desktop\package.json")
    $version = Normalize-Version ([string]$tauriConfig.version)
    if ([string]::IsNullOrWhiteSpace($version)) {
        $workspaceManifestPath = Join-Path $RepoRoot "Cargo.toml"
        $workspaceManifest = Get-Content -LiteralPath $workspaceManifestPath -Raw
        $workspaceMatch = [regex]::Match($workspaceManifest, 'version = "([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?)"')
        if ($workspaceMatch.Success) {
            $version = Normalize-Version $workspaceMatch.Groups[1].Value
        }
    }
    if ([string]::IsNullOrWhiteSpace($version)) {
        throw "failed to detect current workspace version"
    }
    if ((Normalize-Version ([string]$rootPackage.version)) -ne $version) {
        throw "root package.json version mismatch: $($rootPackage.version) != $version"
    }
    if ((Normalize-Version ([string]$desktopPackage.version)) -ne $version) {
        throw "ui/desktop package.json version mismatch: $($desktopPackage.version) != $version"
    }
    return $version
}

function Get-LatestPublishedRelease() {
    $headers = @{
        "User-Agent" = "Cerbena-Updater-E2E"
        "Accept" = "application/vnd.github+json"
    }
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/BerkutSolutions/cerbena-browser/releases/latest" -Headers $headers
    $version = Normalize-Version ([string]$release.tag_name)
    $assets = @($release.assets)
    $msiAsset = $assets | Where-Object { $_.name -like "cerbena-browser-*.msi" } | Select-Object -First 1
    $checksumsAsset = $assets | Where-Object { $_.name -eq "checksums.txt" } | Select-Object -First 1
    $signatureAsset = $assets | Where-Object { $_.name -eq "checksums.sig" } | Select-Object -First 1
    if ($null -eq $msiAsset -or $null -eq $checksumsAsset -or $null -eq $signatureAsset) {
        throw "latest GitHub release is missing the MSI-only trusted updater contract (msi + checksum assets)"
    }
    return @{
        Version = $version
        HtmlUrl = [string]$release.html_url
        MsiAssetName = if ($null -ne $msiAsset) { [string]$msiAsset.name } else { "" }
    }
}

function Copy-TrackedWorkspaceSnapshot([string]$SourceRoot, [string]$DestinationRoot) {
    $tracked = Invoke-Native "git" @("-C", $SourceRoot, "ls-files", "--cached", "--others", "--exclude-standard") -Quiet
    $files = ($tracked.StdOut -split "`r?`n") |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
        Sort-Object -Unique
    foreach ($relativePath in $files) {
        $sourcePath = Join-Path $SourceRoot $relativePath
        $targetPath = Join-Path $DestinationRoot $relativePath
        $targetDirectory = Split-Path -Parent $targetPath
        if (-not [string]::IsNullOrWhiteSpace($targetDirectory)) {
            [System.IO.Directory]::CreateDirectory($targetDirectory) | Out-Null
        }
        Copy-Item -LiteralPath $sourcePath -Destination $targetPath -Force
    }
}

function Replace-VersionAcrossSnapshot([string]$SnapshotRoot, [string]$CurrentVersion, [string]$TargetVersion) {
    $textExtensions = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($extension in @(".toml", ".json", ".js", ".jsx", ".rs", ".md", ".yml", ".yaml", ".mjs", ".ps1")) {
        [void]$textExtensions.Add($extension)
    }
    $files = Get-ChildItem -LiteralPath $SnapshotRoot -Recurse -File
    foreach ($file in $files) {
        $fullPath = $file.FullName
        $extension = [System.IO.Path]::GetExtension($fullPath)
        if (-not $textExtensions.Contains($extension)) {
            continue
        }
        $content = [System.IO.File]::ReadAllText($fullPath, [System.Text.Encoding]::UTF8)
        if (-not $content.Contains($CurrentVersion)) {
            continue
        }
        Write-Utf8NoBomFile $fullPath ($content.Replace($CurrentVersion, $TargetVersion))
    }
}

function Stop-ProcessesInRoot([string]$Root) {
    $normalizedRoot = [System.IO.Path]::GetFullPath($Root)
    Get-Process -ErrorAction SilentlyContinue | ForEach-Object {
        try {
            if ([string]::IsNullOrWhiteSpace($_.Path)) {
                return
            }
            $processPath = [System.IO.Path]::GetFullPath($_.Path)
            if ($processPath.StartsWith($normalizedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
                Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
            }
        } catch {
        }
    }
}

function Read-UpdateStoreSnapshot([string]$LocalAppDataRoot) {
    $storePaths = @(
        (Join-Path $LocalAppDataRoot "dev.browser.launcher\app_update_store.json"),
        (Join-Path $LocalAppDataRoot "Cerbena Browser\app_update_store.json")
    )
    foreach ($storePath in $storePaths) {
        if (-not (Test-Path $storePath)) {
            continue
        }
        try {
            return Get-Content -LiteralPath $storePath -Raw | ConvertFrom-Json
        } catch {
            return $null
        }
    }
    return $null
}

function Get-StoreFieldValue($Store, [string[]]$Names, [string]$Default = "") {
    if ($null -eq $Store) {
        return $Default
    }
    foreach ($name in $Names) {
        $property = $Store.PSObject.Properties[$name]
        if ($null -ne $property) {
            $value = [string]$property.Value
            if (-not [string]::IsNullOrWhiteSpace($value)) {
                return $value
            }
        }
    }
    return $Default
}

function Get-RuntimeLogCandidates([string]$LocalAppDataRoot) {
    return @(
        (Join-Path $LocalAppDataRoot "dev.browser.launcher\runtime_logs.log"),
        (Join-Path $LocalAppDataRoot "Cerbena Browser\runtime_logs.log")
    )
}

function Get-ExistingRuntimeLogPath([string]$LocalAppDataRoot) {
    foreach ($path in (Get-RuntimeLogCandidates $LocalAppDataRoot)) {
        if (Test-Path $path) {
            return $path
        }
    }
    return $null
}

function Get-FileTail([string]$Path, [int]$Tail = 20) {
    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path $Path)) {
        return @()
    }
    return @(Get-Content -LiteralPath $Path -Tail $Tail)
}

function Test-StoreStatusAllowsBackgroundRelaunch([string]$Status) {
    if ($null -eq $Status) {
        return $false
    }
    return @("handoff", "downloaded", "applying", "ready_to_restart", "completed", "applied_pending_relaunch") -contains $Status
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$currentVersion = Resolve-WorkspaceVersion $repoRoot
$publishedRelease = Get-LatestPublishedRelease
if (-not [string]::IsNullOrWhiteSpace($ExpectedPublishedVersion) -and -not (Test-VersionEquivalent $publishedRelease.Version $ExpectedPublishedVersion)) {
    throw "latest published release $($publishedRelease.Version) does not match expected version $ExpectedPublishedVersion"
}
if ((Compare-SemVer $publishedRelease.Version $MinimumPublishedVersion) -lt 0) {
    throw "latest published release $($publishedRelease.Version) is older than required minimum $MinimumPublishedVersion"
}
if ((Compare-SemVer $publishedRelease.Version $LegacyVersion) -le 0) {
    throw "legacy test version $LegacyVersion must be older than published release $($publishedRelease.Version)"
}

$sessionRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("cerbena-updater-e2e-" + [guid]::NewGuid().ToString("N"))
$snapshotRoot = Join-Path $sessionRoot "snapshot"
$localAppDataRoot = Join-Path $sessionRoot "localappdata"
$installRoot = Join-Path $sessionRoot "install"
$targetRoot = Join-Path $sessionRoot "target"
$versionProbePath = Join-Path $sessionRoot "updated-version.txt"
$reportPath = Join-Path $sessionRoot "report.json"
$script:SessionLogPath = Join-Path $sessionRoot "published-updater-e2e.log"
$copiedExePath = Join-Path $installRoot "cerbena.exe"
$copiedUpdaterPath = Join-Path $installRoot "cerbena-updater.exe"
$installModeMarkerPath = Join-Path $installRoot "cerbena-install-mode.txt"

try {
    [System.IO.Directory]::CreateDirectory($snapshotRoot) | Out-Null
    [System.IO.Directory]::CreateDirectory($localAppDataRoot) | Out-Null
    [System.IO.Directory]::CreateDirectory($installRoot) | Out-Null
    [System.IO.Directory]::CreateDirectory($targetRoot) | Out-Null
    Write-Log "session root: $sessionRoot"
    Write-Log "workspace version: $currentVersion"
    Write-Log "published version: $($publishedRelease.Version)"
    Write-Log "published release url: $($publishedRelease.HtmlUrl)"

    Write-Host "Creating tracked workspace snapshot..." -ForegroundColor Cyan
    Write-Log "creating tracked workspace snapshot"
    Copy-TrackedWorkspaceSnapshot $repoRoot $snapshotRoot
    Replace-VersionAcrossSnapshot $snapshotRoot $currentVersion $LegacyVersion

    $buildEnv = @{
        "CARGO_TARGET_DIR" = $targetRoot
    }
    Write-Host "Building temporary legacy desktop binary ($LegacyVersion)..." -ForegroundColor Cyan
    Write-Log "building temporary legacy desktop binary version=$LegacyVersion"
    Invoke-Native "cargo" @("build", "--release", "--manifest-path", (Join-Path $snapshotRoot "ui\desktop\src-tauri\Cargo.toml")) $snapshotRoot $buildEnv -Quiet:$CompactOutput | Out-Null

    $builtExe = Join-Path $targetRoot "release\browser-desktop-ui.exe"
    if (-not (Test-Path $builtExe)) {
        throw "expected legacy desktop binary was not produced: $builtExe"
    }
    Copy-Item -LiteralPath $builtExe -Destination $copiedExePath -Force
    Copy-Item -LiteralPath $builtExe -Destination $copiedUpdaterPath -Force
    Write-Utf8NoBomFile $installModeMarkerPath "msi`n"

    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $copiedExePath
    $startInfo.Arguments = "--updater"
    $startInfo.WorkingDirectory = $installRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.Environment["LOCALAPPDATA"] = $localAppDataRoot
    $startInfo.Environment["CERBENA_SELFTEST_REPORT_VERSION_FILE"] = $versionProbePath
    $startInfo.Environment["CERBENA_UPDATER_RUNTIME_LOG"] = (Join-Path $localAppDataRoot "Cerbena Browser\runtime_logs.log")
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "failed to launch updater e2e binary"
    }
    Write-Log "launched legacy updater pid=$($process.Id) exe=$copiedExePath"

    $deadline = [DateTime]::UtcNow.AddMinutes($TimeoutMinutes)
    $resolvedVersion = $null
    $lastHeartbeatAt = [DateTime]::UtcNow.AddSeconds(-10)
    $lastStoreStatus = ""
    $lastStoreError = ""
    while ([DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 500
        if (Test-Path $versionProbePath) {
            $resolvedVersion = (Get-Content -LiteralPath $versionProbePath -Raw).Trim()
            if (-not [string]::IsNullOrWhiteSpace($resolvedVersion)) {
                Write-Log "version probe resolved version=$resolvedVersion"
                break
            }
        }
        $store = Read-UpdateStoreSnapshot $localAppDataRoot
        $runtimeLogPath = Get-ExistingRuntimeLogPath $localAppDataRoot
        $storeStatus = Get-StoreFieldValue $store @("status")
        $storeError = Get-StoreFieldValue $store @("lastError", "last_error")
        if ($storeStatus -ne $lastStoreStatus -or $storeError -ne $lastStoreError) {
            Write-Log "store update status=$storeStatus error=$storeError runtimeLog=$runtimeLogPath"
            $lastStoreStatus = $storeStatus
            $lastStoreError = $storeError
        }
        if ([DateTime]::UtcNow -ge $lastHeartbeatAt.AddSeconds(5)) {
            $runtimeTail = (Get-FileTail $runtimeLogPath 5) -join " || "
            Write-Log "heartbeat exited=$($process.HasExited) status=$storeStatus probeExists=$(Test-Path $versionProbePath) runtimeTail=$runtimeTail"
            $lastHeartbeatAt = [DateTime]::UtcNow
        }
        if ($null -ne $store -and [string]$store.status -eq "up_to_date") {
            $storeVersion = Normalize-Version ([string]$store.latestVersion)
            if (-not [string]::IsNullOrWhiteSpace($storeVersion)) {
                Write-Log "resolved version from store latestVersion=$storeVersion"
                $resolvedVersion = $storeVersion
                break
            }
        }
        if ($process.HasExited -and -not (Test-Path $versionProbePath)) {
            if (Test-StoreStatusAllowsBackgroundRelaunch $storeStatus) {
                Write-Log "launcher process exited but background relaunch is still allowed by store status=$storeStatus"
                continue
            }
            $detail = if ($null -eq $store) {
                "updater process exited before writing the version probe"
            } else {
                "updater process exited early with status '$storeStatus' and error '$storeError'"
            }
            $runtimeTail = (Get-FileTail $runtimeLogPath 20) -join [Environment]::NewLine
            if (-not [string]::IsNullOrWhiteSpace($runtimeTail)) {
                Write-Log "runtime log tail before failure:`n$runtimeTail"
            }
            throw $detail
        }
    }

    if ([string]::IsNullOrWhiteSpace($resolvedVersion)) {
        $store = Read-UpdateStoreSnapshot $localAppDataRoot
        $status = Get-StoreFieldValue $store @("status") "missing"
        $lastError = Get-StoreFieldValue $store @("lastError", "last_error")
        $runtimeLogPath = Get-ExistingRuntimeLogPath $localAppDataRoot
        $runtimeTail = (Get-FileTail $runtimeLogPath 30) -join [Environment]::NewLine
        if (-not [string]::IsNullOrWhiteSpace($runtimeTail)) {
            Write-Log "runtime log tail on timeout:`n$runtimeTail"
        }
        throw "timed out waiting for the relaunched updated build to report its version; last updater status: $status; error: $lastError"
    }
    if (-not (Test-VersionEquivalent $resolvedVersion $publishedRelease.Version)) {
        throw "updated build reported version $resolvedVersion, expected published release $($publishedRelease.Version)"
    }

    $report = @{
        legacyVersion = $LegacyVersion
        publishedVersion = $publishedRelease.Version
        releaseUrl = $publishedRelease.HtmlUrl
        contractMode = $ContractMode
        msiAssetName = $publishedRelease.MsiAssetName
        updatedVersion = $resolvedVersion
    } | ConvertTo-Json -Depth 4
    Write-Utf8NoBomFile $reportPath $report
    Write-Log "published updater e2e passed legacy=$LegacyVersion updated=$resolvedVersion"
    Write-Host "Published updater e2e passed: $LegacyVersion -> $resolvedVersion" -ForegroundColor Green
} finally {
    Stop-ProcessesInRoot $installRoot
    Start-Sleep -Milliseconds 300
    if (-not $KeepArtifacts) {
        Remove-Item -LiteralPath $sessionRoot -Recurse -Force -ErrorAction SilentlyContinue
    } else {
        Write-Host "Kept updater e2e artifacts at $sessionRoot" -ForegroundColor Yellow
    }
}
