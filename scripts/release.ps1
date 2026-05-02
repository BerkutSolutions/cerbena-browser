param(
    [ValidateSet("check", "package", "publish", "full")]
    [string]$Mode = "check",
    [switch]$SkipDockerPreflight,
    [switch]$SkipSecurityGates,
    [switch]$SkipVulnerabilityGates,
    [switch]$SkipLocalDockerVulnerabilityGates,
    [switch]$CompactOutput
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$defaultRemoteName = "origin"
$defaultRemoteUrl = "https://github.com/BerkutSolutions/cerbena-browser.git"
$defaultRepoSlug = "BerkutSolutions/cerbena-browser"

function Write-Title([string]$Text) {
    Write-Host ""
    Write-Host ("== " + $Text + " ==") -ForegroundColor Cyan
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

function Test-GitRepository([string]$Root) {
    $prevErrorAction = $ErrorActionPreference
    $hasNativePref = $null -ne (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)
    if ($hasNativePref) {
        $prevNativePref = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
    }
    $ErrorActionPreference = "Continue"
    try {
        & git -C $Root rev-parse --is-inside-work-tree *> $null
    } finally {
        $ErrorActionPreference = $prevErrorAction
        if ($hasNativePref) {
            $PSNativeCommandUseErrorActionPreference = $prevNativePref
        }
    }
    $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
    return $exitCode -eq 0
}

function Get-GitRemoteUrl([string]$Root, [string]$RemoteName) {
    $prevErrorAction = $ErrorActionPreference
    $hasNativePref = $null -ne (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)
    if ($hasNativePref) {
        $prevNativePref = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
    }
    $ErrorActionPreference = "Continue"
    try {
        $output = & git -C $Root remote get-url $RemoteName 2>$null
    } finally {
        $ErrorActionPreference = $prevErrorAction
        if ($hasNativePref) {
            $PSNativeCommandUseErrorActionPreference = $prevNativePref
        }
    }
    $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
    if ($exitCode -ne 0) {
        return ""
    }
    return (($output | Select-Object -First 1).ToString()).Trim()
}

function Get-GitHeadCommit([string]$Root) {
    $prevErrorAction = $ErrorActionPreference
    $hasNativePref = $null -ne (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)
    if ($hasNativePref) {
        $prevNativePref = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
    }
    $ErrorActionPreference = "Continue"
    try {
        $output = & git -C $Root rev-parse --verify HEAD 2>$null
    } finally {
        $ErrorActionPreference = $prevErrorAction
        if ($hasNativePref) {
            $PSNativeCommandUseErrorActionPreference = $prevNativePref
        }
    }
    $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
    if ($exitCode -ne 0) {
        return ""
    }
    return (($output | Select-Object -First 1).ToString()).Trim()
}

function Get-GitTag([string]$Root, [string]$TagName) {
    $prevErrorAction = $ErrorActionPreference
    $hasNativePref = $null -ne (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)
    if ($hasNativePref) {
        $prevNativePref = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
    }
    $ErrorActionPreference = "Continue"
    try {
        $output = & git -C $Root tag --list $TagName 2>$null
    } finally {
        $ErrorActionPreference = $prevErrorAction
        if ($hasNativePref) {
            $PSNativeCommandUseErrorActionPreference = $prevNativePref
        }
    }
    $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
    if ($exitCode -ne 0) {
        return ""
    }
    $first = $output | Select-Object -First 1
    if ($null -eq $first) {
        return ""
    }
    return $first.ToString().Trim()
}

function Ensure-PublishRepositoryBootstrap([string]$Root, [string]$Version) {
    $tag = "v$Version"
    # Bootstrap flow for missing repos/remotes mirrors:
    # git init
    # git branch -M main
    # git remote add origin https://github.com/BerkutSolutions/cerbena-browser.git
    # git push -u origin main
    $isGitRepo = Test-GitRepository $Root
    if (-not $isGitRepo) {
        Write-Title "Bootstrap Git Repository"
        Invoke-Native "git" @("-C", $Root, "init")
    }

    $currentBranch = (& git -C $Root branch --show-current 2>$null | Select-Object -First 1).ToString().Trim()
    if ([string]::IsNullOrWhiteSpace($currentBranch) -or $currentBranch -ne "main") {
        Invoke-Native "git" @("-C", $Root, "branch", "-M", "main")
    }

    $origin = Get-GitRemoteUrl $Root $defaultRemoteName
    if ([string]::IsNullOrWhiteSpace($origin)) {
        Invoke-Native "git" @("-C", $Root, "remote", "add", $defaultRemoteName, $defaultRemoteUrl)
    }

    $headCommit = Get-GitHeadCommit $Root
    if ([string]::IsNullOrWhiteSpace($headCommit)) {
        Invoke-Native "git" @("-C", $Root, "add", ".")
        $staged = & git -C $Root diff --cached --name-only
        $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
        if ($exitCode -ne 0) {
            throw "failed to inspect staged bootstrap changes"
        }
        if ([string]::IsNullOrWhiteSpace(($staged -join ""))) {
            throw "unable to create bootstrap commit because no files are staged"
        }
        Invoke-Native "git" @("-C", $Root, "commit", "-m", $tag)
        Invoke-Native "git" @("-C", $Root, "push", "-u", $defaultRemoteName, "main")
    }
}

function Assert-ReleaseContracts([string]$Root) {
    $required = @(
        "README.md",
        "README.en.md",
        "CHANGELOG.md",
        "scripts\local-ci-preflight.ps1",
        "scripts\docker-runtime-preflight.ps1",
        "scripts\security-gates-preflight.ps1",
        "scripts\vulnerability-gates-preflight.ps1",
        "scripts\generate-release-artifacts.ps1",
        "scripts\build-installer.ps1"
    )
    foreach ($rel in $required) {
        $path = Join-Path $Root $rel
        if (-not (Test-Path $path)) {
            throw "required release contract file is missing: $rel"
        }
    }
}

function Assert-GitHubCliAvailable() {
    if (-not (Get-Command "gh" -ErrorAction SilentlyContinue)) {
        throw "gh is required for GitHub Release publication. Install GitHub CLI and authenticate with 'gh auth login'."
    }
}

function Resolve-InstallerArtifactPath([string]$Root, [string]$Version) {
    $installerRoot = Join-Path $Root ("build\installer\" + $Version)
    $innoPath = Join-Path $installerRoot ("output\cerbena-browser-setup-" + $Version + ".exe")
    if (Test-Path $innoPath) {
        return $innoPath
    }
    $fallbackPath = Join-Path $installerRoot ("cerbena-browser-setup-" + $Version + ".exe")
    if (Test-Path $fallbackPath) {
        return $fallbackPath
    }
    return ""
}

function Resolve-ReleaseUploadAssetPaths([string]$Root, [string]$Version) {
    $releaseRoot = Join-Path $Root ("build\release\" + $Version)
    $bundleRoot = Join-Path $releaseRoot "staging\cerbena-windows-x64"
    $installerPath = Resolve-InstallerArtifactPath $Root $Version

    $paths = @(
        (Join-Path $releaseRoot "cerbena-windows-x64.zip"),
        (Join-Path $releaseRoot "checksums.txt"),
        (Join-Path $releaseRoot "checksums.sig"),
        (Join-Path $releaseRoot "release-manifest.json"),
        (Join-Path $bundleRoot "cerbena-updater.exe")
    )
    if (-not [string]::IsNullOrWhiteSpace($installerPath)) {
        $paths += $installerPath
    }
    return $paths
}

function Ensure-ReleaseUploadAssets([string]$Root, [string]$Version) {
    $releaseRoot = Join-Path $Root ("build\release\" + $Version)
    $requiredReleaseFiles = @(
        (Join-Path $releaseRoot "cerbena-windows-x64.zip"),
        (Join-Path $releaseRoot "checksums.txt"),
        (Join-Path $releaseRoot "checksums.sig"),
        (Join-Path $releaseRoot "release-manifest.json"),
        (Join-Path $releaseRoot "staging\cerbena-windows-x64\cerbena-updater.exe")
    )
    $missingReleaseFiles = @($requiredReleaseFiles | Where-Object { -not (Test-Path $_) })
    if ($missingReleaseFiles.Count -gt 0) {
        Generate-Artifacts $Root $Version
    }

    $installerPath = Resolve-InstallerArtifactPath $Root $Version
    if ([string]::IsNullOrWhiteSpace($installerPath)) {
        Build-Installer $Root
    }

    $resolvedAssets = Resolve-ReleaseUploadAssetPaths $Root $Version
    foreach ($path in $resolvedAssets) {
        if (-not (Test-Path $path)) {
            throw "required GitHub Release asset is missing: $path"
        }
    }
}

function Get-ChangelogReleaseNotes([string]$Root, [string]$Version) {
    $changelogPath = Join-Path $Root "CHANGELOG.md"
    if (-not (Test-Path $changelogPath)) {
        throw "missing changelog: $changelogPath"
    }

    $lines = [System.IO.File]::ReadAllLines($changelogPath)
    $headingPattern = "^##\s+" + [regex]::Escape($Version) + "(?:\s|$)"
    $start = -1
    for ($index = 0; $index -lt $lines.Length; $index++) {
        if ($lines[$index] -match $headingPattern) {
            $start = $index
            break
        }
    }
    if ($start -lt 0) {
        throw "CHANGELOG.md is missing a release section for $Version"
    }

    $end = $lines.Length
    for ($index = $start + 1; $index -lt $lines.Length; $index++) {
        if ($lines[$index] -match "^##\s+") {
            $end = $index
            break
        }
    }

    return (($lines[$start..($end - 1)]) -join "`n").Trim()
}

function New-GitHubReleaseNotesFile([string]$Root, [string]$Version) {
    $notesRoot = Join-Path $Root ("build\release\" + $Version)
    New-Item -ItemType Directory -Path $notesRoot -Force | Out-Null
    $notesPath = Join-Path $notesRoot "github-release-notes.md"
    $notes = Get-ChangelogReleaseNotes $Root $Version
    Write-Utf8NoBomFile $notesPath ($notes + "`n")
    return $notesPath
}

function Test-GitHubReleaseExists([string]$Tag) {
    $prevErrorAction = $ErrorActionPreference
    $hasNativePref = $null -ne (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)
    if ($hasNativePref) {
        $prevNativePref = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
    }
    $ErrorActionPreference = "Continue"
    try {
        & gh release view $Tag --repo $defaultRepoSlug *> $null
    } finally {
        $ErrorActionPreference = $prevErrorAction
        if ($hasNativePref) {
            $PSNativeCommandUseErrorActionPreference = $prevNativePref
        }
    }
    $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
    return $exitCode -eq 0
}

function Publish-GitHubReleaseAssets([string]$Root, [string]$Version) {
    Assert-GitHubCliAvailable
    Ensure-ReleaseUploadAssets $Root $Version

    # The publish path intentionally drives `gh release create`, `gh release edit`, and
    # `gh release upload --clobber` so the GitHub Release page and trust assets stay in sync.
    $tag = "v$Version"
    $notesPath = New-GitHubReleaseNotesFile $Root $Version
    if (-not (Test-GitHubReleaseExists $tag)) {
        Invoke-Native "gh" @(
            "release", "create", $tag,
            "--repo", $defaultRepoSlug,
            "--title", ("Cerbena Browser " + $Version),
            "--notes-file", $notesPath
        )
    } else {
        Invoke-Native "gh" @(
            "release", "edit", $tag,
            "--repo", $defaultRepoSlug,
            "--title", ("Cerbena Browser " + $Version),
            "--notes-file", $notesPath
        )
    }

    # Publish trust assets with `gh release upload --clobber` so secure updater metadata is always attached.
    $uploadArgs = @("release", "upload", $tag, "--repo", $defaultRepoSlug, "--clobber")
    $uploadArgs += Resolve-ReleaseUploadAssetPaths $Root $Version
    Invoke-Native "gh" $uploadArgs
}

function Run-Checks([string]$Root) {
    Write-Title "Preflight"
    $args = @(
        "-ExecutionPolicy", "Bypass",
        "-File", (Join-Path $Root "scripts\local-ci-preflight.ps1")
    )
    if ($SkipDockerPreflight) {
        $args += "-SkipDockerPreflight"
    }
    if ($SkipSecurityGates) {
        $args += "-SkipSecurityGates"
    }
    $args += "-SkipVulnerabilityGates"
    if ($CompactOutput) {
        $args += "-CompactOutput"
    }
    Invoke-Native "powershell" $args
}

function Run-VulnerabilityGates([string]$Root) {
    if ($SkipVulnerabilityGates) {
        return
    }

    Write-Title "Vulnerability Gates"
    $vulnArgs = @(
        "-ExecutionPolicy", "Bypass",
        "-File", (Join-Path $Root "scripts\vulnerability-gates-preflight.ps1")
    )
    if ($CompactOutput) {
        $vulnArgs += "-CompactOutput"
    }
    if ($SkipLocalDockerVulnerabilityGates) {
        $vulnArgs += "-DisableLocalDockerSandbox"
    }
    Invoke-Native "powershell" $vulnArgs
}

function Generate-Artifacts([string]$Root, [string]$Version) {
    Write-Title "Release Artifacts"
    Invoke-Native "powershell" @(
        "-ExecutionPolicy", "Bypass",
        "-File", (Join-Path $Root "scripts\generate-release-artifacts.ps1"),
        "-Version", $Version
    )
}

function Build-Installer([string]$Root) {
    Write-Title "Build Installer"
    Invoke-Native "powershell" @(
        "-ExecutionPolicy", "Bypass",
        "-File", (Join-Path $Root "scripts\build-installer.ps1")
    )
}

function Publish-Release([string]$Root, [string]$Version) {
    $tag = "v$Version"
    Write-Title "Publish"
    Ensure-PublishRepositoryBootstrap $Root $Version
    $origin = Get-GitRemoteUrl $Root $defaultRemoteName
    if ([string]::IsNullOrWhiteSpace($origin)) {
        throw "git remote origin is not configured"
    }

    Invoke-Native "git" @("-C", $Root, "add", ".")
    $staged = git -C $Root diff --cached --name-only
    $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
    if ($exitCode -ne 0) {
        throw "failed to inspect staged changes"
    }
    if (-not [string]::IsNullOrWhiteSpace(($staged -join ""))) {
        Invoke-Native "git" @("-C", $Root, "commit", "-m", "release: $tag")
    }

    Invoke-Native "git" @("-C", $Root, "pull", "--rebase", $defaultRemoteName, "main")
    Invoke-Native "git" @("-C", $Root, "push", $defaultRemoteName, "main")

    $existingTag = Get-GitTag $Root $tag
    if ([string]::IsNullOrWhiteSpace($existingTag)) {
        Invoke-Native "git" @("-C", $Root, "tag", "-a", $tag, "-m", "Release $Version")
    }
    Invoke-Native "git" @("-C", $Root, "push", $defaultRemoteName, $tag)
    Publish-GitHubReleaseAssets $Root $Version
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$tauriConfig = Read-JsonFile (Join-Path $repoRoot "ui\desktop\src-tauri\tauri.conf.json")
$rootPackage = Read-JsonFile (Join-Path $repoRoot "package.json")
$desktopPackage = Read-JsonFile (Join-Path $repoRoot "ui\desktop\package.json")
$version = [string]$tauriConfig.version

if ([string]::IsNullOrWhiteSpace($version)) {
    throw "unable to resolve version from ui/desktop/src-tauri/tauri.conf.json"
}
if ([string]$rootPackage.version -ne $version) {
    throw "root package.json version mismatch: $($rootPackage.version) != $version"
}
if ([string]$desktopPackage.version -ne $version) {
    throw "ui/desktop package.json version mismatch: $($desktopPackage.version) != $version"
}

Set-Location $repoRoot
Assert-ReleaseContracts $repoRoot

Write-Title "Release Configuration"
Write-Host ("Version: " + $version) -ForegroundColor White
Write-Host ("Mode: " + $Mode) -ForegroundColor White

switch ($Mode) {
    "check" {
        Run-Checks $repoRoot
        Run-VulnerabilityGates $repoRoot
    }
    "package" {
        Run-Checks $repoRoot
        Run-VulnerabilityGates $repoRoot
        Generate-Artifacts $repoRoot $version
        Build-Installer $repoRoot
    }
    "publish" {
        Run-VulnerabilityGates $repoRoot
        Publish-Release $repoRoot $version
    }
    "full" {
        Run-Checks $repoRoot
        Run-VulnerabilityGates $repoRoot
        Generate-Artifacts $repoRoot $version
        Build-Installer $repoRoot
        Publish-Release $repoRoot $version
    }
}

Write-Host ""
Write-Host "Release flow completed." -ForegroundColor Green
