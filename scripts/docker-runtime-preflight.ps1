param(
    [switch]$SkipNetworkProbe,
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
            return ($output | Out-String).Trim()
        }
        $output = & $FilePath @Arguments
        $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
        if ($exitCode -ne 0) {
            $argsText = ($Arguments -join " ")
            throw "command failed ($exitCode): $FilePath $argsText"
        }
        return ($output | Out-String).Trim()
    } finally {
        $ErrorActionPreference = $prevErrorAction
        if ($hasNativePref) {
            $PSNativeCommandUseErrorActionPreference = $prevNativePref
        }
    }
}

function Assert-CommandAvailable([string]$CommandName, [string]$Hint) {
    if (-not (Get-Command $CommandName -ErrorAction SilentlyContinue)) {
        throw "$CommandName is required for Docker runtime preflight. $Hint"
    }
}

function Test-DockerAccessDenied([string]$Message) {
    if ([string]::IsNullOrWhiteSpace($Message)) {
        return $false
    }
    return $Message -match "Access is denied" -or
        $Message -match "permission denied while trying to connect to the docker API" -or
        $Message -match "open .*\.docker\\config\.json: Access is denied"
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

Push-Location $repoRoot
try {
    $script:dockerEngineAvailable = $true

    Step "Docker CLI availability" {
        Assert-CommandAvailable "docker" "Install Docker Desktop and ensure docker.exe is available in PATH."
    }

    Step "Docker engine availability" {
        $prevErrorAction = $ErrorActionPreference
        $hasNativePref = $null -ne (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)
        if ($hasNativePref) {
            $prevNativePref = $PSNativeCommandUseErrorActionPreference
            $PSNativeCommandUseErrorActionPreference = $false
        }
        $ErrorActionPreference = "Continue"
        try {
            $serverVersionOutput = & docker version --format "{{.Server.Version}}" 2>&1
        } finally {
            $ErrorActionPreference = $prevErrorAction
            if ($hasNativePref) {
                $PSNativeCommandUseErrorActionPreference = $prevNativePref
            }
        }
        $serverVersionExitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
        $serverVersion = ($serverVersionOutput | Out-String).Trim()
        if ($serverVersionExitCode -ne 0) {
            if (Test-DockerAccessDenied $serverVersion) {
                Write-Warning "Skipping Docker engine checks: access to local Docker daemon is denied in this environment."
                $script:dockerEngineAvailable = $false
                return
            }
            throw "command failed ($serverVersionExitCode): docker version --format {{.Server.Version}}`n$serverVersion"
        }
        if ([string]::IsNullOrWhiteSpace($serverVersion)) {
            $prevErrorAction = $ErrorActionPreference
            $hasNativePref = $null -ne (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)
            if ($hasNativePref) {
                $prevNativePref = $PSNativeCommandUseErrorActionPreference
                $PSNativeCommandUseErrorActionPreference = $false
            }
            $ErrorActionPreference = "Continue"
            try {
                $infoOutput = & docker info --format "{{.ServerVersion}}" 2>&1
            } finally {
                $ErrorActionPreference = $prevErrorAction
                if ($hasNativePref) {
                    $PSNativeCommandUseErrorActionPreference = $prevNativePref
                }
            }
            $infoExitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
            $serverVersion = ($infoOutput | Out-String).Trim()
            if ($infoExitCode -ne 0) {
                if (Test-DockerAccessDenied $serverVersion) {
                    Write-Warning "Skipping Docker engine checks: access to local Docker daemon is denied in this environment."
                    $script:dockerEngineAvailable = $false
                    return
                }
                throw "command failed ($infoExitCode): docker info --format {{.ServerVersion}}`n$serverVersion"
            }
        }
        if ([string]::IsNullOrWhiteSpace($serverVersion)) {
            throw "Docker engine did not report a server version."
        }
        Write-Host ("Docker server version: " + $serverVersion) -ForegroundColor DarkGray
    }

    Step "Container runtime source contract" {
        $runtimeSources = @(
            "ui\desktop\src-tauri\src\network_sandbox_container_runtime.rs",
            "ui\desktop\src-tauri\src\network_sandbox_container_runtime_core.rs",
            "ui\desktop\src-tauri\src\network_sandbox_container_runtime_core_ops.rs",
            "ui\desktop\src-tauri\src\network_sandbox_container_runtime_core_ops_openvpn.rs",
            "ui\desktop\src-tauri\src\network_sandbox_container_runtime_core_ops_singbox.rs",
            "ui\desktop\src-tauri\src\network_sandbox_container_runtime_core_ops_amnezia.rs"
        )
        $existingSources = @(
            $runtimeSources |
                ForEach-Object { Join-Path $repoRoot $_ } |
                Where-Object { Test-Path -LiteralPath $_ }
        )
        if ($existingSources.Count -eq 0) {
            throw "missing container runtime source set: $($runtimeSources -join ', ')"
        }
        $content = ($existingSources | ForEach-Object { Get-Content -Path $_ -Raw }) -join "`n"
        foreach ($needle in @(
            "cerbena.kind=network-sandbox-runtime",
            "cerbena.profile_id",
            "cerbena/network-sandbox"
        )) {
            if (-not $content.Contains($needle)) {
                throw "container runtime source is missing required marker: $needle"
            }
        }
    }

    if (-not $SkipNetworkProbe) {
        Step "Managed Docker network probe" {
            if (-not $script:dockerEngineAvailable) {
                Write-Warning "Skipping managed Docker network probe because Docker daemon access is unavailable."
                return
            }
            $networkName = "cerbena-preflight-" + [Guid]::NewGuid().ToString("N")
            $networkCreated = $false
            try {
                try {
                    Invoke-Native "docker" @("network", "create", $networkName) -Quiet:$CompactOutput | Out-Null
                } catch {
                    $message = $_.Exception.Message
                    if ($message -match "could not find plugin bridge" -or $message -match "plugin not found") {
                        Write-Warning "Skipping managed Docker network probe because the current Docker runner does not expose the bridge network plugin."
                        return
                    }
                    throw
                }
                $networkCreated = $true
                $networks = Invoke-Native "docker" @("network", "ls", "--format", "{{.Name}}") -Quiet:$CompactOutput
                if (-not ($networks -split "`r?`n" | Where-Object { $_ -eq $networkName })) {
                    throw "managed Docker network probe did not create $networkName"
                }
            } finally {
                if ($networkCreated) {
                    try {
                        Invoke-Native "docker" @("network", "rm", $networkName) -Quiet:$true | Out-Null
                    } catch {
                        Write-Warning "failed to remove Docker preflight network ${networkName}: $($_.Exception.Message)"
                    }
                }
            }
        }
    }

    Write-Host ""
    Write-Host "Docker runtime preflight passed." -ForegroundColor Green
} finally {
    Pop-Location
}
