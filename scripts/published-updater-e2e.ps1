param(
    [string]$BaseVersion = "",
    [string]$BaseMsiPath = "",
    [string]$MinimumPublishedVersion = "",
    [string]$ExpectedPublishedVersion = "",
    [string]$ReleaseApiUrl = "",
    [string]$ReleaseVersion = "",
    [string]$ReleaseHtmlUrl = "",
    [string]$ReleaseMsiAssetName = "",
    [string]$MsiInstallDirOverride = "",
    [ValidateSet("msi_only")]
    [string]$ContractMode = "msi_only",
    [int]$TimeoutMinutes = 25,
    [switch]$CompactOutput,
    [switch]$KeepArtifacts
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$script:PublishedUpdaterTranscriptPath = Join-Path ([System.IO.Path]::GetTempPath()) ("cerbena-published-updater-e2e-transcript-" + [guid]::NewGuid().ToString("N") + ".log")
$script:PublishedUpdaterTranscriptStarted = $false

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

function Invoke-Native([string]$FilePath, [string[]]$Arguments = @(), [string]$WorkingDirectory = "", [hashtable]$Environment = @{}, [switch]$Quiet, [int]$TimeoutSeconds = 0, [string]$ProgressLabel = "", [int]$ProgressHeartbeatSeconds = 15) {
    Write-Log "spawn command file=$FilePath args=$($Arguments -join ' ') timeoutSeconds=$TimeoutSeconds quiet=$Quiet"
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
    $startedAt = [DateTime]::UtcNow
    $lastProgressAt = $startedAt
    while (-not $process.WaitForExit(1000)) {
        if ($TimeoutSeconds -gt 0 -and [DateTime]::UtcNow -ge $startedAt.AddSeconds($TimeoutSeconds)) {
            try {
                $process.Kill()
            } catch {
            }
            $stdout.Wait(5000)
            $stderr.Wait(5000)
            $tail = @($stdout.Result, $stderr.Result) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
            throw "command timed out after $TimeoutSeconds seconds: $FilePath $($Arguments -join ' ')`n$($tail -join [Environment]::NewLine)"
        }
        if (-not [string]::IsNullOrWhiteSpace($ProgressLabel) -and [DateTime]::UtcNow -ge $lastProgressAt.AddSeconds($ProgressHeartbeatSeconds)) {
            Write-Log "$ProgressLabel still running elapsedSeconds=$([int]([DateTime]::UtcNow - $startedAt).TotalSeconds)"
            $lastProgressAt = [DateTime]::UtcNow
        }
    }
    $stdout.Wait()
    $stderr.Wait()
    Write-CommandOutputToLog "stdout" $FilePath $stdout.Result
    Write-CommandOutputToLog "stderr" $FilePath $stderr.Result
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
    Write-Log "command completed exitCode=0 file=$FilePath elapsedSeconds=$([int]([DateTime]::UtcNow - $startedAt).TotalSeconds)"
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

function Write-CommandOutputToLog([string]$Kind, [string]$FilePath, [string]$Content) {
    if ([string]::IsNullOrWhiteSpace($Content)) {
        return
    }
    $header = "----- $Kind BEGIN ($FilePath) -----"
    $footer = "----- $Kind END ($FilePath) -----"
    Append-Utf8NoBomLine $script:SessionLogPath $header
    $normalized = $Content -replace "`r`n", "`n"
    foreach ($line in ($normalized -split "`n")) {
        Append-Utf8NoBomLine $script:SessionLogPath $line
    }
    Append-Utf8NoBomLine $script:SessionLogPath $footer
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
        ApiUrl = "https://api.github.com/repos/BerkutSolutions/cerbena-browser/releases/latest"
    }
}

function Resolve-ReleaseUnderTest() {
    if (-not [string]::IsNullOrWhiteSpace($ReleaseApiUrl)) {
        $resolvedVersion = Normalize-Version $ReleaseVersion
        if ([string]::IsNullOrWhiteSpace($resolvedVersion)) {
            throw "ReleaseVersion is required when ReleaseApiUrl is provided"
        }
        if ([string]::IsNullOrWhiteSpace($ReleaseMsiAssetName)) {
            throw "ReleaseMsiAssetName is required when ReleaseApiUrl is provided"
        }
        return @{
            Version = $resolvedVersion
            HtmlUrl = if ([string]::IsNullOrWhiteSpace($ReleaseHtmlUrl)) { $ReleaseApiUrl } else { $ReleaseHtmlUrl }
            MsiAssetName = $ReleaseMsiAssetName
            ApiUrl = $ReleaseApiUrl
        }
    }
    return Get-LatestPublishedRelease
}

function Install-MsiPackage([string]$MsiPath, [string]$InstallDir, [string]$LogPath, [int]$TimeoutSeconds = 120) {
    $args = @('/i', $MsiPath, '/qn', '/norestart', '/l*v', $LogPath)
    if (-not [string]::IsNullOrWhiteSpace($InstallDir)) {
        $args += ('INSTALLDIR="' + $InstallDir + '"')
    }
    $maxAttempts = 3
    for ($attempt = 1; $attempt -le $maxAttempts; $attempt++) {
        $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList $args -PassThru -WindowStyle Hidden
        if ($null -eq $process) {
            throw "failed to start msiexec for $MsiPath"
        }
        $null = $process.Handle
        $startedAt = [DateTime]::UtcNow
        $lastHeartbeat = $startedAt
        while (-not $process.WaitForExit(1000)) {
            if ([DateTime]::UtcNow -ge $startedAt.AddSeconds($TimeoutSeconds)) {
                try {
                    Start-Process -FilePath "taskkill.exe" -ArgumentList @("/PID", [string]$process.Id, "/T", "/F") -WindowStyle Hidden -Wait | Out-Null
                } catch {
                }
                try {
                    $process.Kill()
                } catch {
                }
                throw "msiexec timed out after $TimeoutSeconds seconds for $MsiPath; log: $LogPath"
            }
            if ([DateTime]::UtcNow -ge $lastHeartbeat.AddSeconds(10)) {
                Write-Log "base msi install still running elapsedSeconds=$([int]([DateTime]::UtcNow - $startedAt).TotalSeconds) attempt=$attempt/$maxAttempts"
                $lastHeartbeat = [DateTime]::UtcNow
            }
        }
        if ($process.ExitCode -eq 0 -or $process.ExitCode -eq 3010) {
            return
        }
        $msiLogTail = (Get-FileTail $LogPath 120) -join [Environment]::NewLine
        $retryableBusy = $false
        if (-not [string]::IsNullOrWhiteSpace($msiLogTail)) {
            $tailLower = $msiLogTail.ToLowerInvariant()
            if ($tailLower.Contains("error 2503") -or $tailLower.Contains("error 2502") -or $tailLower.Contains("inprogressinstallinfo.ipi") -or $tailLower.Contains("called runscript when not marked in progress")) {
                $retryableBusy = $true
            }
        }
        if ($attempt -lt $maxAttempts -and ($process.ExitCode -eq 1603 -or $process.ExitCode -eq 1618 -or $retryableBusy)) {
            Write-Log "base msi install retry scheduled attempt=$attempt exitCode=$($process.ExitCode) retryableBusy=$retryableBusy"
            Stop-InstallerAndCerbenaProcesses -InstallRoot $InstallDir
            Wait-InstallerIdle -TimeoutSeconds 20
            Start-Sleep -Seconds 2
            continue
        }
        throw "msiexec failed with exit code $($process.ExitCode) for $MsiPath; log: $LogPath"
    }
}

function Install-MsiAdministrativeImage([string]$MsiPath, [string]$TargetRoot, [string]$LogPath, [int]$TimeoutSeconds = 120) {
    if ([string]::IsNullOrWhiteSpace($MsiPath) -or -not (Test-Path -LiteralPath $MsiPath)) {
        throw "administrative image source MSI is missing: $MsiPath"
    }
    if ([string]::IsNullOrWhiteSpace($TargetRoot)) {
        throw "administrative image target root is empty"
    }
    [System.IO.Directory]::CreateDirectory($TargetRoot) | Out-Null
    $args = @('/a', $MsiPath, '/qn', '/norestart', ('TARGETDIR="' + $TargetRoot + '"'), '/l*v', $LogPath)
    Write-Log "starting MSI administrative extraction source=$MsiPath targetRoot=$TargetRoot log=$LogPath"
    $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList $args -PassThru -WindowStyle Hidden
    if ($null -eq $process) {
        throw "failed to start administrative MSI extraction for $MsiPath"
    }
    $null = $process.Handle
    $startedAt = [DateTime]::UtcNow
    $lastHeartbeat = $startedAt
    while (-not $process.WaitForExit(1000)) {
        if ([DateTime]::UtcNow -ge $startedAt.AddSeconds($TimeoutSeconds)) {
            try { Start-Process -FilePath "taskkill.exe" -ArgumentList @("/PID", [string]$process.Id, "/T", "/F") -WindowStyle Hidden -Wait | Out-Null } catch {}
            try { $process.Kill() } catch {}
            throw "MSI administrative extraction timed out after $TimeoutSeconds seconds: $MsiPath"
        }
        if ([DateTime]::UtcNow -ge $lastHeartbeat.AddSeconds(10)) {
            Write-Log "MSI administrative extraction still running elapsedSeconds=$([int]([DateTime]::UtcNow - $startedAt).TotalSeconds)"
            $lastHeartbeat = [DateTime]::UtcNow
        }
    }
    if ($process.ExitCode -ne 0 -and $process.ExitCode -ne 3010) {
        throw "MSI administrative extraction failed with exit code $($process.ExitCode): $MsiPath"
    }
    Write-Log "MSI administrative extraction completed exitCode=$($process.ExitCode) targetRoot=$TargetRoot"
}

function Get-MsiPropertyValue([string]$MsiPath, [string]$PropertyName) {
    if ([string]::IsNullOrWhiteSpace($MsiPath) -or -not (Test-Path -LiteralPath $MsiPath)) {
        return ""
    }
    $installer = $null
    $database = $null
    $view = $null
    $record = $null
    try {
        $installer = New-Object -ComObject WindowsInstaller.Installer
        $database = $installer.GetType().InvokeMember("OpenDatabase", "InvokeMethod", $null, $installer, @($MsiPath, 0))
        $query = "SELECT Value FROM Property WHERE Property = '$PropertyName'"
        $view = $database.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $database, ($query))
        $view.GetType().InvokeMember("Execute", "InvokeMethod", $null, $view, $null) | Out-Null
        $record = $view.GetType().InvokeMember("Fetch", "InvokeMethod", $null, $view, $null)
        if ($null -eq $record) {
            return ""
        }
        return [string]$record.GetType().InvokeMember("StringData", "GetProperty", $null, $record, 1)
    } catch {
        return ""
    } finally {
        if ($null -ne $view) {
            try { $view.GetType().InvokeMember("Close", "InvokeMethod", $null, $view, $null) | Out-Null } catch {}
        }
    }
}

