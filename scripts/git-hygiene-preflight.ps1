param(
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
            return $output
        }

        $output = & $FilePath @Arguments
        $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
        if ($exitCode -ne 0) {
            $argsText = ($Arguments -join " ")
            throw "command failed ($exitCode): $FilePath $argsText"
        }
        return $output
    } finally {
        $ErrorActionPreference = $prevErrorAction
        if ($hasNativePref) {
            $PSNativeCommandUseErrorActionPreference = $prevNativePref
        }
    }
}

function Invoke-GitStatusCheck([string]$RepoRoot, [string[]]$Arguments) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = "git"
    $psi.Arguments = ('-C "{0}" {1}' -f $RepoRoot, ($Arguments -join " "))
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

    return [pscustomobject]@{
        ExitCode = $process.ExitCode
        StdOut = $stdout
    }
}

function Assert-IgnoredAndUntracked([string]$RepoRoot, [string]$RelativePath) {
    $absolute = Join-Path $RepoRoot $RelativePath
    if (-not (Test-Path $absolute)) {
        return
    }

    $ignored = Invoke-GitStatusCheck -RepoRoot $RepoRoot -Arguments @("check-ignore", "--", $RelativePath)
    if ($ignored.ExitCode -ne 0) {
        throw "$RelativePath must be ignored by git before release/publication."
    }

    $tracked = Invoke-GitStatusCheck -RepoRoot $RepoRoot -Arguments @("ls-files", "--error-unmatch", "--", $RelativePath)
    if ($tracked.ExitCode -eq 0) {
        throw "$RelativePath is tracked by git but must remain local-only."
    }

    $staged = git -C $RepoRoot diff --cached --name-only -- $RelativePath
    if ($LASTEXITCODE -ne 0) {
        throw "failed to inspect staged files for $RelativePath"
    }
    if (-not [string]::IsNullOrWhiteSpace(($staged -join "").Trim())) {
        throw "$RelativePath has staged content and would leak into publication."
    }
}

function Assert-Tracked([string]$RepoRoot, [string]$RelativePath) {
    $absolute = Join-Path $RepoRoot $RelativePath
    if (-not (Test-Path $absolute)) {
        throw "$RelativePath must exist in the repository."
    }

    $tracked = Invoke-GitStatusCheck -RepoRoot $RepoRoot -Arguments @("ls-files", "--error-unmatch", "--", $RelativePath)
    if ($tracked.ExitCode -ne 0) {
        throw "$RelativePath must be tracked by git for CI/publication flows."
    }

    $ignored = Invoke-GitStatusCheck -RepoRoot $RepoRoot -Arguments @("check-ignore", "--", $RelativePath)
    if ($ignored.ExitCode -eq 0) {
        throw "$RelativePath is ignored by git but must be published for CI/publication flows."
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

Push-Location $repoRoot
try {
    Step "Git ignore contract" {
        $gitignore = Get-Content (Join-Path $repoRoot ".gitignore") -Raw
        foreach ($needle in @(
            ".work/",
            ".cache/",
            "build/",
            "target/",
            "node_modules/",
            "docs/build/",
            "ui/desktop/src-tauri/target/"
        )) {
            if (-not $gitignore.Contains($needle)) {
                throw ".gitignore must contain $needle"
            }
        }
    }

    Step "Local-only paths stay local" {
        foreach ($path in @(
            ".work",
            ".cache",
            "build",
            "target",
            "node_modules",
            "docs/build",
            "ui/desktop/node_modules",
            "ui/desktop/src-tauri/target"
        )) {
            Assert-IgnoredAndUntracked -RepoRoot $repoRoot -RelativePath $path
        }
    }

    Step "Release and CI scripts are published" {
        foreach ($path in @(
            "scripts/build-installer.ps1",
            "scripts/generate-release-artifacts.ps1",
            "scripts/git-hygiene-preflight.ps1",
            "scripts/local-ci-preflight.ps1",
            "scripts/security-gates-preflight.ps1",
            "scripts/vulnerability-gates-preflight.ps1"
        )) {
            Assert-Tracked -RepoRoot $repoRoot -RelativePath $path
        }
    }

    Write-Host ""
    Write-Host "Git hygiene preflight passed." -ForegroundColor Green
} finally {
    Pop-Location
}
