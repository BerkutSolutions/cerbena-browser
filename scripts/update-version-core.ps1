param(
    [string]$Version = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Utf8NoBomFile([string]$Path, [string]$Content) {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Read-JsonFile([string]$Path) {
    if (-not (Test-Path $Path)) {
        throw "missing JSON file: $Path"
    }
    return Get-Content $Path -Raw | ConvertFrom-Json
}

function Get-NormalizedRelativePath([string]$AbsolutePath, [string]$RootPath) {
    $resolved = [System.IO.Path]::GetFullPath($AbsolutePath)
    $resolvedRoot = [System.IO.Path]::GetFullPath($RootPath)
    $rootWithSeparator = $resolvedRoot.TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($rootWithSeparator, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "path '$AbsolutePath' is outside repository root '$RootPath'"
    }
    return $resolved.Substring($rootWithSeparator.Length).Replace('\', '/')
}

function Get-VersionLiteralPaths([string]$RootPath, [string]$Version) {
    $skipDirs = New-Object System.Collections.Generic.HashSet[string]([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($name in @(".docusaurus", ".git", ".tools", ".work", "target", "node_modules", "build")) {
        [void]$skipDirs.Add($name)
    }
    $allowedExts = New-Object System.Collections.Generic.HashSet[string]([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($ext in @("toml", "lock", "json", "js", "jsx", "rs", "md", "ps1")) {
        [void]$allowedExts.Add($ext)
    }
    $found = New-Object System.Collections.Generic.HashSet[string]([System.StringComparer]::Ordinal)
    $stack = New-Object System.Collections.Generic.Stack[string]
    $stack.Push([System.IO.Path]::GetFullPath($RootPath))
    while ($stack.Count -gt 0) {
        $dir = $stack.Pop()
        foreach ($childDir in [System.IO.Directory]::GetDirectories($dir)) {
            $name = [System.IO.Path]::GetFileName($childDir)
            if (-not $skipDirs.Contains($name)) {
                $stack.Push($childDir)
            }
        }
        foreach ($file in [System.IO.Directory]::GetFiles($dir)) {
            $ext = [System.IO.Path]::GetExtension($file).TrimStart('.')
            if (-not $allowedExts.Contains($ext)) {
                continue
            }
            $content = ""
            try {
                $content = [System.IO.File]::ReadAllText($file, [System.Text.Encoding]::UTF8)
            } catch {
                continue
            }
            if (-not $content.Contains($Version)) {
                continue
            }
            $relative = Get-NormalizedRelativePath $file $RootPath
            if ($relative -eq "ui/desktop/src-tauri/Cargo.lock") {
                continue
            }
            [void]$found.Add($relative)
        }
    }
    return @($found | Sort-Object)
}

function Sync-VersionManifestCoverage(
    [string]$ManifestPath,
    [string]$RootPath,
    [string]$CurrentVersion
) {
    $manifest = Read-JsonFile $ManifestPath
    if ($null -eq $manifest.targets) {
        throw "version sync manifest is missing targets array"
    }
    $existingPaths = New-Object System.Collections.Generic.HashSet[string]([System.StringComparer]::Ordinal)
    foreach ($entry in $manifest.targets) {
        $entryPath = [string]$entry.path
        if (-not [string]::IsNullOrWhiteSpace($entryPath)) {
            [void]$existingPaths.Add($entryPath.Replace('\', '/'))
        }
    }

    $literalPaths = Get-VersionLiteralPaths -RootPath $RootPath -Version $CurrentVersion
    $added = New-Object System.Collections.Generic.List[string]
    foreach ($literalPath in $literalPaths) {
        if ($existingPaths.Contains($literalPath)) {
            continue
        }
        $manifest.targets += [pscustomobject]@{
            path = $literalPath
            strategy = "replace_all"
        }
        [void]$existingPaths.Add($literalPath)
        $added.Add($literalPath) | Out-Null
    }

    if ($added.Count -gt 0) {
        $manifestJson = $manifest | ConvertTo-Json -Depth 12
        Write-Utf8NoBomFile $ManifestPath ($manifestJson + "`n")
    }
    return @($added)
}

function Normalize-Version([string]$Value) {
    $normalized = [string]$Value
    $normalized = $normalized.Trim()
    if ($normalized.StartsWith("v")) {
        $normalized = $normalized.Substring(1)
    }
    if ([string]::IsNullOrWhiteSpace($normalized)) {
        throw "version must not be empty"
    }
    if ($normalized -notmatch '^\d+\.\d+\.\d+$') {
        throw "version must use semantic format X.Y.Z"
    }
    return $normalized
}

function Apply-ReplaceAll([string]$Content, [string]$CurrentVersion, [string]$NextVersion, [string]$Path) {
    if ($CurrentVersion -eq $NextVersion) {
        return $Content
    }
    if (-not $Content.Contains($CurrentVersion)) {
        throw "version string '$CurrentVersion' was not found in $Path"
    }
    return $Content.Replace($CurrentVersion, $NextVersion)
}

function Apply-ChangelogHeading([string]$Content, [string]$CurrentVersion, [string]$NextVersion, [string]$Path) {
    if ($CurrentVersion -eq $NextVersion) {
        return $Content
    }
    $pattern = "(?m)^##\s+" + [regex]::Escape($CurrentVersion) + "(\s*)$"
    $replaced = [regex]::Replace($Content, $pattern, ("## " + $NextVersion + '$1'), 1)
    if ($replaced -eq $Content) {
        throw "release heading for $CurrentVersion was not found in $Path"
    }
    return $replaced
}

function Apply-RegexSingleReplace(
    [string]$Content,
    [string]$Pattern,
    [scriptblock]$ReplacementFactory,
    [string]$Path,
    [string]$Description
) {
    $match = [regex]::Match($Content, $Pattern)
    if (-not $match.Success) {
        throw "$Description was not found in $Path"
    }
    $replacement = & $ReplacementFactory $match
    return $Content.Substring(0, $match.Index) + $replacement + $Content.Substring($match.Index + $match.Length)
}

function Apply-CargoVersionField([string]$Content, [string]$CurrentVersion, [string]$NextVersion, [string]$Path) {
    if ($CurrentVersion -eq $NextVersion) {
        return $Content
    }
    $pattern = '(?m)^version = "' + [regex]::Escape($CurrentVersion) + '"\r?$'
    return Apply-RegexSingleReplace $Content $pattern {
        param($match)
        $match.Value.Replace($CurrentVersion, $NextVersion)
    } $Path "Cargo version field"
}

function Apply-JsonTopLevelVersion([string]$Content, [string]$CurrentVersion, [string]$NextVersion, [string]$Path) {
    if ($CurrentVersion -eq $NextVersion) {
        return $Content
    }
    $pattern = '(?m)^(\s*"version"\s*:\s*)"' + [regex]::Escape($CurrentVersion) + '"(,?)\r?$'
    return Apply-RegexSingleReplace $Content $pattern {
        param($match)
        $match.Groups[1].Value + '"' + $NextVersion + '"' + $match.Groups[2].Value
    } $Path "JSON top-level version field"
}

function Apply-PackageLockRootVersions([string]$Content, [string]$CurrentVersion, [string]$NextVersion, [string]$Path) {
    if ($CurrentVersion -eq $NextVersion) {
        return $Content
    }
    $updated = $Content
    $rootPattern = '(?m)^(\s*"version"\s*:\s*)"' + [regex]::Escape($CurrentVersion) + '"(,?)\r?$'
    $packagesPattern = '(?ms)(""\s*:\s*\{.*?^\s*"version"\s*:\s*)"' + [regex]::Escape($CurrentVersion) + '"'
    $updated = Apply-RegexSingleReplace $updated $rootPattern {
        param($match)
        $match.Groups[1].Value + '"' + $NextVersion + '"' + $match.Groups[2].Value
    } $Path "package-lock root version field"
    $updated = Apply-RegexSingleReplace $updated $packagesPattern {
        param($match)
        $match.Groups[1].Value + '"' + $NextVersion + '"'
    } $Path 'package-lock packages[""] version field'
    return $updated
}

function Apply-JsExportConst([string]$Content, [string]$CurrentVersion, [string]$NextVersion, [string]$Path) {
    if ($CurrentVersion -eq $NextVersion) {
        return $Content
    }
    $pattern = '(?m)^export const APP_VERSION = "' + [regex]::Escape($CurrentVersion) + '";\r?$'
    return Apply-RegexSingleReplace $Content $pattern {
        param($match)
        $match.Value.Replace($CurrentVersion, $NextVersion)
    } $Path "APP_VERSION export"
}

function Apply-CargoLockPackageVersions(
    [string]$Content,
    [string]$CurrentVersion,
    [string]$NextVersion,
    [string]$Path,
    [string[]]$PackageNames
) {
    if ($CurrentVersion -eq $NextVersion) {
        return $Content
    }
    if ($null -eq $PackageNames -or $PackageNames.Count -eq 0) {
        throw "cargo lock target $Path does not declare packageNames"
    }

    $normalizedNames = New-Object System.Collections.Generic.HashSet[string]([System.StringComparer]::Ordinal)
    foreach ($name in $PackageNames) {
        if (-not [string]::IsNullOrWhiteSpace($name)) {
            [void]$normalizedNames.Add($name)
        }
    }

    $lines = $Content -split "`n", 0, "SimpleMatch"
    $updatedAny = $false
    $currentPackage = $null
    for ($index = 0; $index -lt $lines.Length; $index++) {
        $line = $lines[$index]
        if ($line -match '^\[\[package\]\]\r?$') {
            $currentPackage = $null
            continue
        }
        if ($line -match '^name = "([^"]+)"\r?$') {
            $candidateName = $matches[1]
            if ($normalizedNames.Contains($candidateName)) {
                $currentPackage = $candidateName
            } else {
                $currentPackage = $null
            }
            continue
        }
        if ($null -ne $currentPackage -and $line -match '^version = "' + [regex]::Escape($CurrentVersion) + '"\r?$') {
            $lines[$index] = $line.Replace($CurrentVersion, $NextVersion)
            $updatedAny = $true
            $currentPackage = $null
        }
    }

    if (-not $updatedAny) {
        throw "no workspace package versions were updated in $Path"
    }
    return ($lines -join "`n")
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$manifestPath = Join-Path $repoRoot "scripts\version-sync-targets.json"
$tauriConfigPath = Join-Path $repoRoot "ui\desktop\src-tauri\tauri.conf.json"
$tauriConfig = Read-JsonFile $tauriConfigPath
$currentVersion = Normalize-Version ([string]$tauriConfig.version)

if ([string]::IsNullOrWhiteSpace($Version)) {
    Write-Host ""
    Write-Host ("Current version: " + $currentVersion) -ForegroundColor Cyan
    $Version = Read-Host "Enter a new version"
}

$nextVersion = Normalize-Version $Version
if ($nextVersion -eq $currentVersion) {
    Write-Host ("Version is already " + $nextVersion + ". Nothing to update.") -ForegroundColor Yellow
    exit 0
}

$autoAddedManifestPaths = @(Sync-VersionManifestCoverage -ManifestPath $manifestPath -RootPath $repoRoot -CurrentVersion $currentVersion)
$targetsManifest = Read-JsonFile $manifestPath

$updatedFiles = New-Object System.Collections.Generic.List[string]
$pendingWrites = New-Object System.Collections.Generic.List[object]
foreach ($target in $targetsManifest.targets) {
    $relativePath = [string]$target.path
    $strategy = [string]$target.strategy
    $packageNamesProperty = $target.PSObject.Properties["packageNames"]
    $packageNames = if ($null -ne $packageNamesProperty) { @($packageNamesProperty.Value) } else { @() }
    $path = Join-Path $repoRoot $relativePath
    if (-not (Test-Path $path)) {
        throw "version target is missing: $relativePath"
    }
    $original = [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)
    $updated = switch ($strategy) {
        "replace_all" { Apply-ReplaceAll $original $currentVersion $nextVersion $relativePath }
        "changelog_heading" { Apply-ChangelogHeading $original $currentVersion $nextVersion $relativePath }
        "cargo_version_field" { Apply-CargoVersionField $original $currentVersion $nextVersion $relativePath }
        "json_top_level_version" { Apply-JsonTopLevelVersion $original $currentVersion $nextVersion $relativePath }
        "package_lock_root_versions" { Apply-PackageLockRootVersions $original $currentVersion $nextVersion $relativePath }
        "js_export_const" { Apply-JsExportConst $original $currentVersion $nextVersion $relativePath }
        "cargo_lock_package_versions" { Apply-CargoLockPackageVersions $original $currentVersion $nextVersion $relativePath $packageNames }
        default { throw "unknown version sync strategy '$strategy' for $relativePath" }
    }
    if ($updated -ne $original) {
        $pendingWrites.Add([pscustomobject]@{
            Path = $path
            RelativePath = $relativePath
            Content = $updated
        }) | Out-Null
        $updatedFiles.Add($relativePath) | Out-Null
    }
}

foreach ($pending in $pendingWrites) {
    Write-Utf8NoBomFile $pending.Path $pending.Content
}

$publishedUpdaterScriptPath = Join-Path $repoRoot "scripts\published-updater-e2e.ps1"
if (Test-Path -LiteralPath $publishedUpdaterScriptPath) {
    $publishedUpdaterScript = [System.IO.File]::ReadAllText($publishedUpdaterScriptPath, [System.Text.Encoding]::UTF8)
    if ($publishedUpdaterScript -match '\[string\]\$MinimumPublishedVersion\s*=\s*"[0-9]+\.[0-9]+\.[0-9]+"') {
        throw "scripts/published-updater-e2e.ps1 contains a pinned MinimumPublishedVersion literal. Use dynamic minimum based on BaseVersion."
    }
}

Write-Host ""
Write-Host ("Updated version: " + $currentVersion + " -> " + $nextVersion) -ForegroundColor Green
if (@($autoAddedManifestPaths).Count -gt 0) {
    Write-Host "Auto-added version-sync targets:" -ForegroundColor Cyan
    foreach ($path in $autoAddedManifestPaths) {
        Write-Host (" - " + $path) -ForegroundColor DarkGray
    }
}
foreach ($item in $updatedFiles) {
    Write-Host (" - " + $item) -ForegroundColor DarkGray
}
