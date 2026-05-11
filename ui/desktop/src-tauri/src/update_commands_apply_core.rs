use super::*;

pub(crate) fn launch_zip_apply_helper_impl(
    pid: u32,
    archive_path: &Path,
    install_root: &Path,
    relaunch_executable: Option<&Path>,
    runtime_log_path: Option<&str>,
) -> Result<(), String> {
    let helper = build_zip_apply_helper_script_impl(
        pid,
        archive_path,
        install_root,
        relaunch_executable,
        runtime_log_path,
    );
    Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &helper,
        ])
        .spawn()
        .map_err(|e| format!("spawn zip update helper: {e}"))?;
    Ok(())
}

pub(crate) fn build_zip_apply_helper_script_impl(
    pid: u32,
    archive_path: &Path,
    install_root: &Path,
    relaunch_executable: Option<&Path>,
    runtime_log_path: Option<&str>,
) -> String {
    let relaunch = relaunch_executable
        .map(powershell_quote)
        .unwrap_or_else(|| "$null".to_string());
    let runtime_log = runtime_log_path
        .map(|value| powershell_quote(Path::new(value)))
        .unwrap_or_else(|| "$null".to_string());
    format!(
        "$pidValue={pid};\
        $archive={archive};\
        $installRoot={install};\
        $relaunchExe={relaunch};\
        $runtimeLogPath={runtime_log};\
        $versionProbe=$env:CERBENA_SELFTEST_REPORT_VERSION_FILE;\
        $autoExitAfter='20';\
        $targetExecutables=@('cerbena.exe','browser-desktop-ui.exe','cerbena-updater.exe');\
        function Write-Log([string]$message) {{\
            if (-not $runtimeLogPath -or [string]::IsNullOrWhiteSpace($runtimeLogPath)) {{ return }};\
            try {{\
                $directory = Split-Path -Parent $runtimeLogPath;\
                if ($directory) {{ [System.IO.Directory]::CreateDirectory($directory) | Out-Null }};\
                [System.IO.File]::AppendAllText($runtimeLogPath, ('[' + [DateTime]::UtcNow.ToString('o') + '] [updater-helper][zip] ' + $message + [Environment]::NewLine), (New-Object System.Text.UTF8Encoding($false)));\
            }} catch {{}}\
        }};\
        Write-Log ('helper started pid=' + $pidValue + ' archive=' + $archive + ' installRoot=' + $installRoot);\
        while (Get-Process -Id $pidValue -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 250 }};\
        Write-Log 'launcher process exited; starting zip apply';\
        $targetPaths=@();\
        foreach ($exeName in $targetExecutables) {{\
            $candidate=Join-Path $installRoot $exeName;\
            if (Test-Path -LiteralPath $candidate) {{\
                $targetPaths += [System.IO.Path]::GetFullPath($candidate);\
            }}\
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
        $temp=Join-Path ([System.IO.Path]::GetTempPath()) ('cerbena-update-' + [guid]::NewGuid().ToString('N'));\
        New-Item -ItemType Directory -Path $temp -Force | Out-Null;\
        try {{\
            Write-Log ('expanding archive to temp=' + $temp);\
            Expand-Archive -LiteralPath $archive -DestinationPath $temp -Force;\
            $source=$temp;\
            $entries=Get-ChildItem -LiteralPath $temp;\
            if ($entries.Count -eq 1 -and $entries[0].PSIsContainer) {{ $source=$entries[0].FullName }};\
            $copySucceeded=$false;\
            for ($attempt=0; $attempt -lt 10 -and -not $copySucceeded; $attempt++) {{\
                try {{\
                    Get-ChildItem -LiteralPath $source | ForEach-Object {{ Copy-Item -LiteralPath $_.FullName -Destination $installRoot -Recurse -Force }};\
                    $copySucceeded=$true;\
                    Write-Log ('copy succeeded attempt=' + ($attempt + 1));\
                }} catch {{\
                    Write-Log ('copy failed attempt=' + ($attempt + 1) + ' error=' + $_.Exception.Message);\
                    if ($attempt -ge 9) {{ throw }};\
                    Start-Sleep -Milliseconds 500;\
                }}\
            }};\
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
                Write-Log 'relaunch skipped because executable is missing';\
            }}\
        }} catch {{\
            Write-Log ('helper failed: ' + $_.Exception.Message);\
            throw;\
        }} finally {{\
            Write-Log 'cleaning temporary extraction directory';\
            if (Test-Path -LiteralPath $temp) {{ Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue }}\
        }}",
        pid = pid,
        archive = powershell_quote(archive_path),
        install = powershell_quote(install_root),
        relaunch = relaunch,
        runtime_log = runtime_log,
        helper_log_env = UPDATER_HELPER_LOG_ENV,
        auto_exit_env = UPDATER_RELAUNCH_AUTO_EXIT_ENV
    )
}

pub(crate) fn launch_msi_apply_helper_impl(
    pid: u32,
    msi_path: &Path,
    target_install_root: Option<&Path>,
    update_store_path: Option<&str>,
    target_version: Option<&str>,
    runtime_log_path: Option<&str>,
) -> Result<(), String> {
    let helper = build_msi_apply_helper_script_impl(
        pid,
        msi_path,
        target_install_root,
        update_store_path,
        target_version,
        runtime_log_path,
    );
    Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &helper,
        ])
        .spawn()
        .map_err(|e| format!("spawn msi update helper: {e}"))?;
    Ok(())
}

pub(crate) fn build_msi_apply_helper_script_impl(
    pid: u32,
    msi_path: &Path,
    target_install_root: Option<&Path>,
    update_store_path: Option<&str>,
    target_version: Option<&str>,
    runtime_log_path: Option<&str>,
) -> String {
    support::build_msi_apply_helper_script_impl(
        pid,
        msi_path,
        target_install_root,
        update_store_path,
        target_version,
        runtime_log_path,
    )
}

#[path = "update_commands_apply_core_support.rs"]
mod support;
pub(crate) use support::*;


