param(
    [ValidateSet("interactive", "version", "check", "package", "publish", "full")]
    [string]$Mode = "interactive",
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

. (Join-Path $PSScriptRoot "release-signing.ps1")

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
            return $output
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

function Get-CurrentReleaseVersion([string]$Root) {
    $tauriConfig = Read-JsonFile (Join-Path $Root "ui\desktop\src-tauri\tauri.conf.json")
    $rootPackage = Read-JsonFile (Join-Path $Root "package.json")
    $desktopPackage = Read-JsonFile (Join-Path $Root "ui\desktop\package.json")
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
    return $version
}

function Write-Utf8NoBomFile([string]$Path, [string]$Content) {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Test-ReleaseSigningEnvironmentPresent() {
    $hasPrivateKey = -not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable("CERBENA_RELEASE_SIGNING_PRIVATE_KEY_XML")) `
        -or -not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable("CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH"))
    $hasPfxPath = -not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable("CERBENA_AUTHENTICODE_PFX_PATH"))
    $hasPfxPassword = -not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable("CERBENA_AUTHENTICODE_PFX_PASSWORD"))
    return $hasPrivateKey -and $hasPfxPath -and $hasPfxPassword
}

function Sync-ReleaseSigningPublicKey([string]$Root, [string]$MaterialDirectory) {
    $sourcePath = Join-Path $MaterialDirectory "release-signing-public-key.xml"
    if (-not (Test-Path $sourcePath)) {
        throw "release signing bootstrap did not produce public key: $sourcePath"
    }
    $targetPath = Get-ReleaseSigningPublicKeyPath $Root
    $targetDirectory = Split-Path -Parent $targetPath
    if (-not (Test-Path $targetDirectory)) {
        New-Item -ItemType Directory -Path $targetDirectory -Force | Out-Null
    }
    Write-Utf8NoBomFile $targetPath (([System.IO.File]::ReadAllText($sourcePath, [System.Text.Encoding]::UTF8)).Trim() + "`n")
}

function Test-HasCommittedReleaseSigningPublicKey([string]$Root) {
    $path = Get-ReleaseSigningPublicKeyPath $Root
    if (-not (Test-Path $path)) {
        return $false
    }
    return -not [string]::IsNullOrWhiteSpace(([System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)).Trim())
}

function Ensure-ReleaseSigningBootstrap([string]$Root) {
    if (Test-ReleaseSigningEnvironmentPresent) {
        Initialize-ReleaseSigningEnvironment $Root
        return
    }

    $materialDirectory = Get-LatestLocalSigningMaterialDirectory $Root
    if ([string]::IsNullOrWhiteSpace($materialDirectory)) {
        if (Test-HasCommittedReleaseSigningPublicKey $Root) {
            throw @"
release signing material is missing, but a committed public verification key already exists.

Refusing to bootstrap a brand-new signing identity automatically because that would break updater trust for already published clients.

Provide the matching operator bundle via CERBENA_* variables or restore the original files under:
.work/release-signing
"@
        }
        Write-Title "Bootstrap Release Signing"
        Invoke-Native "powershell" @(
            "-ExecutionPolicy", "Bypass",
            "-File", (Join-Path $Root "scripts\new-release-signing-material.ps1"),
            "-OutputDir", (Join-Path $Root ".work\release-signing")
        )
        $materialDirectory = Get-LatestLocalSigningMaterialDirectory $Root
        if ([string]::IsNullOrWhiteSpace($materialDirectory)) {
            throw "release signing bootstrap completed but no local signing bundle was discovered under .work/release-signing"
        }
        Sync-ReleaseSigningPublicKey -Root $Root -MaterialDirectory $materialDirectory
    }

    Initialize-ReleaseSigningEnvironment $Root
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
        "scripts\update-version.ps1",
        "scripts\version-sync-targets.json",
        "scripts\docker-runtime-preflight.ps1",
        "scripts\security-gates-preflight.ps1",
        "scripts\vulnerability-gates-preflight.ps1",
        "scripts\generate-release-artifacts.ps1",
        "scripts\build-installer.ps1",
        "scripts\release-signing.ps1",
        "scripts\new-release-signing-material.ps1",
        "config\release\release-signing-public-key.xml"
    )
    foreach ($rel in $required) {
        $path = Join-Path $Root $rel
        if (-not (Test-Path $path)) {
            throw "required release contract file is missing: $rel"
        }
    }
}