function Remove-InstalledTempProductForMsi([string]$MsiPath) {
    $productCode = (Get-MsiPropertyValue -MsiPath $MsiPath -PropertyName "ProductCode").Trim()
    if ([string]::IsNullOrWhiteSpace($productCode)) {
        Write-Log "msi product code not resolved for cleanup: $MsiPath"
        return
    }
    $installer = $null
    try {
        $installer = New-Object -ComObject WindowsInstaller.Installer
        $state = [int]$installer.ProductState($productCode)
        if ($state -ne 5) {
            return
        }
        $installLocation = ""
        $versionString = ""
        try { $installLocation = [string]$installer.ProductInfo($productCode, "InstallLocation") } catch {}
        try { $versionString = [string]$installer.ProductInfo($productCode, "VersionString") } catch {}
        $normalizedInstallLocation = ""
        if (-not [string]::IsNullOrWhiteSpace($installLocation)) {
            try {
                $normalizedInstallLocation = [System.IO.Path]::GetFullPath($installLocation).TrimEnd('\')
            } catch {
                $normalizedInstallLocation = $installLocation.Trim()
            }
        }
        $tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\')
        $isTempScoped = (-not [string]::IsNullOrWhiteSpace($normalizedInstallLocation)) -and
            $normalizedInstallLocation.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase) -and
            $normalizedInstallLocation.ToLowerInvariant().Contains("cerbena-updater-e2e-")
        Write-Log "detected installed base productCode=$productCode version=$versionString installLocation=$installLocation tempScoped=$isTempScoped"
        if (-not $isTempScoped) {
            Write-Log "skipping uninstall for non-temp productCode=$productCode to avoid touching user installation"
            return
        }
        Write-Log "uninstalling temp-scoped product with matching base MSI productCode=$productCode before isolated session install"
        $args = @('/x', $productCode, '/qn', '/norestart')
        $proc = Start-Process -FilePath 'msiexec.exe' -ArgumentList $args -PassThru -WindowStyle Hidden
        if ($null -eq $proc) {
            throw "failed to start uninstall for product $productCode"
        }
        $null = $proc.Handle
        if (-not $proc.WaitForExit(90000)) {
            try { Start-Process -FilePath "taskkill.exe" -ArgumentList @("/PID", [string]$proc.Id, "/T", "/F") -WindowStyle Hidden -Wait | Out-Null } catch {}
            try { $proc.Kill() } catch {}
            throw "stale temp product uninstall timed out for product $productCode"
        }
        if ($proc.ExitCode -ne 0 -and $proc.ExitCode -ne 1605) {
            throw "stale temp product uninstall failed for product $productCode with exitCode=$($proc.ExitCode)"
        }
        Write-Log "stale temp e2e product cleanup completed productCode=$productCode exitCode=$($proc.ExitCode)"
    } catch {
        Write-Log "stale temp e2e product cleanup failed: $($_.Exception.Message)"
    }
}

