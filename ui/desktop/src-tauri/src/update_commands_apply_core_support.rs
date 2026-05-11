use super::*;

pub(crate) fn build_msi_apply_helper_script_impl(
    pid: u32,
    msi_path: &Path,
    target_install_root: Option<&Path>,
    update_store_path: Option<&str>,
    target_version: Option<&str>,
    runtime_log_path: Option<&str>,
) -> String {
    let store = update_store_path
        .map(|value| powershell_quote(Path::new(value)))
        .unwrap_or_else(|| "$null".to_string());
    let version = target_version
        .map(|value| format!("'{}'", value.replace('\'', "''")))
        .unwrap_or_else(|| "$null".to_string());
    let install_root = target_install_root
        .map(powershell_quote)
        .unwrap_or_else(|| "$null".to_string());
    let runtime_log = runtime_log_path
        .map(|value| powershell_quote(Path::new(value)))
        .unwrap_or_else(|| "$null".to_string());
    format!(
        "$ErrorActionPreference='Stop';\
        $pidValue={pid};\
        $msiPath={msi};\
        $installRoot={install};\
        $storePath={store};\
        $targetVersion={version};\
        $runtimeLogPath={runtime_log};\
        $msiLogPath=([System.IO.Path]::ChangeExtension($msiPath, '.msiexec.log'));\
        $msiInstallDirOverride=$env:{install_dir_env};\
        $msiWaitTimeoutMs = 120000;\
        if ($env:{msi_timeout_env}) {{\
            try {{\
                $parsedTimeout = [int]$env:{msi_timeout_env};\
                if ($parsedTimeout -ge 15000) {{ $msiWaitTimeoutMs = $parsedTimeout }};\
            }} catch {{}}\
        }};\
        $versionProbe=$env:CERBENA_SELFTEST_REPORT_VERSION_FILE;\
        $autoExitAfter='20';\
        $targetExecutables=@('cerbena.exe','browser-desktop-ui.exe','cerbena-updater.exe','cerbena-launcher.exe');\
        function Write-Log([string]$message) {{\
            if (-not $runtimeLogPath -or [string]::IsNullOrWhiteSpace($runtimeLogPath)) {{ return }};\
            try {{\
                $directory = Split-Path -Parent $runtimeLogPath;\
                if ($directory) {{ [System.IO.Directory]::CreateDirectory($directory) | Out-Null }};\
                [System.IO.File]::AppendAllText($runtimeLogPath, ('[' + [DateTime]::UtcNow.ToString('o') + '] [updater-helper][msi] ' + $message + [Environment]::NewLine), (New-Object System.Text.UTF8Encoding($false)));\
            }} catch {{}}\
        }};\
        function Describe-MsiExit([int]$code) {{\
            switch ($code) {{\
                1602 {{ return 'msi install canceled before completion' }}\
                1618 {{ return 'another Windows Installer transaction is already running (1618)' }}\
                3010 {{ return 'msi install completed and requested a relaunch (3010)' }}\
                default {{ return ('msi install failed with exit code ' + $code) }}\
            }}\
        }};\
        function Resolve-RelaunchExecutable() {{\
            if (-not $installRoot -or [string]::IsNullOrWhiteSpace($installRoot) -or -not (Test-Path -LiteralPath $installRoot)) {{ return $null }};\
            foreach ($exeName in @('cerbena.exe','browser-desktop-ui.exe')) {{\
                $candidate = Join-Path $installRoot $exeName;\
                if (Test-Path -LiteralPath $candidate) {{ return $candidate }};\
            }};\
            return $null;\
        }};\
        function Update-Store([string]$status, [string]$lastError, [bool]$pendingApply) {{\
            if (-not $storePath -or [string]::IsNullOrWhiteSpace($storePath) -or -not (Test-Path -LiteralPath $storePath)) {{ return }};\
            try {{\
                $json = Get-Content -LiteralPath $storePath -Raw | ConvertFrom-Json;\
                $json.status = $status;\
                $json.lastError = if ([string]::IsNullOrWhiteSpace($lastError)) {{ $null }} else {{ $lastError }};\
                $json.pendingApplyOnExit = $pendingApply;\
                if ($targetVersion) {{ $json.stagedVersion = $targetVersion }};\
                $updated = $json | ConvertTo-Json -Depth 8;\
                [System.IO.File]::WriteAllText($storePath, $updated, (New-Object System.Text.UTF8Encoding($false)));\
                Write-Log ('store updated status=' + $status + ' pending=' + $pendingApply + ' error=' + $lastError);\
            }} catch {{}}\
        }};\
        try {{\
        Write-Log ('helper started pid=' + $pidValue + ' msi=' + $msiPath + ' installRoot=' + $installRoot + ' installDirOverride=' + $msiInstallDirOverride + ' store=' + $storePath + ' targetVersion=' + $targetVersion);\
        while (Get-Process -Id $pidValue -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 250 }};\
        Write-Log 'launcher process exited; starting msi apply';\
        Update-Store 'applying' $null $false;\
        $targetPaths=@();\
        if ($installRoot -and (Test-Path -LiteralPath $installRoot)) {{\
            foreach ($exeName in $targetExecutables) {{\
                $candidate=Join-Path $installRoot $exeName;\
                if (Test-Path -LiteralPath $candidate) {{\
                    $targetPaths += [System.IO.Path]::GetFullPath($candidate);\
                }}\
            }};\
        }};\
        $runningTargets=@(Get-Process -ErrorAction SilentlyContinue | Where-Object {{\
            $_.Id -ne $PID -and $_.Id -ne $pidValue\
        }} | Where-Object {{\
            try {{\
                $processPath=$_.Path;\
                $processPath -and ($targetPaths -contains [System.IO.Path]::GetFullPath($processPath))\
            }} catch {{\
                $false\
            }}\
        }});\
        foreach ($proc in $runningTargets) {{\
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue;\
        }};\
        Write-Log ('stopped running targets count=' + $runningTargets.Count);\
        foreach ($proc in $runningTargets) {{\
            try {{\
                $proc.WaitForExit(15000) | Out-Null;\
            }} catch {{}}\
        }};\
        $msiArgs=@('/i', $msiPath, '/qn', '/norestart', '/l*v', $msiLogPath);\
        if ($msiInstallDirOverride -and -not [string]::IsNullOrWhiteSpace($msiInstallDirOverride)) {{\
            $msiArgs += ('INSTALLDIR=\"' + $msiInstallDirOverride + '\"');\
        }};\
        $attempt = 0;\
        $maxAttempts = 3;\
        $exitCode = -1;\
        $completed = $false;\
        while (-not $completed -and $attempt -lt $maxAttempts) {{\
            $attempt++;\
            Write-Log ('invoking msiexec attempt=' + $attempt + ' path=' + $msiPath + ' log=' + $msiLogPath + ' installRoot=' + $installRoot + ' installDirOverride=' + $msiInstallDirOverride);\
            $existingMsiexec = @(Get-CimInstance Win32_Process -Filter \"Name = 'msiexec.exe'\" -ErrorAction SilentlyContinue);\
            Write-Log ('msiexec pre-spawn processCount=' + $existingMsiexec.Count + ' attempt=' + $attempt);\
            $proc = $null;\
            try {{\
                $proc = Start-Process -FilePath 'msiexec.exe' -ArgumentList $msiArgs -WindowStyle Hidden -PassThru -ErrorAction Stop;\
            }} catch {{\
                $spawnError = $_.Exception.Message;\
                Write-Log ('msiexec spawn failed attempt=' + $attempt + ' error=' + $spawnError);\
                Update-Store 'error' ('msiexec spawn failed: ' + $spawnError + '; verbose log: ' + $msiLogPath) $false;\
                exit 125;\
            }};\
            if ($null -eq $proc) {{\
                Write-Log ('msiexec spawn returned null attempt=' + $attempt);\
                Update-Store 'error' ('msiexec spawn returned null process; verbose log: ' + $msiLogPath) $false;\
                exit 126;\
            }};\
            $null = $proc.Handle;\
            $timedOut = $false;\
            Write-Log ('msiexec spawned pid=' + $proc.Id + ' attempt=' + $attempt);\
            $waitStartedAt = [DateTime]::UtcNow;\
            $lastWaitHeartbeatAt = $waitStartedAt.AddSeconds(-2);\
            while (-not $proc.WaitForExit(1000)) {{\
                $elapsedMs = [int]([DateTime]::UtcNow - $waitStartedAt).TotalMilliseconds;\
                if ($elapsedMs -ge $msiWaitTimeoutMs) {{\
                    $timedOut = $true;\
                    break;\
                }};\
                if ([DateTime]::UtcNow -ge $lastWaitHeartbeatAt.AddSeconds(2)) {{\
                    Write-Log ('msiexec wait heartbeat attempt=' + $attempt + ' pid=' + $proc.Id + ' elapsedMs=' + $elapsedMs + ' timeoutMs=' + $msiWaitTimeoutMs);\
                    $lastWaitHeartbeatAt = [DateTime]::UtcNow;\
                }};\
            }};\
            if ($timedOut) {{\
                Write-Log ('msiexec timed out after ' + $msiWaitTimeoutMs + 'ms attempt=' + $attempt + ' pid=' + $proc.Id + ' log=' + $msiLogPath);\
                try {{ Start-Process -FilePath 'taskkill.exe' -ArgumentList @('/PID', [string]$proc.Id, '/T', '/F') -WindowStyle Hidden -Wait | Out-Null }} catch {{}};\
                try {{ $proc.Kill() }} catch {{}};\
                try {{ $proc.WaitForExit(10000) | Out-Null }} catch {{}};\
            }};\
            if ($timedOut) {{\
                Update-Store 'error' ('msiexec timed out; verbose log: ' + $msiLogPath) $false;\
                exit 124;\
            }};\
            $exitCode = $proc.ExitCode;\
            Write-Log ('msiexec completed attempt=' + $attempt + ' exitCode=' + $exitCode + ' log=' + $msiLogPath);\
            if (Test-Path -LiteralPath $msiLogPath) {{\
                try {{\
                    $msiLogSize = (Get-Item -LiteralPath $msiLogPath -ErrorAction Stop).Length;\
                    Write-Log ('msiexec log detected sizeBytes=' + $msiLogSize + ' attempt=' + $attempt + ' path=' + $msiLogPath);\
                }} catch {{\
                    Write-Log ('msiexec log stat failed attempt=' + $attempt + ' path=' + $msiLogPath + ' error=' + $_.Exception.Message);\
                }};\
            }} else {{\
                Write-Log ('msiexec log missing after completion attempt=' + $attempt + ' path=' + $msiLogPath);\
            }};\
            if ($exitCode -eq 1618 -and $attempt -lt $maxAttempts) {{\
                Write-Log ('msiexec returned 1618; retrying attempt=' + ($attempt + 1));\
                Start-Sleep -Seconds 5;\
                continue;\
            }};\
            $completed = $true;\
        }};\
        if (-not $completed) {{\
            Update-Store 'error' ('msiexec did not complete after retries; verbose log: ' + $msiLogPath) $false;\
            exit 1618;\
        }};\
        if ($exitCode -eq 1602) {{\
            Update-Store 'canceled' ((Describe-MsiExit $exitCode) + '; verbose log: ' + $msiLogPath) $false;\
            exit $exitCode;\
        }};\
        if ($exitCode -ne 0 -and $exitCode -ne 3010) {{\
            Update-Store 'error' ((Describe-MsiExit $exitCode) + '; verbose log: ' + $msiLogPath) $false;\
            exit $exitCode;\
        }};\
        if (Test-Path -LiteralPath $msiLogPath) {{\
            try {{\
                $logLines = Get-Content -LiteralPath $msiLogPath -ErrorAction Stop;\
                for ($index = $logLines.Count - 1; $index -ge 0; $index--) {{\
                    $line = [string]$logLines[$index];\
                    if ($line -match 'Property\\(S\\): INSTALLDIR = (.+)$') {{\
                        $candidateRoot = $matches[1].Trim().Trim('\"');\
                        if (-not [string]::IsNullOrWhiteSpace($candidateRoot) -and (Test-Path -LiteralPath $candidateRoot)) {{\
                            $installRoot = $candidateRoot;\
                            Write-Log ('resolved install root from msi log installRoot=' + $installRoot);\
                            break;\
                        }}\
                    }}\
                }}\
            }} catch {{\
                Write-Log ('failed to resolve install root from msi log: ' + $_.Exception.Message);\
            }}\
        }};\
        $relaunchExe = Resolve-RelaunchExecutable;\
        if ($targetVersion -and -not [string]::IsNullOrWhiteSpace($targetVersion) -and $relaunchExe -and (Test-Path -LiteralPath $relaunchExe)) {{\
            try {{\
                $resolvedVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($relaunchExe).ProductVersion;\
                if (-not [string]::IsNullOrWhiteSpace($resolvedVersion)) {{\
                    $normalizedResolvedVersion = ($resolvedVersion -replace '[^0-9\\.]', ' ').Split(' ')[0].Trim();\
                    if (-not [string]::IsNullOrWhiteSpace($normalizedResolvedVersion) -and -not $normalizedResolvedVersion.StartsWith($targetVersion)) {{\
                        $versionError = 'msi apply completed but relaunch executable version mismatch expected=' + $targetVersion + ' actual=' + $resolvedVersion + ' exe=' + $relaunchExe;\
                        Write-Log $versionError;\
                        Update-Store 'error' ($versionError + '; verbose log: ' + $msiLogPath) $false;\
                        exit 42;\
                    }}\
                }}\
            }} catch {{\
                Write-Log ('failed to inspect relaunch executable version: ' + $_.Exception.Message);\
            }}\
        }};\
        Update-Store 'applied_pending_relaunch' $null $false;\
        if ($relaunchExe -and (Test-Path -LiteralPath $relaunchExe)) {{\
            $relaunchInfo = New-Object System.Diagnostics.ProcessStartInfo;\
            $relaunchInfo.FileName = $relaunchExe;\
            $relaunchInfo.WorkingDirectory = Split-Path -Parent $relaunchExe;\
            $relaunchInfo.UseShellExecute = $false;\
            if ($versionProbe) {{\
                $relaunchInfo.EnvironmentVariables['CERBENA_SELFTEST_REPORT_VERSION_FILE'] = $versionProbe;\
                $relaunchInfo.EnvironmentVariables['{auto_exit_env}'] = $autoExitAfter;\
            }};\
            if ($runtimeLogPath) {{\
                $relaunchInfo.EnvironmentVariables['{helper_log_env}'] = $runtimeLogPath;\
            }};\
            [System.Diagnostics.Process]::Start($relaunchInfo) | Out-Null;\
            Write-Log ('relaunch started exe=' + $relaunchExe);\
        }} else {{\
            Write-Log ('relaunch skipped because executable is missing installRoot=' + $installRoot);\
        }};\
        }} catch {{\
            $message = $_.Exception.Message;\
            Write-Log ('helper exception: ' + $message);\
            Update-Store 'error' ('helper exception: ' + $message + '; verbose log: ' + $msiLogPath) $false;\
            exit 1;\
        }}",
        pid = pid,
        msi = powershell_quote(msi_path),
        install = install_root,
        store = store,
        version = version,
        runtime_log = runtime_log,
        install_dir_env = UPDATER_MSI_INSTALL_DIR_ENV,
        msi_timeout_env = UPDATER_MSI_TIMEOUT_MS_ENV,
        helper_log_env = UPDATER_HELPER_LOG_ENV,
        auto_exit_env = UPDATER_RELAUNCH_AUTO_EXIT_ENV
    )
}

pub(crate) fn resolve_relaunch_executable_path_impl(install_root: &Path) -> Option<PathBuf> {
    let candidates = [
        install_root.join("cerbena.exe"),
        install_root.join("browser-desktop-ui.exe"),
    ];
    candidates.into_iter().find(|path| path.is_file())
}