function Assert-LocalPublicationBlockerContract([string]$Root) {
    $gitignorePath = Join-Path $Root ".gitignore"
    if (-not (Test-Path -LiteralPath $gitignorePath)) {
        throw ".gitignore is required for local publication blocker contract"
    }
    $gitignoreText = Get-Content -LiteralPath $gitignorePath -Raw
    $requiredBlockers = @(
        "# release-local-only: scripts/release.ps1",
        "# release-local-only: scripts/local-ci-preflight.ps1",
        "# release-local-only: scripts/local-updater-e2e.ps1",
        "# release-local-only: scripts/published-updater-e2e.ps1"
    )
    foreach ($marker in $requiredBlockers) {
        if (-not $gitignoreText.Contains($marker)) {
            throw "publish blocked: .gitignore is missing local publication blocker marker '$marker'"
        }
    }
}

function Assert-GitHubCliAvailable() {
    if (-not (Get-Command "gh" -ErrorAction SilentlyContinue)) {
        throw "gh is required for GitHub Release publication. Install GitHub CLI and authenticate with 'gh auth login'."
    }
}

function Resolve-InstallerArtifactPaths([string]$Root, [string]$Version) {
    $installerRoot = Join-Path $Root ("build\installer\" + $Version)
    $msiPath = Join-Path $installerRoot ("output\cerbena-browser-" + $Version + ".msi")
    $innoPath = Join-Path $installerRoot ("output\cerbena-browser-setup-" + $Version + ".exe")
    $fallbackPath = Join-Path $installerRoot ("cerbena-browser-setup-" + $Version + ".exe")
    $paths = @()
    foreach ($candidate in @($msiPath, $innoPath, $fallbackPath)) {
        if (Test-Path $candidate) {
            $paths += $candidate
        }
    }
    return $paths
}

function Get-RequiredInstallerAssetPaths([string]$Root, [string]$Version) {
    $installerRoot = Join-Path $Root ("build\installer\" + $Version)
    return @{
        Msi = Join-Path $installerRoot ("output\cerbena-browser-" + $Version + ".msi")
    }
}

function Assert-InstallerAssetContract([string]$Root, [string]$Version) {
    $required = Get-RequiredInstallerAssetPaths $Root $Version
    if (-not (Test-Path $required.Msi)) {
        throw "required MSI installer is missing: $($required.Msi)"
    }
}

function Resolve-ReleaseUploadAssetPaths([string]$Root, [string]$Version) {
    $releaseRoot = Join-Path $Root ("build\release\" + $Version)
    $paths = New-Object System.Collections.Generic.List[string]
    [void]$paths.Add((Join-Path $releaseRoot "checksums.txt"))
    [void]$paths.Add((Join-Path $releaseRoot "checksums.sig"))
    [void]$paths.Add((Join-Path $releaseRoot "release-manifest.json"))
    foreach ($installerPath in (Resolve-InstallerArtifactPaths -Root $Root -Version $Version)) {
        [void]$paths.Add($installerPath)
    }

    return @(
        $paths | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique
    )
}

function Ensure-ReleaseUploadAssets([string]$Root, [string]$Version) {
    $releaseRoot = Join-Path $Root ("build\release\" + $Version)
    $requiredReleaseFiles = @(
        (Join-Path $releaseRoot "checksums.txt"),
        (Join-Path $releaseRoot "checksums.sig"),
        (Join-Path $releaseRoot "release-manifest.json")
    )
    $missingReleaseFiles = @($requiredReleaseFiles | Where-Object { -not (Test-Path $_) })
    if ($missingReleaseFiles.Count -gt 0) {
        Generate-Artifacts $Root $Version
    }

    $requiredInstallers = Get-RequiredInstallerAssetPaths $Root $Version
    $hasMsi = Test-Path $requiredInstallers.Msi
    if (-not $hasMsi) {
        Build-Installer $Root
    }
    Assert-InstallerAssetContract $Root $Version

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
    Remove-UnexpectedGitHubReleaseAssets $Root $Version
    Assert-GitHubReleaseAssetsPublished $Root $Version
}

function Remove-UnexpectedGitHubReleaseAssets([string]$Root, [string]$Version) {
    $tag = "v$Version"
    $expectedAssetNames = Resolve-ReleaseUploadAssetPaths $Root $Version |
        ForEach-Object { [System.IO.Path]::GetFileName($_) }
    $publishedAssetNames = Invoke-Native "gh" @(
        "release", "view", $tag,
        "--repo", $defaultRepoSlug,
        "--json", "assets",
        "--jq", ".assets[].name"
    ) -Quiet
    $publishedSet = @($publishedAssetNames | ForEach-Object { $_.ToString().Trim() } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    $unexpected = @($publishedSet | Where-Object { $expectedAssetNames -notcontains $_ })
    foreach ($assetName in $unexpected) {
        Invoke-Native "gh" @(
            "release", "delete-asset", $tag, $assetName,
            "--repo", $defaultRepoSlug,
            "--yes"
        )
    }
}

function Assert-GitHubReleaseAssetsPublished([string]$Root, [string]$Version) {
    $tag = "v$Version"
    $expectedAssetNames = Resolve-ReleaseUploadAssetPaths $Root $Version |
        ForEach-Object { [System.IO.Path]::GetFileName($_) }
    $publishedAssetNames = Invoke-Native "gh" @(
        "release", "view", $tag,
        "--repo", $defaultRepoSlug,
        "--json", "assets",
        "--jq", ".assets[].name"
    ) -Quiet
    $publishedSet = @($publishedAssetNames | ForEach-Object { $_.ToString().Trim() } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    $missing = @($expectedAssetNames | Where-Object { $publishedSet -notcontains $_ })
    if ($missing.Count -gt 0) {
        throw "GitHub Release is missing required published assets: $($missing -join ', ')"
    }
}

function Run-Checks([string]$Root, [switch]$SkipPublishedUpdaterE2E) {
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
    if ($SkipPublishedUpdaterE2E) {
        $args += "-SkipPublishedUpdaterE2E"
    }
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
    Ensure-ReleaseSigningBootstrap $Root
    Invoke-Native "powershell" @(
        "-ExecutionPolicy", "Bypass",
        "-File", (Join-Path $Root "scripts\generate-release-artifacts.ps1"),
        "-Version", $Version
    )
}

function Build-Installer([string]$Root) {
    Write-Title "Build Installer"
    Ensure-ReleaseSigningBootstrap $Root
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

    Assert-LocalPublicationBlockerContract $Root

    $localOnlyScriptPaths = @(
        "scripts/release.ps1",
        "scripts/local-ci-preflight.ps1",
        "scripts/local-updater-e2e.ps1",
        "scripts/published-updater-e2e.ps1",
        ".work/local-scripts"
    )
    Invoke-Native "git" @("-C", $Root, "add", ".")

    $blockedStagedBeforeReset = @(& git -C $Root diff --cached --name-only -- $localOnlyScriptPaths)
    $blockedExitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
    if ($blockedExitCode -ne 0) {
        throw "failed to inspect staged local-only scripts before reset"
    }
    $blockedStagedBeforeReset = @($blockedStagedBeforeReset | ForEach-Object { $_.ToString().Trim() } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    if ($blockedStagedBeforeReset.Count -gt 0) {
        Write-Host ("Auto-unstaging local-only paths before publish: " + ($blockedStagedBeforeReset -join ", ")) -ForegroundColor Yellow
    }

    foreach ($localOnlyPath in $localOnlyScriptPaths) {
        $fullPath = Join-Path $Root $localOnlyPath
        if ((Test-Path -LiteralPath $fullPath) -or $localOnlyPath.StartsWith(".work")) {
            Invoke-Native "git" @("-C", $Root, "reset", "HEAD", "--", $localOnlyPath) -Quiet
        }
    }

    $blockedStagedAfterReset = @(& git -C $Root diff --cached --name-only -- $localOnlyScriptPaths)
    $blockedAfterExitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
    if ($blockedAfterExitCode -ne 0) {
        throw "failed to inspect staged local-only scripts after reset"
    }
    $blockedStagedAfterReset = @($blockedStagedAfterReset | ForEach-Object { $_.ToString().Trim() } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    if ($blockedStagedAfterReset.Count -gt 0) {
        throw "publish blocked: local-only scripts remain staged after auto-reset: $($blockedStagedAfterReset -join ', ')"
    }

    $staged = git -C $Root diff --cached --name-only
    $exitCode = if (Get-Variable -Name LASTEXITCODE -ErrorAction SilentlyContinue) { $LASTEXITCODE } else { 0 }
    if ($exitCode -ne 0) {
        throw "failed to inspect staged changes"
    }
    if (-not [string]::IsNullOrWhiteSpace(($staged -join ""))) {
        Invoke-Native "git" @("-C", $Root, "commit", "-m", "release: $tag")
    }

    Invoke-Native "git" @("-C", $Root, "pull", "--rebase", "--autostash", $defaultRemoteName, "main")
    Invoke-Native "git" @("-C", $Root, "push", $defaultRemoteName, "main")

    $existingTag = Get-GitTag $Root $tag
    if ([string]::IsNullOrWhiteSpace($existingTag)) {
        Invoke-Native "git" @("-C", $Root, "tag", "-a", $tag, "-m", "Release $Version")
    }
    Invoke-Native "git" @("-C", $Root, "push", $defaultRemoteName, $tag)
    Publish-GitHubReleaseAssets $Root $Version
}

function Run-LocalUpdaterE2E([string]$Root, [string]$Version) {
    Write-Title "Local updater end-to-end test"
    Invoke-Native "powershell" @(
        "-ExecutionPolicy", "Bypass",
        "-File", (Join-Path $Root "scripts\local-updater-e2e.ps1"),
        "-Version", $Version,
        "-TimeoutMinutes", "8",
        "-PublishedRetryCount", "1"
    )
}

function Invoke-ChangeVersion([string]$Root) {
    Write-Title "Change Version"
    $currentVersion = Get-CurrentReleaseVersion $Root
    Write-Host ("Current version: " + $currentVersion) -ForegroundColor White
    [void](Invoke-Native "powershell" @(
        "-ExecutionPolicy", "Bypass",
        "-File", (Join-Path $Root "scripts\update-version.ps1")
    ))
}

function Show-ReleaseMenu([string]$Root) {
    while ($true) {
        $currentVersion = Get-CurrentReleaseVersion $Root
        Write-Host ""
        Write-Host "Cerbena release menu" -ForegroundColor Cyan
        Write-Host ("Current version: " + $currentVersion) -ForegroundColor White
        Write-Host "1. Change version"
        Write-Host "2. Full cycle"
        Write-Host "3. Publish only"
        Write-Host "4. Checks only"
        Write-Host "Press Ctrl+C to exit." -ForegroundColor DarkGray
        $selection = (Read-Host "Select 1/2/3/4").Trim()
        switch ($selection) {
            "1" {
                [void](Invoke-ChangeVersion $Root)
                continue
            }
            "2" { return "full" }
            "3" { return "publish" }
            "4" { return "check" }
            default {
                Write-Host "Unknown selection. Enter 1, 2, 3, or 4." -ForegroundColor Yellow
            }
        }
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

Set-Location $repoRoot
Assert-ReleaseContracts $repoRoot

if ($Mode -eq "interactive") {
    $Mode = Show-ReleaseMenu $repoRoot
}

$version = Get-CurrentReleaseVersion $repoRoot

Write-Title "Release Configuration"
Write-Host ("Version: " + $version) -ForegroundColor White
Write-Host ("Mode: " + $Mode) -ForegroundColor White

switch ($Mode) {
    "version" {
        Invoke-ChangeVersion $repoRoot
    }
    "check" {
        Run-Checks $repoRoot -SkipPublishedUpdaterE2E
        Run-VulnerabilityGates $repoRoot
    }
    "package" {
        Run-Checks $repoRoot -SkipPublishedUpdaterE2E
        Run-VulnerabilityGates $repoRoot
        Generate-Artifacts $repoRoot $version
        Build-Installer $repoRoot
    }
    "publish" {
        Generate-Artifacts $repoRoot $version
        Build-Installer $repoRoot
        Run-LocalUpdaterE2E $repoRoot $version
        Publish-Release $repoRoot $version
    }
    "full" {
        Run-Checks $repoRoot -SkipPublishedUpdaterE2E
        Run-VulnerabilityGates $repoRoot
        Generate-Artifacts $repoRoot $version
        Build-Installer $repoRoot
        Run-LocalUpdaterE2E $repoRoot $version
        Publish-Release $repoRoot $version
    }
}

Write-Host ""
Write-Host "Release flow completed." -ForegroundColor Green
