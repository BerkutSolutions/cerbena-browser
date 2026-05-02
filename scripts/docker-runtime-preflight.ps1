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

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

Push-Location $repoRoot
try {
    Step "Docker CLI availability" {
        Assert-CommandAvailable "docker" "Install Docker Desktop and ensure docker.exe is available in PATH."
    }

    Step "Docker engine availability" {
        $serverVersion = Invoke-Native "docker" @("version", "--format", "{{.Server.Version}}") -Quiet:$CompactOutput
        if ([string]::IsNullOrWhiteSpace($serverVersion)) {
            $serverVersion = Invoke-Native "docker" @("info", "--format", "{{.ServerVersion}}") -Quiet:$CompactOutput
        }
        if ([string]::IsNullOrWhiteSpace($serverVersion)) {
            throw "Docker engine did not report a server version."
        }
        Write-Host ("Docker server version: " + $serverVersion) -ForegroundColor DarkGray
    }

    Step "Container runtime source contract" {
        $runtimeSource = Join-Path $repoRoot "ui\desktop\src-tauri\src\network_sandbox_container_runtime.rs"
        if (-not (Test-Path $runtimeSource)) {
            throw "missing container runtime source: $runtimeSource"
        }
        $content = Get-Content -Path $runtimeSource -Raw
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
