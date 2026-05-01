param(
    [switch]$SkipWorkspaceTests,
    [switch]$SkipDocsBuild,
    [switch]$SkipUiChecks,
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

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

Push-Location $repoRoot
try {
    Step "TASKS4 security artifacts contract" {
        $required = @(
            "docs\eng\operators\security-validation.md",
            "docs\ru\operators\security-validation.md",
            "cmd\launcher\tests\docs_quality_tests.rs",
            "cmd\launcher\tests\launcher_stability_tests.rs",
            "scripts\git-hygiene-preflight.ps1"
        )
        foreach ($rel in $required) {
            $path = Join-Path $repoRoot $rel
            if (-not (Test-Path $path)) {
                throw "required security artifact is missing: $rel"
            }
        }
    }

    if (-not $SkipWorkspaceTests) {
        Step "Rust workspace security regression checks" {
            Invoke-Native "cargo" @("test", "--workspace") -Quiet:$CompactOutput
        }
    }

    if (-not $SkipDocsBuild) {
        Step "Security docs build" {
            Invoke-Native "npm.cmd" @("run", "docs:build") -Quiet:$CompactOutput
        }
    }

    if (-not $SkipUiChecks) {
        Step "Desktop UI smoke and i18n" {
            Push-Location (Join-Path $repoRoot "ui\desktop")
            try {
                Invoke-Native "npm.cmd" @("test") -Quiet:$CompactOutput
            } finally {
                Pop-Location
            }
        }
    }

    Write-Host ""
    Write-Host "Security gates preflight passed." -ForegroundColor Green
} finally {
    Pop-Location
}