function Assert-IsolatedInstallRoot([string]$SessionRoot, [string]$CandidateRoot) {
    if ([string]::IsNullOrWhiteSpace($CandidateRoot)) {
        return
    }
    $normalizedSessionRoot = [System.IO.Path]::GetFullPath($SessionRoot).TrimEnd('\')
    $normalizedCandidateRoot = [System.IO.Path]::GetFullPath($CandidateRoot).TrimEnd('\')
    $standardUserRoot = [System.IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA "Cerbena Browser")).TrimEnd('\')
    if ($normalizedCandidateRoot.StartsWith($standardUserRoot, [System.StringComparison]::OrdinalIgnoreCase) -and
        (-not $normalizedCandidateRoot.StartsWith($normalizedSessionRoot, [System.StringComparison]::OrdinalIgnoreCase))) {
        throw "forbidden install root detected in updater e2e: $normalizedCandidateRoot (standard user directory must not be used by test harness)"
    }
}

function Uninstall-InstalledProductByCode([string]$ProductCode, [int]$TimeoutSeconds = 120) {
    if ([string]::IsNullOrWhiteSpace($ProductCode)) {
        return
    }
    try {
        $installer = New-Object -ComObject WindowsInstaller.Installer
        $state = [int]$installer.ProductState($ProductCode)
        if ($state -ne 5) {
            return
        }
    } catch {
        return
    }
    Write-Log "attempting uninstall of test product productCode=$ProductCode"
    $proc = Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/x', $ProductCode, '/qn', '/norestart') -PassThru -WindowStyle Hidden
    if ($null -eq $proc) {
        Write-Log "failed to start uninstall process for productCode=$ProductCode"
        return
    }
    $null = $proc.Handle
    if (-not $proc.WaitForExit($TimeoutSeconds * 1000)) {
        try { Start-Process -FilePath "taskkill.exe" -ArgumentList @("/PID", [string]$proc.Id, "/T", "/F") -WindowStyle Hidden -Wait | Out-Null } catch {}
        try { $proc.Kill() } catch {}
        Write-Log "test product uninstall timed out productCode=$ProductCode"
        return
    }
    Write-Log "test product uninstall completed productCode=$ProductCode exitCode=$($proc.ExitCode)"
}

function Get-MsiProductCode([string]$MsiPath) {
    if ([string]::IsNullOrWhiteSpace($MsiPath)) {
        return ""
    }
    try {
        return (Get-MsiPropertyValue -MsiPath $MsiPath -PropertyName "ProductCode").Trim()
    } catch {
        return ""
    }
}

function Resolve-InstallDirFromMsiLog([string]$LogPath) {
    if ([string]::IsNullOrWhiteSpace($LogPath) -or -not (Test-Path -LiteralPath $LogPath)) {
        return ""
    }
    try {
        $lines = Get-Content -LiteralPath $LogPath
        for ($index = $lines.Count - 1; $index -ge 0; $index--) {
            $line = [string]$lines[$index]
            if ($line -match 'Property\(S\): INSTALLDIR = (.+)$') {
                $value = $matches[1].Trim()
                if (-not [string]::IsNullOrWhiteSpace($value)) {
                    return $value.Trim('"')
                }
            }
        }
    } catch {
    }
    return ""
}

function Mirror-InstallRoot([string]$SourceRoot, [string]$TargetRoot) {
    if ([string]::IsNullOrWhiteSpace($SourceRoot) -or -not (Test-Path -LiteralPath $SourceRoot)) {
        throw "mirror source root is missing: $SourceRoot"
    }
    [System.IO.Directory]::CreateDirectory($TargetRoot) | Out-Null
    $files = Get-ChildItem -LiteralPath $SourceRoot -Recurse -File -ErrorAction SilentlyContinue
    foreach ($file in $files) {
        $relative = ""
        $relativePathMethod = [System.IO.Path].GetMethod("GetRelativePath", [type[]]@([string], [string]))
        if ($null -ne $relativePathMethod) {
            $relative = [string]$relativePathMethod.Invoke($null, @($SourceRoot, $file.FullName))
        } else {
            $base = [System.IO.Path]::GetFullPath($SourceRoot)
            if (-not $base.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
                $base += [System.IO.Path]::DirectorySeparatorChar
            }
            $baseUri = New-Object System.Uri($base)
            $fileUri = New-Object System.Uri([System.IO.Path]::GetFullPath($file.FullName))
            $relative = [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($fileUri).ToString()).Replace('/', [System.IO.Path]::DirectorySeparatorChar)
        }
        $targetPath = Join-Path $TargetRoot $relative
        $targetDirectory = Split-Path -Parent $targetPath
        if (-not [string]::IsNullOrWhiteSpace($targetDirectory)) {
            [System.IO.Directory]::CreateDirectory($targetDirectory) | Out-Null
        }
        Copy-Item -LiteralPath $file.FullName -Destination $targetPath -Force
    }
}

function Resolve-BaseRuntimeFallbackRoot([string]$RepoRoot, [string]$BaseVersion, [string]$BaseMsiPath) {
    $candidates = @(
        (Join-Path (Split-Path -Parent $BaseMsiPath) ("..\..\release\" + $BaseVersion + "\staging\cerbena-windows-x64")),
        (Join-Path $RepoRoot ("build\release\" + $BaseVersion + "\staging\cerbena-windows-x64"))
    )
    foreach ($candidate in $candidates) {
        try {
            $resolved = [System.IO.Path]::GetFullPath($candidate)
            $exe = Join-Path $resolved "cerbena.exe"
            if (Test-Path -LiteralPath $exe) {
                return $resolved
            }
        } catch {
        }
    }
    return ""
}

function Reset-SessionUpdaterState([string]$InstallRoot) {
    if ([string]::IsNullOrWhiteSpace($InstallRoot) -or -not (Test-Path -LiteralPath $InstallRoot)) {
        return
    }
    $pathsToRemove = @(
        (Join-Path $InstallRoot "app_update_store.json"),
        (Join-Path $InstallRoot "runtime_logs.log"),
        (Join-Path $InstallRoot "updates")
    )
    foreach ($path in $pathsToRemove) {
        try {
            if (Test-Path -LiteralPath $path) {
                Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction SilentlyContinue
                Write-Log "reset session updater state removed path=$path"
            }
        } catch {
        }
    }
}

function Seed-E2EUpdateStore([string]$LocalAppDataRoot) {
    if ([string]::IsNullOrWhiteSpace($LocalAppDataRoot)) {
        return
    }
    $storePath = Join-Path $LocalAppDataRoot "Cerbena Browser\app_update_store.json"
    $storeDir = Split-Path -Parent $storePath
    if (-not [string]::IsNullOrWhiteSpace($storeDir)) {
        [System.IO.Directory]::CreateDirectory($storeDir) | Out-Null
    }
    $seed = @{
        auto_update_enabled = $true
        status = "idle"
        has_update = $false
    } | ConvertTo-Json -Depth 4
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($storePath, $seed, $utf8)
    Write-Log "seeded updater store for e2e auto_update_enabled=true path=$storePath"
}

function Stop-InstallerAndCerbenaProcesses([string]$InstallRoot = "") {
    $normalizedInstallRoot = ""
    if (-not [string]::IsNullOrWhiteSpace($InstallRoot)) {
        try {
            $normalizedInstallRoot = [System.IO.Path]::GetFullPath($InstallRoot)
        } catch {
            $normalizedInstallRoot = $InstallRoot
        }
    }
    $shouldStopMsiexecPid = {
        param($ProcessId)
        try {
            $proc = Get-CimInstance Win32_Process -Filter ("ProcessId = " + [string]$ProcessId) -ErrorAction SilentlyContinue
            if ($null -eq $proc) {
                return $false
            }
            $commandLine = [string]$proc.CommandLine
            if ([string]::IsNullOrWhiteSpace($commandLine)) {
                return $false
            }
            $commandLower = $commandLine.ToLowerInvariant()
            if ($commandLower.Contains("cerbena")) {
                return $true
            }
            if (-not [string]::IsNullOrWhiteSpace($normalizedInstallRoot)) {
                return $commandLower.Contains($normalizedInstallRoot.ToLowerInvariant())
            }
            return $false
        } catch {
            return $false
        }
    }
    foreach ($process in @(Get-Process -ErrorAction SilentlyContinue)) {
        try {
            if ($process.ProcessName -ieq "msiexec") {
                if (& $shouldStopMsiexecPid $process.Id) {
                    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
                }
                continue
            }
            if ([string]::IsNullOrWhiteSpace($process.Path)) {
                continue
            }
            $name = [System.IO.Path]::GetFileNameWithoutExtension($process.Path)
            $isCerbenaBinary = $name -like "cerbena*"
            $insideInstallRoot = $false
            if (-not [string]::IsNullOrWhiteSpace($normalizedInstallRoot)) {
                $normalizedProcessPath = [System.IO.Path]::GetFullPath($process.Path)
                $insideInstallRoot = $normalizedProcessPath.StartsWith($normalizedInstallRoot, [System.StringComparison]::OrdinalIgnoreCase)
            }
            if ($isCerbenaBinary -or $insideInstallRoot) {
                Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            }
        } catch {
        }
    }
    Start-Sleep -Milliseconds 350
}

function Wait-InstallerIdle([int]$TimeoutSeconds = 30) {
    $isActiveInstallerPid = {
        param([int]$ProcessId)
        try {
            $proc = Get-CimInstance Win32_Process -Filter ("ProcessId = " + [string]$ProcessId) -ErrorAction SilentlyContinue
            if ($null -eq $proc) {
                return $false
            }
            $commandLine = [string]$proc.CommandLine
            if ([string]::IsNullOrWhiteSpace($commandLine)) {
                return $false
            }
            $lower = $commandLine.ToLowerInvariant()
            return (
                $lower.Contains(".msi") -or
                $lower.Contains("/i ") -or
                $lower.Contains("/x ") -or
                $lower.Contains("/update ") -or
                $lower.Contains("/package ")
            )
        } catch {
            return $false
        }
    }
    $deadline = [DateTime]::UtcNow.AddSeconds([Math]::Max(5, $TimeoutSeconds))
    while ([DateTime]::UtcNow -lt $deadline) {
        $activeMsiexec = @(
            Get-Process -Name "msiexec" -ErrorAction SilentlyContinue | Where-Object {
                try {
                    (-not $_.HasExited) -and (& $isActiveInstallerPid $_.Id)
                } catch {
                    $false
                }
            }
        )
        $inProgressPath = "C:\Windows\Installer\inprogressinstallinfo.ipi"
        $inProgressExists = Test-Path -LiteralPath $inProgressPath
        if (@($activeMsiexec).Count -eq 0 -and -not $inProgressExists) {
            return $true
        }
        Start-Sleep -Milliseconds 500
    }
    Write-Log "installer idle wait timed out; forcing msiexec cleanup"
    foreach ($proc in @(Get-Process -Name "msiexec" -ErrorAction SilentlyContinue | Where-Object { & $isActiveInstallerPid $_.Id })) {
        try { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue } catch {}
    }
    Start-Sleep -Seconds 2
    $activeAfterKill = @(Get-Process -Name "msiexec" -ErrorAction SilentlyContinue)
    if (@($activeAfterKill).Count -gt 0) {
        Write-Log "installer idle wait timed out and msiexec remains active count=$(@($activeAfterKill).Count); continuing with guarded install retries"
        return $false
    }
    return $true
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

function Resolve-StagedMsiLogPath($Store) {
    if ($null -eq $Store) {
        return ""
    }
    $assetPath = Get-StoreFieldValue $Store @("stagedAssetPath", "staged_asset_path")
    if ([string]::IsNullOrWhiteSpace($assetPath)) {
        return ""
    }
    if ($assetPath.ToLowerInvariant().EndsWith(".msi")) {
        return ($assetPath.Substring(0, $assetPath.Length - 4) + ".msiexec.log")
    }
    return ""
}

function Find-MsiexecForMsiPath([string]$MsiPath) {
    if ([string]::IsNullOrWhiteSpace($MsiPath)) {
        return @()
    }
    $needle = $MsiPath.ToLowerInvariant()
    try {
        return @(Get-CimInstance Win32_Process -Filter "Name = 'msiexec.exe'" -ErrorAction SilentlyContinue | Where-Object {
            $cmd = [string]$_.CommandLine
            -not [string]::IsNullOrWhiteSpace($cmd) -and $cmd.ToLowerInvariant().Contains($needle)
        })
    } catch {
        return @()
    }
}

function Format-ProcessSnapshot([string]$MsiPath = "", [switch]$IncludeAllMsiexec) {
    $lines = @()
    try {
        $needle = ""
        if (-not [string]::IsNullOrWhiteSpace($MsiPath)) {
            $needle = $MsiPath.ToLowerInvariant()
        }
        $processes = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object {
            $_.Name -ieq "msiexec.exe" -or $_.Name -ieq "powershell.exe"
        })
        foreach ($proc in $processes) {
            $cmd = [string]$proc.CommandLine
            if ($proc.Name -ieq "msiexec.exe" -and -not $IncludeAllMsiexec -and -not [string]::IsNullOrWhiteSpace($needle)) {
                if ([string]::IsNullOrWhiteSpace($cmd) -or -not $cmd.ToLowerInvariant().Contains($needle)) {
                    continue
                }
            }
            $lines += ("pid={0} name={1} parent={2} cmd={3}" -f $proc.ProcessId, $proc.Name, $proc.ParentProcessId, $cmd)
        }
    } catch {
    }
    return ($lines -join [Environment]::NewLine)
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$currentVersion = Resolve-WorkspaceVersion $repoRoot
$publishedRelease = Resolve-ReleaseUnderTest
if (-not [string]::IsNullOrWhiteSpace($ExpectedPublishedVersion) -and -not (Test-VersionEquivalent $publishedRelease.Version $ExpectedPublishedVersion)) {
    throw "latest published release $($publishedRelease.Version) does not match expected version $ExpectedPublishedVersion"
}
if ([string]::IsNullOrWhiteSpace($BaseVersion)) {
    throw "BaseVersion is required for MSI updater e2e"
}
if ([string]::IsNullOrWhiteSpace($MinimumPublishedVersion)) {
    $MinimumPublishedVersion = $BaseVersion
}
if ((Compare-SemVer $publishedRelease.Version $MinimumPublishedVersion) -lt 0) {
    throw "latest published release $($publishedRelease.Version) is older than required minimum $MinimumPublishedVersion"
}
if ([string]::IsNullOrWhiteSpace($BaseMsiPath) -or -not (Test-Path -LiteralPath $BaseMsiPath)) {
    throw "BaseMsiPath is required for MSI updater e2e and must exist: $BaseMsiPath"
}
if ((Compare-SemVer $publishedRelease.Version $BaseVersion) -le 0) {
    throw "base MSI version $BaseVersion must be older than published release $($publishedRelease.Version)"
}

$sessionRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("cerbena-updater-e2e-" + [guid]::NewGuid().ToString("N"))
$localAppDataRoot = Join-Path $sessionRoot "localappdata"
$installRoot = Join-Path $localAppDataRoot "Cerbena Browser"
$versionProbePath = Join-Path $sessionRoot "updated-version.txt"
$reportPath = Join-Path $sessionRoot "report.json"
$script:SessionLogPath = Join-Path $sessionRoot "published-updater-e2e.log"

try {
    try {
        Start-Transcript -Path $script:PublishedUpdaterTranscriptPath -Force | Out-Null
        $script:PublishedUpdaterTranscriptStarted = $true
    } catch {
    }
    Remove-InstalledTempProductForMsi -MsiPath $BaseMsiPath
    Wait-InstallerIdle -TimeoutSeconds 45
    Stop-InstallerAndCerbenaProcesses -InstallRoot $installRoot
    [System.IO.Directory]::CreateDirectory($localAppDataRoot) | Out-Null
    [System.IO.Directory]::CreateDirectory($installRoot) | Out-Null
    Write-Log "session root: $sessionRoot"
    Write-Log "workspace version: $currentVersion"
    Write-Log "base version: $BaseVersion"
    Write-Log "base msi path: $BaseMsiPath"
    Write-Log "published version: $($publishedRelease.Version)"
    Write-Log "published release url: $($publishedRelease.HtmlUrl)"
    Write-Log "release api url: $($publishedRelease.ApiUrl)"
    Write-Log "published updater transcript path: $script:PublishedUpdaterTranscriptPath"
    $baseMsiLogPath = Join-Path $sessionRoot ("base-install-" + $BaseVersion + ".msiexec.log")
    $baseMsiAdminLogPath = Join-Path $sessionRoot ("base-admin-extract-" + $BaseVersion + ".msiexec.log")
    Write-Host "Installing base MSI ($BaseVersion)..." -ForegroundColor Cyan
    Write-Log "installing base msi version=$BaseVersion path=$BaseMsiPath installRoot=$installRoot"
    $usedBaseRuntimeFallback = $false
    $adminExtractResolvedExe = ""
    try {
        Install-MsiPackage -MsiPath $BaseMsiPath -InstallDir $installRoot -LogPath $baseMsiLogPath -TimeoutSeconds ($TimeoutMinutes * 60)
    } catch {
        $installError = $_.Exception.Message
        $allowFallback = $false
        if (-not [string]::IsNullOrWhiteSpace($installError)) {
            $errorLower = $installError.ToLowerInvariant()
            $allowFallback = (
                $errorLower.Contains("exit code 1603") -or
                $errorLower.Contains("error 2503") -or
                $errorLower.Contains("error 2502") -or
                $errorLower.Contains("inprogressinstallinfo.ipi")
            )
        }
        if (-not $allowFallback) {
            throw
        }
        $adminExtractSucceeded = $false
        try {
            Install-MsiAdministrativeImage -MsiPath $BaseMsiPath -TargetRoot $installRoot -LogPath $baseMsiAdminLogPath -TimeoutSeconds 90
            $adminExtractExe = Get-ChildItem -LiteralPath $installRoot -Recurse -File -Filter "cerbena.exe" -ErrorAction SilentlyContinue |
                Select-Object -First 1 -ExpandProperty FullName
            if (Test-Path -LiteralPath $adminExtractExe) {
                $adminExtractResolvedExe = $adminExtractExe
                Write-Log "base runtime prepared via administrative MSI extraction exe=$adminExtractExe installRoot=$installRoot log=$baseMsiAdminLogPath"
                $adminExtractSucceeded = $true
            } else {
                Write-Log "administrative MSI extraction completed but cerbena.exe was not found under $installRoot"
            }
        } catch {
            Write-Log "administrative MSI extraction fallback failed: $($_.Exception.Message)"
        }
        if (-not $adminExtractSucceeded) {
            $fallbackRoot = Resolve-BaseRuntimeFallbackRoot -RepoRoot $repoRoot -BaseVersion $BaseVersion -BaseMsiPath $BaseMsiPath
            if ([string]::IsNullOrWhiteSpace($fallbackRoot)) {
                throw
            }
            Write-Log "base msi install failed with transient installer contention; using fallback base runtime root: $fallbackRoot"
            Mirror-InstallRoot -SourceRoot $fallbackRoot -TargetRoot $installRoot
        }
        $usedBaseRuntimeFallback = $true
    }
    $effectiveInstallRoot = $installRoot
    $installedExePath = Join-Path $effectiveInstallRoot "cerbena.exe"
    if (-not [string]::IsNullOrWhiteSpace($adminExtractResolvedExe) -and (Test-Path -LiteralPath $adminExtractResolvedExe)) {
        $installedExePath = $adminExtractResolvedExe
        $effectiveInstallRoot = Split-Path -Parent $adminExtractResolvedExe
        Write-Log "using administrative extraction runtime root: $effectiveInstallRoot"
    }
    if (-not (Test-Path -LiteralPath $installedExePath)) {
        $resolvedFromLog = Resolve-InstallDirFromMsiLog $baseMsiLogPath
        if (-not [string]::IsNullOrWhiteSpace($resolvedFromLog)) {
            Assert-IsolatedInstallRoot -SessionRoot $installRoot -CandidateRoot $resolvedFromLog
            $resolvedCandidate = Join-Path $resolvedFromLog "cerbena.exe"
            if (Test-Path -LiteralPath $resolvedCandidate) {
                $effectiveInstallRoot = $resolvedFromLog
                $installedExePath = $resolvedCandidate
                Write-Log "base msi installed into alternate root from log: $effectiveInstallRoot"
            }
        }
    }
    Assert-IsolatedInstallRoot -SessionRoot $installRoot -CandidateRoot $effectiveInstallRoot
    if (-not (Test-Path -LiteralPath $installedExePath)) {
        throw "base MSI install completed but cerbena.exe is missing: $installedExePath"
    }
    $normalizedSessionInstallRoot = [System.IO.Path]::GetFullPath($installRoot).TrimEnd('\')
    $normalizedEffectiveInstallRoot = [System.IO.Path]::GetFullPath($effectiveInstallRoot).TrimEnd('\')
    if (-not $normalizedEffectiveInstallRoot.StartsWith($normalizedSessionInstallRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        Write-Log "base msi resolved install root is outside current session; mirroring runtime into session root source=$normalizedEffectiveInstallRoot target=$normalizedSessionInstallRoot"
        Mirror-InstallRoot -SourceRoot $effectiveInstallRoot -TargetRoot $installRoot
        $mirroredExe = Get-ChildItem -LiteralPath $installRoot -Recurse -File -Filter "cerbena.exe" -ErrorAction SilentlyContinue |
            Select-Object -First 1 -ExpandProperty FullName
        if (Test-Path -LiteralPath $mirroredExe) {
            $effectiveInstallRoot = Split-Path -Parent $mirroredExe
            $installedExePath = $mirroredExe
            Write-Log "mirrored runtime to session root and switched launch path: $installedExePath"
        } else {
            throw "failed to mirror base runtime into session root; cerbena.exe is missing under: $installRoot"
        }
    }
    $installRoot = $effectiveInstallRoot
    Reset-SessionUpdaterState -InstallRoot $installRoot
    Seed-E2EUpdateStore -LocalAppDataRoot $localAppDataRoot
    if ($usedBaseRuntimeFallback) {
        Write-Log "base runtime fallback prepared exe=$installedExePath installRoot=$installRoot (msi log path retained at $baseMsiLogPath)"
    } else {
        Write-Log "base msi installed exe=$installedExePath installRoot=$installRoot log=$baseMsiLogPath"
    }

    $runtimeLogPath = (Join-Path $localAppDataRoot "Cerbena Browser\runtime_logs.log")
    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $installedExePath
    $startInfo.Arguments = "--updater"
    $startInfo.WorkingDirectory = $installRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.Environment["LOCALAPPDATA"] = $localAppDataRoot
    $startInfo.Environment["CERBENA_SELFTEST_REPORT_VERSION_FILE"] = $versionProbePath
    $startInfo.Environment["CERBENA_UPDATER_RUNTIME_LOG"] = $runtimeLogPath
    $startInfo.Environment["CERBENA_RELEASE_LATEST_API_URL"] = [string]$publishedRelease.ApiUrl
    $msiHelperTimeoutMs = [Math]::Min(120000, [Math]::Max(15000, ($TimeoutMinutes * 60 * 1000) - 20000))
    $startInfo.Environment["CERBENA_UPDATER_MSI_TIMEOUT_MS"] = [string]$msiHelperTimeoutMs
    Write-Log "msi helper timeout override: ${msiHelperTimeoutMs}ms"
    $resolvedMsiInstallDirOverride = $MsiInstallDirOverride
    if ($resolvedMsiInstallDirOverride -eq "__SESSION_LOCALAPPDATA__") {
        # Keep MSI helper install target aligned with the actual runtime root selected above.
        # This is critical when base runtime came from administrative extraction and landed
        # in a nested directory such as "...\\LocalApp\\Cerbena Browser".
        $resolvedMsiInstallDirOverride = $installRoot
    }
    if (-not [string]::IsNullOrWhiteSpace($resolvedMsiInstallDirOverride)) {
        $startInfo.Environment["CERBENA_UPDATER_MSI_INSTALL_DIR"] = $resolvedMsiInstallDirOverride
        Write-Log "msi install dir override: $resolvedMsiInstallDirOverride"
    }
    $updaterFallbackExePath = Join-Path $installRoot "cerbena-updater.exe"
    Stop-InstallerAndCerbenaProcesses -InstallRoot $installRoot
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "failed to launch updater e2e binary"
    }
    Write-Log "launched base msi updater pid=$($process.Id) exe=$installedExePath"

    $deadline = [DateTime]::UtcNow.AddMinutes($TimeoutMinutes)
    $flowStartupDeadline = [DateTime]::UtcNow.AddSeconds(20)
    $fallbackFlowStartupDeadline = [DateTime]::UtcNow.AddSeconds(45)
    $fallbackUpdaterLaunchAttempted = $false
    $resolvedVersion = $null
    $lastHeartbeatAt = [DateTime]::UtcNow.AddSeconds(-10)
    $lastStoreStatus = ""
    $lastStoreError = ""
    $backgroundRelaunchExitAt = $null
    $backgroundRelaunchDeadline = $deadline
    $applyingObservedAt = $null
    $lastApplyingDiagAt = $null
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
        if (-not $fallbackUpdaterLaunchAttempted -and [DateTime]::UtcNow -ge $flowStartupDeadline) {
            $runtimeTailShort = (Get-FileTail $runtimeLogPath 5) -join " || "
            $hasStoreState = -not [string]::IsNullOrWhiteSpace($storeStatus)
            if (-not $hasStoreState -and -not $process.HasExited -and (Test-Path -LiteralPath $updaterFallbackExePath)) {
                Write-Log "updater store state did not appear from cerbena.exe --updater within startup window; switching to fallback binary $updaterFallbackExePath runtimeTail=$runtimeTailShort"
                try { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue } catch {}
                Start-Sleep -Milliseconds 250
                $fallbackStartInfo = New-Object System.Diagnostics.ProcessStartInfo
                $fallbackStartInfo.FileName = $updaterFallbackExePath
                $fallbackStartInfo.Arguments = "--updater"
                $fallbackStartInfo.WorkingDirectory = $installRoot
                $fallbackStartInfo.UseShellExecute = $false
                $fallbackStartInfo.CreateNoWindow = $true
                $fallbackStartInfo.Environment["LOCALAPPDATA"] = $localAppDataRoot
                $fallbackStartInfo.Environment["CERBENA_SELFTEST_REPORT_VERSION_FILE"] = $versionProbePath
                $fallbackStartInfo.Environment["CERBENA_UPDATER_RUNTIME_LOG"] = $runtimeLogPath
                $fallbackStartInfo.Environment["CERBENA_RELEASE_LATEST_API_URL"] = [string]$publishedRelease.ApiUrl
                $fallbackStartInfo.Environment["CERBENA_UPDATER_MSI_TIMEOUT_MS"] = [string]$msiHelperTimeoutMs
                if (-not [string]::IsNullOrWhiteSpace($resolvedMsiInstallDirOverride)) {
                    $fallbackStartInfo.Environment["CERBENA_UPDATER_MSI_INSTALL_DIR"] = $resolvedMsiInstallDirOverride
                }
                $fallbackProcess = New-Object System.Diagnostics.Process
                $fallbackProcess.StartInfo = $fallbackStartInfo
                if (-not $fallbackProcess.Start()) {
                    throw "failed to launch fallback updater binary: $updaterFallbackExePath"
                }
                $process = $fallbackProcess
                Write-Log "launched fallback updater pid=$($process.Id) exe=$updaterFallbackExePath"
                $fallbackUpdaterLaunchAttempted = $true
                $fallbackFlowStartupDeadline = [DateTime]::UtcNow.AddSeconds(30)
            }
        }
        if ($fallbackUpdaterLaunchAttempted -and [DateTime]::UtcNow -ge $fallbackFlowStartupDeadline) {
            $hasStoreStateAfterFallback = -not [string]::IsNullOrWhiteSpace($storeStatus)
            if (-not $hasStoreStateAfterFallback) {
                $runtimeTailFallback = (Get-FileTail $runtimeLogPath 40) -join [Environment]::NewLine
                if (-not [string]::IsNullOrWhiteSpace($runtimeTailFallback)) {
                    Write-Log "runtime log tail before fallback no-store-state failure:`n$runtimeTailFallback"
                }
                throw "fallback updater did not produce store state within startup window"
            }
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
                if ($null -eq $backgroundRelaunchExitAt) {
                    $backgroundRelaunchExitAt = [DateTime]::UtcNow
                }
                if ($storeStatus -eq "applying") {
                    if ($null -eq $applyingObservedAt) {
                        $applyingObservedAt = [DateTime]::UtcNow
                        $lastApplyingDiagAt = [DateTime]::UtcNow.AddSeconds(-11)
                    }
                    $stagedMsiLogPath = Resolve-StagedMsiLogPath $store
                    $stagedMsiPath = Get-StoreFieldValue $store @("stagedAssetPath", "staged_asset_path")
                    $matchingMsiexec = @(Find-MsiexecForMsiPath $stagedMsiPath)
                    if ($null -eq $lastApplyingDiagAt -or [DateTime]::UtcNow -ge $lastApplyingDiagAt.AddSeconds(10)) {
                        $msiLogExists = Test-Path -LiteralPath $stagedMsiLogPath
                        $msiLogSize = ""
                        if ($msiLogExists) {
                            try {
                                $msiLogSize = (Get-Item -LiteralPath $stagedMsiLogPath -ErrorAction Stop).Length
                            } catch {
                                $msiLogSize = "unknown"
                            }
                        }
                        Write-Log "applying diagnostics elapsedSeconds=$([int]([DateTime]::UtcNow - $applyingObservedAt).TotalSeconds) matchingMsiexecCount=$(@($matchingMsiexec).Count) stagedMsiLogExists=$msiLogExists stagedMsiLogSizeBytes=$msiLogSize stagedMsiPath=$stagedMsiPath"
                        $lastApplyingDiagAt = [DateTime]::UtcNow
                    }
                    if (@($matchingMsiexec).Count -eq 0 -and [DateTime]::UtcNow -ge $applyingObservedAt.AddSeconds(20)) {
                        $msiTail = (Get-FileTail $stagedMsiLogPath 40) -join [Environment]::NewLine
                        if (-not [string]::IsNullOrWhiteSpace($msiTail)) {
                            Write-Log "msi log tail before applying-without-msiexec:`n$msiTail"
                        }
                        $processSnapshot = Format-ProcessSnapshot -MsiPath $stagedMsiPath -IncludeAllMsiexec
                        if (-not [string]::IsNullOrWhiteSpace($processSnapshot)) {
                            Write-Log "process snapshot before applying-without-msiexec:`n$processSnapshot"
                        }
                        Write-Log "matching msiexec process was not found for staged MSI path; waiting for helper/store state to resolve"
                    }
                    $applyingWithoutMsiexecAbortAt = $applyingObservedAt.AddSeconds(25)
                    if (@($matchingMsiexec).Count -eq 0 -and [DateTime]::UtcNow -ge $applyingWithoutMsiexecAbortAt) {
                        $msiTail = (Get-FileTail $stagedMsiLogPath 60) -join [Environment]::NewLine
                        if (-not [string]::IsNullOrWhiteSpace($msiTail)) {
                            Write-Log "msi log tail before applying-without-msiexec-timeout:`n$msiTail"
                        }
                        $processSnapshot = Format-ProcessSnapshot -MsiPath $stagedMsiPath -IncludeAllMsiexec
                        if (-not [string]::IsNullOrWhiteSpace($processSnapshot)) {
                            Write-Log "process snapshot before applying-without-msiexec-timeout:`n$processSnapshot"
                        }
                        throw "updater helper applying stalled: no matching msiexec process detected for staged MSI"
                    }
                    $hardApplyingStallAbortAt = $applyingObservedAt.AddSeconds(60)
                    if ([DateTime]::UtcNow -ge $hardApplyingStallAbortAt) {
                        foreach ($procInfo in @($matchingMsiexec)) {
                            try { Stop-Process -Id ([int]$procInfo.ProcessId) -Force -ErrorAction SilentlyContinue } catch {}
                        }
                        $msiTail = (Get-FileTail $stagedMsiLogPath 60) -join [Environment]::NewLine
                        if (-not [string]::IsNullOrWhiteSpace($msiTail)) {
                            Write-Log "msi log tail before hard applying stall abort:`n$msiTail"
                        }
                        $processSnapshot = Format-ProcessSnapshot -MsiPath $stagedMsiPath -IncludeAllMsiexec
                        if (-not [string]::IsNullOrWhiteSpace($processSnapshot)) {
                            Write-Log "process snapshot before hard applying stall abort:`n$processSnapshot"
                        }
                        throw "updater helper applying stalled beyond 60 seconds and was aborted"
                    }
                    $applyingAbortAt = $applyingObservedAt.AddMilliseconds($msiHelperTimeoutMs + 30000)
                    if ($applyingAbortAt -gt $backgroundRelaunchDeadline) {
                        $applyingAbortAt = $backgroundRelaunchDeadline
                    }
                    if ([DateTime]::UtcNow -ge $applyingAbortAt) {
                        foreach ($procInfo in @($matchingMsiexec)) {
                            try { Stop-Process -Id ([int]$procInfo.ProcessId) -Force -ErrorAction SilentlyContinue } catch {}
                        }
                        $msiTail = (Get-FileTail $stagedMsiLogPath 60) -join [Environment]::NewLine
                        if (-not [string]::IsNullOrWhiteSpace($msiTail)) {
                            Write-Log "msi log tail before applying timeout kill:`n$msiTail"
                        }
                        $processSnapshot = Format-ProcessSnapshot $stagedMsiPath
                        if (-not [string]::IsNullOrWhiteSpace($processSnapshot)) {
                            Write-Log "process snapshot before applying timeout kill:`n$processSnapshot"
                        }
                        throw "updater helper applying exceeded computed timeout and was aborted"
                    }
                } else {
                    $applyingObservedAt = $null
                    $lastApplyingDiagAt = $null
                }
                if ($storeStatus -eq "error") {
                    $runtimeTailLines = Get-FileTail $runtimeLogPath 40
                    $runtimeTail = $runtimeTailLines -join [Environment]::NewLine
                    if (-not [string]::IsNullOrWhiteSpace($runtimeTail)) {
                        Write-Log "runtime log tail before relaunch store-error:`n$runtimeTail"
                    }
                    throw "updater helper reported error while waiting for relaunch: $storeError"
                }
                $runtimeTailLines = Get-FileTail $runtimeLogPath 20
                $runtimeTail = $runtimeTailLines -join [Environment]::NewLine
                if ($runtimeTail -match "relaunch skipped because executable is missing") {
                    Write-Log "runtime log tail before relaunch failure:`n$runtimeTail"
                    throw "updater helper applied the MSI but could not relaunch the installed executable"
                }
                if ([DateTime]::UtcNow -ge $backgroundRelaunchDeadline) {
                    if (-not [string]::IsNullOrWhiteSpace($runtimeTail)) {
                        Write-Log "runtime log tail before relaunch timeout:`n$runtimeTail"
                    }
                    throw "updater helper did not relaunch the updated build before timeout; last updater status: $storeStatus; last error: $storeError"
                }
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
        baseVersion = $BaseVersion
        baseMsiPath = $BaseMsiPath
        publishedVersion = $publishedRelease.Version
        releaseUrl = $publishedRelease.HtmlUrl
        contractMode = $ContractMode
        msiAssetName = $publishedRelease.MsiAssetName
        updatedVersion = $resolvedVersion
    } | ConvertTo-Json -Depth 4
    Write-Utf8NoBomFile $reportPath $report
    Write-Log "published updater e2e passed base=$BaseVersion updated=$resolvedVersion"
    Write-Host "Published updater e2e passed: $BaseVersion -> $resolvedVersion" -ForegroundColor Green
} finally {
    try {
        $baseProductCode = Get-MsiProductCode $BaseMsiPath
        if (-not [string]::IsNullOrWhiteSpace($baseProductCode)) {
            Uninstall-InstalledProductByCode -ProductCode $baseProductCode -TimeoutSeconds 120
        }
        $storeSnapshotForCleanup = Read-UpdateStoreSnapshot $localAppDataRoot
        $stagedMsiForCleanup = Get-StoreFieldValue $storeSnapshotForCleanup @("stagedAssetPath", "staged_asset_path")
        if (-not [string]::IsNullOrWhiteSpace($stagedMsiForCleanup) -and $stagedMsiForCleanup.ToLowerInvariant().EndsWith(".msi")) {
            $updatedProductCode = Get-MsiProductCode $stagedMsiForCleanup
            if (-not [string]::IsNullOrWhiteSpace($updatedProductCode) -and $updatedProductCode -ne $baseProductCode) {
                Uninstall-InstalledProductByCode -ProductCode $updatedProductCode -TimeoutSeconds 120
            }
        }
    } catch {
        Write-Log "final test product cleanup failed: $($_.Exception.Message)"
    }
    Stop-InstallerAndCerbenaProcesses -InstallRoot $installRoot
    Stop-ProcessesInRoot $installRoot
    Start-Sleep -Milliseconds 300
    if (-not $KeepArtifacts) {
        Remove-Item -LiteralPath $sessionRoot -Recurse -Force -ErrorAction SilentlyContinue
    } else {
        Write-Host "Kept updater e2e artifacts at $sessionRoot" -ForegroundColor Yellow
    }
    if ($script:PublishedUpdaterTranscriptStarted) {
        try { Stop-Transcript | Out-Null } catch {}
    }
}
