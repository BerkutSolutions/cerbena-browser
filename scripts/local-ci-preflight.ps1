param(
    [switch]$SkipCargoTest,
    [switch]$SkipDesktopRust,
    [switch]$SkipDocs,
    [switch]$SkipUi,
    [switch]$SkipPublishedUpdaterE2E,
    [switch]$SkipGitHygiene,
    [switch]$SkipDockerPreflight,
    [switch]$SkipSecurityGates,
    [switch]$SkipVulnerabilityGates,
    [switch]$SkipReleaseBuild,
    [switch]$CompactOutput
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Step([string]$Title, [scriptblock]$Action) {
    Write-Host ""
    Write-Host "== $Title ==" -ForegroundColor Cyan
    & $Action
}

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

function Invoke-DesktopDevSmoke([string]$WorkingDirectory, [int]$GraceSeconds = 12) {
    $stdoutPath = Join-Path ([System.IO.Path]::GetTempPath()) ("cerbena-dev-smoke-out-" + [guid]::NewGuid().ToString("N") + ".log")
    $stderrPath = Join-Path ([System.IO.Path]::GetTempPath()) ("cerbena-dev-smoke-err-" + [guid]::NewGuid().ToString("N") + ".log")
    $localAppDataRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("cerbena-dev-smoke-localappdata-" + [guid]::NewGuid().ToString("N"))
    $process = $null
    try {
        [System.IO.Directory]::CreateDirectory($localAppDataRoot) | Out-Null
        $startInfo = New-Object System.Diagnostics.ProcessStartInfo
        $startInfo.FileName = (Join-Path $env:SystemRoot "System32\cmd.exe")
        $startInfo.WorkingDirectory = $WorkingDirectory
        $startInfo.Arguments = "/d /c npm.cmd run dev"
        $startInfo.UseShellExecute = $false
        $startInfo.CreateNoWindow = $true
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true
        $startInfo.Environment["LOCALAPPDATA"] = $localAppDataRoot
        $process = New-Object System.Diagnostics.Process
        $process.StartInfo = $startInfo
        if (-not $process.Start()) {
            throw "failed to start desktop dev smoke process"
        }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $deadline = [DateTime]::UtcNow.AddSeconds($GraceSeconds)
        while ([DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 500
            if (-not $process.HasExited) {
                continue
            }
            $stdoutTask.Wait()
            $stderrTask.Wait()
            [System.IO.File]::WriteAllText($stdoutPath, $stdoutTask.Result)
            [System.IO.File]::WriteAllText($stderrPath, $stderrTask.Result)
            $stdoutTail = if ([string]::IsNullOrWhiteSpace($stdoutTask.Result)) { "" } else { (($stdoutTask.Result -split "`r?`n") | Select-Object -Last 40) -join [Environment]::NewLine }
            $stderrTail = if ([string]::IsNullOrWhiteSpace($stderrTask.Result)) { "" } else { (($stderrTask.Result -split "`r?`n") | Select-Object -Last 40) -join [Environment]::NewLine }
            $tail = @($stdoutTail, $stderrTail) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
            if ($process.ExitCode -ne 0) {
                throw "desktop dev smoke failed ($($process.ExitCode)): npm run dev`n$($tail -join [Environment]::NewLine)"
            }
            $combinedOutput = @($stdoutTask.Result, $stderrTask.Result) -join [Environment]::NewLine
            if ($combinedOutput -match "\[dev\]\[page-load\] window=main") {
                return
            }
            throw "desktop dev smoke exited too early before the grace window completed.`n$($tail -join [Environment]::NewLine)"
        }
    } finally {
        if ($null -ne $process -and -not $process.HasExited) {
            try {
                & taskkill /PID $process.Id /T /F *> $null
            } catch {
            }
        }
        Remove-Item -LiteralPath $stdoutPath -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $stderrPath -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $localAppDataRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

Push-Location $repoRoot
try {
    Step "Documentation contract" {
        $required = @(
            "README.md",
            "README.en.md",
            "docs\README.md",
            "docs\ru\README.md",
            "docs\eng\README.md",
            "docs\ru\core-docs\ui.md",
            "docs\eng\core-docs\ui.md",
            "docs\ru\release-runbook.md",
            "docs\eng\release-runbook.md",
            "scripts\local-ci-preflight.ps1"
        )
        foreach ($rel in $required) {
            $path = Join-Path $repoRoot $rel
            if (-not (Test-Path $path)) {
                throw "required file is missing: $rel"
            }
        }
    }

    if (-not $SkipGitHygiene) {
        Step "Git hygiene preflight" {
            Invoke-Native "powershell" @(
                "-ExecutionPolicy", "Bypass",
                "-File", "scripts/git-hygiene-preflight.ps1"
            ) -Quiet:$CompactOutput
        }
    }

    if (-not $SkipCargoTest) {
        Step "Rust workspace tests" {
            Invoke-Native "cargo" @("test", "--workspace") -Quiet:$CompactOutput
        }
    }

    if (-not $SkipDesktopRust) {
        Step "Desktop backend Rust tests" {
            Push-Location (Join-Path $repoRoot "ui\desktop\src-tauri")
            try {
                Invoke-Native "cargo" @("test") -Quiet:$CompactOutput
            } finally {
                Pop-Location
            }
        }
    }

    if (-not $SkipDesktopRust) {
        Step "Traffic isolation regression tests" {
            Push-Location (Join-Path $repoRoot "ui\desktop\src-tauri")
            try {
                Invoke-Native "cargo" @("test", "traffic_isolation") -Quiet:$CompactOutput
            } finally {
                Pop-Location
            }
        }
    }

    if (-not $SkipDesktopRust) {
        Step "Trusted updater regression tests" {
            Push-Location (Join-Path $repoRoot "ui\desktop\src-tauri")
            try {
                Invoke-Native "cargo" @("test", "trusted_updater") -Quiet:$CompactOutput
            } finally {
                Pop-Location
            }
        }
    }

    if (-not $SkipPublishedUpdaterE2E) {
        Step "Published updater end-to-end test" {
            Invoke-Native "powershell" @(
                "-ExecutionPolicy", "Bypass",
                "-File", "scripts/published-updater-e2e.ps1"
            ) -Quiet:$CompactOutput
        }
    }

    if (-not $SkipReleaseBuild) {
        Step "Launcher release build" {
            Invoke-Native "cargo" @("build", "-p", "cerbena-launcher", "--release") -Quiet:$CompactOutput
        }
    }

    if (-not $SkipDocs) {
        Step "Docs install and build" {
            Invoke-Native "npm.cmd" @("install") -Quiet:$CompactOutput
            Invoke-Native "npm.cmd" @("run", "docs:build") -Quiet:$CompactOutput
        }
    }

    if (-not $SkipUi) {
        Step "Desktop UI checks" {
            Push-Location (Join-Path $repoRoot "ui\desktop")
            try {
                Invoke-Native "npm.cmd" @("install") -Quiet:$CompactOutput
                Invoke-Native "npm.cmd" @("test") -Quiet:$CompactOutput
            } finally {
                Pop-Location
            }
        }
    }

    if (-not $SkipUi) {
        Step "Desktop UI dev smoke" {
            Invoke-DesktopDevSmoke (Join-Path $repoRoot "ui\desktop")
        }
    }

    if (-not $SkipDockerPreflight) {
        Step "Docker runtime preflight" {
            Invoke-Native "powershell" @(
                "-ExecutionPolicy", "Bypass",
                "-File", "scripts/docker-runtime-preflight.ps1"
            ) -Quiet:$CompactOutput
        }
    }

    if (-not $SkipSecurityGates) {
        Step "Security gates preflight" {
            Invoke-Native "powershell" @(
                "-ExecutionPolicy", "Bypass",
                "-File", "scripts/security-gates-preflight.ps1"
            ) -Quiet:$CompactOutput
        }
    }

    if (-not $SkipVulnerabilityGates) {
        Step "Vulnerability gates preflight" {
            Invoke-Native "powershell" @(
                "-ExecutionPolicy", "Bypass",
                "-File", "scripts/vulnerability-gates-preflight.ps1"
            ) -Quiet:$CompactOutput
        }
    }

    Write-Host ""
    Write-Host "Local CI preflight passed." -ForegroundColor Green
} finally {
    Pop-Location
}
