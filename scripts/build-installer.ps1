param(
    [string]$Version = "",
    [switch]$SkipReleasePackaging,
    [switch]$GenerateOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "release-signing.ps1")

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

function Read-JsonFile([string]$Path) {
    if (-not (Test-Path $Path)) {
        throw "missing JSON file: $Path"
    }
    return Get-Content $Path -Raw | ConvertFrom-Json
}

function Find-InnoSetupCompiler {
    $paths = @(
        (Get-Command "ISCC.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue),
        "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
        "C:\Program Files\Inno Setup 6\ISCC.exe"
    ) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }

    foreach ($path in $paths) {
        if (Test-Path $path) {
            return $path
        }
    }

    return $null
}

function Find-CSharpCompiler {
    $paths = @(
        (Get-Command "csc.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue),
        "C:\Windows\Microsoft.NET\Framework64\v4.0.30319\csc.exe",
        "C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe"
    ) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }

    foreach ($path in $paths) {
        if (Test-Path $path) {
            return $path
        }
    }

    return $null
}

function Find-WixTool([string]$RepoRoot) {
    $paths = @(
        (Join-Path $RepoRoot ".tools\wix.exe"),
        (Get-Command "wix.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue)
    ) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }

    foreach ($path in $paths) {
        if (Test-Path $path) {
            return $path
        }
    }

    return $null
}

function Convert-ToInnoPath([string]$Path) {
    return $Path.Replace("\", "\\")
}

function Get-RelativePathCompat([string]$BasePath, [string]$TargetPath) {
    $pathType = [System.IO.Path]
    $method = $pathType.GetMethod("GetRelativePath", [type[]]@([string], [string]))
    if ($null -ne $method) {
        return [string]$method.Invoke($null, @($BasePath, $TargetPath))
    }

    $normalizedBase = [System.IO.Path]::GetFullPath($BasePath)
    if (-not $normalizedBase.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
        $normalizedBase += [System.IO.Path]::DirectorySeparatorChar
    }
    $normalizedTarget = [System.IO.Path]::GetFullPath($TargetPath)
    $baseUri = New-Object System.Uri($normalizedBase)
    $targetUri = New-Object System.Uri($normalizedTarget)
    $relativeUri = $baseUri.MakeRelativeUri($targetUri)
    return [System.Uri]::UnescapeDataString($relativeUri.ToString()).Replace('/', [System.IO.Path]::DirectorySeparatorChar)
}

function Convert-ToWixId([string]$Prefix, [string]$Value) {
    $sanitized = ($Value -replace '[^A-Za-z0-9_]', '_').Trim('_')
    if ([string]::IsNullOrWhiteSpace($sanitized)) {
        $sanitized = "item"
    }
    if ($sanitized[0] -match '[0-9]') {
        $sanitized = "_" + $sanitized
    }
    $candidate = "${Prefix}_${sanitized}"
    if ($candidate.Length -le 60) {
        return $candidate
    }
    $hash = [BitConverter]::ToString(
        [System.Security.Cryptography.SHA256]::Create().ComputeHash(
            [System.Text.Encoding]::UTF8.GetBytes($Value)
        )
    ).Replace("-", "").Substring(0, 16)
    return "${Prefix}_${hash}"
}

function New-StableGuid([string]$Seed) {
    $md5 = [System.Security.Cryptography.MD5]::Create()
    try {
        $bytes = $md5.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($Seed))
        return ([guid]::New($bytes)).ToString().ToUpperInvariant()
    } finally {
        $md5.Dispose()
    }
}

function Convert-VersionToMsiProductVersion([string]$Version) {
    $normalized = $Version.Trim().TrimStart("v", "V")
    $parts = $normalized.Split("-", 2)
    $core = $parts[0].Split(".")
    if ($core.Count -lt 3) {
        throw "version must have at least 3 numeric components for MSI: $Version"
    }
    $major = [int]$core[0]
    $minor = [int]$core[1]
    $patch = [int]$core[2]
    if ($parts.Count -gt 1 -and $parts[1] -match '^[0-9]+(\.[0-9]+)*$') {
        $hotfix = [int](($parts[1].Split(".")[0]))
        $patch = [Math]::Min(65535, ($patch * 100) + $hotfix)
    }
    return "$major.$minor.$patch"
}

function New-ReleaseMetadataEntry([string]$Name, [string]$Target, [string]$Source, [string]$Kind, [string]$InstallerKind, [string]$UpdaterStrategy, [bool]$Primary) {
    $hash = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash.ToLowerInvariant()
    $size = (Get-Item -LiteralPath $Source).Length
    return @{
        name = $Name
        path = $Target
        sha256 = $hash
        size_bytes = $size
        platform = "windows-x64"
        kind = $Kind
        installer_kind = $InstallerKind
        updater_strategy = $UpdaterStrategy
        primary = $Primary
    }
}

function Update-ReleaseMetadataWithInstallerAssets([string]$RepoRoot, [string]$Version, [object[]]$InstallerArtifacts) {
    if ($InstallerArtifacts.Count -eq 0) {
        return
    }

    $releaseRoot = Join-Path $RepoRoot ("build\release\" + $Version)
    $manifestPath = Join-Path $releaseRoot "release-manifest.json"
    $checksumsPath = Join-Path $releaseRoot "checksums.txt"
    $checksumsSignaturePath = Join-Path $releaseRoot "checksums.sig"
    if (-not (Test-Path $manifestPath) -or -not (Test-Path $checksumsPath)) {
        throw "release metadata files are missing; generate release artifacts before building installers"
    }

    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $existingArtifacts = @($manifest.artifacts)
    $installerNames = $InstallerArtifacts | ForEach-Object { $_.name }
    $manifest.artifacts = @(
        $existingArtifacts | Where-Object { $installerNames -notcontains $_.name }
    ) + $InstallerArtifacts

    $existingLines = Get-Content -LiteralPath $checksumsPath |
        Where-Object {
            $line = $_.Trim()
            if ([string]::IsNullOrWhiteSpace($line)) {
                return $false
            }
            $entry = ($line -split '\s+', 3)[-1]
            $name = [System.IO.Path]::GetFileName(($entry -replace '/', '\'))
            return $installerNames -notcontains $name
        }
    $installerLines = $InstallerArtifacts | ForEach-Object { "$($_.sha256)  $($_.path)" }
    $checksumsText = (@($existingLines) + @($installerLines)) -join "`n"
    $checksumsBytes = [System.Text.Encoding]::UTF8.GetBytes($checksumsText)
    $checksumsSignature = New-ReleaseChecksumSignature $checksumsBytes

    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 8), $utf8)
    [System.IO.File]::WriteAllText($checksumsPath, $checksumsText, $utf8)
    [System.IO.File]::WriteAllText($checksumsSignaturePath, $checksumsSignature, $utf8)
}

function New-MsiInstaller(
    [string]$InstallerRoot,
    [string]$PayloadRoot,
    [string]$Version,
    [string]$RepoRoot
) {
    $wix = Find-WixTool $RepoRoot
    if ([string]::IsNullOrWhiteSpace($wix)) {
        throw "WiX toolset is not installed. Install wix.exe (for example via dotnet tool) before building MSI artifacts."
    }

    $msiVersion = Convert-VersionToMsiProductVersion $Version
    $wxsPath = Join-Path $InstallerRoot "CerbenaBrowserInstaller.wxs"
    $msiPath = Join-Path $InstallerRoot "output\cerbena-browser-$Version.msi"
    $markerPath = Join-Path $InstallerRoot "cerbena-install-mode.msi.txt"
    $markerComponentId = "CmpInstallModeMarker"
    $markerFileId = "FileInstallModeMarker"
    $upgradeCode = "30A26884-D6A5-4E10-B42A-5F0B7B14A7D8"

    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($markerPath, "msi`n", $utf8)

    $files = Get-ChildItem -LiteralPath $PayloadRoot -Recurse -File | Sort-Object FullName
    if ($files.Count -eq 0) {
        throw "MSI payload root is empty: $PayloadRoot"
    }

    $directories = @{}
    foreach ($file in $files) {
        $relative = Get-RelativePathCompat -BasePath $PayloadRoot -TargetPath $file.FullName
        $segments = $relative.Split([System.IO.Path]::DirectorySeparatorChar)
        $current = ""
        for ($index = 0; $index -lt ($segments.Count - 1); $index++) {
            $segment = $segments[$index]
            $parent = if ([string]::IsNullOrWhiteSpace($current)) { "" } else { $current }
            $current = if ([string]::IsNullOrWhiteSpace($current)) { $segment } else { (Join-Path $current $segment) }
            if (-not $directories.ContainsKey($current)) {
                $directories[$current] = @{
                    Key = $current
                    Parent = $parent
                    Name = $segment
                    Id = Convert-ToWixId "DIR" $current
                }
            }
        }
    }

    $childrenByParent = @{}
    foreach ($entry in $directories.GetEnumerator()) {
        $parent = $entry.Value.Parent
        if (-not $childrenByParent.ContainsKey($parent)) {
            $childrenByParent[$parent] = New-Object System.Collections.Generic.List[object]
        }
        [void]$childrenByParent[$parent].Add($entry.Value)
    }

    $componentsByDirectory = @{}
    $mainExecutableFileId = ""
    foreach ($file in $files) {
        $relative = Get-RelativePathCompat -BasePath $PayloadRoot -TargetPath $file.FullName
        $relativeNormalized = $relative.Replace("\", "/")
        $directoryRelative = [System.IO.Path]::GetDirectoryName($relative)
        $directoryKey = if ([string]::IsNullOrWhiteSpace($directoryRelative)) { "" } else { $directoryRelative }
        $directoryId = if ([string]::IsNullOrWhiteSpace($directoryKey)) { "INSTALLDIR" } else { $directories[$directoryKey].Id }
        if (-not $componentsByDirectory.ContainsKey($directoryId)) {
            $componentsByDirectory[$directoryId] = New-Object System.Collections.Generic.List[string]
        }
        $componentId = Convert-ToWixId "CMP" $relativeNormalized
        $fileId = Convert-ToWixId "FILE" $relativeNormalized
        if ($relativeNormalized -eq "cerbena.exe") {
            $mainExecutableFileId = $fileId
        }
        $componentGuid = New-StableGuid "msi-file::$relativeNormalized"
        $fileSource = Convert-ToInnoPath $file.FullName
        $xml = @"
        <Component Id="$componentId" Guid="$componentGuid">
          <File Id="$fileId" Source="$fileSource" KeyPath="yes" />
        </Component>
"@
        [void]$componentsByDirectory[$directoryId].Add($xml)
    }

    if ([string]::IsNullOrWhiteSpace($mainExecutableFileId)) {
        throw "MSI payload is missing cerbena.exe"
    }

    if (-not $componentsByDirectory.ContainsKey("INSTALLDIR")) {
        $componentsByDirectory["INSTALLDIR"] = New-Object System.Collections.Generic.List[string]
    }
    [void]$componentsByDirectory["INSTALLDIR"].Add(@"
        <Component Id="$markerComponentId" Guid="$(New-StableGuid 'msi-install-mode-marker')">
          <File Id="$markerFileId" Source="$(Convert-ToInnoPath $markerPath)" KeyPath="yes" Name="cerbena-install-mode.txt" />
        </Component>
"@)

    $iconRegistryComponentGuid = New-StableGuid "msi-browser-registration"
    [void]$componentsByDirectory["INSTALLDIR"].Add(@"
        <Component Id="CmpBrowserRegistration" Guid="$iconRegistryComponentGuid">
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser" Type="string" Value="Cerbena Browser" KeyPath="yes" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser" Name="LocalizedString" Type="string" Value="Cerbena Browser" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\DefaultIcon" Type="string" Value="[INSTALLDIR]cerbena.ico" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\shell\open\command" Type="string" Value="&quot;[INSTALLDIR]cerbena.exe&quot; &quot;%1&quot;" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities" Name="ApplicationName" Type="string" Value="Cerbena Browser" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities" Name="ApplicationDescription" Type="string" Value="Isolated browser profiles with controlled link routing and network policies." />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\UrlAssociations" Name="http" Type="string" Value="CerbenaBrowser.URL" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\UrlAssociations" Name="https" Type="string" Value="CerbenaBrowser.URL" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\FileAssociations" Name=".htm" Type="string" Value="CerbenaBrowser.HTML" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\FileAssociations" Name=".html" Type="string" Value="CerbenaBrowser.HTML" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\FileAssociations" Name=".shtml" Type="string" Value="CerbenaBrowser.HTML" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\FileAssociations" Name=".mht" Type="string" Value="CerbenaBrowser.MHTML" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\FileAssociations" Name=".mhtml" Type="string" Value="CerbenaBrowser.MHTML" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\FileAssociations" Name=".pdf" Type="string" Value="CerbenaBrowser.PDF" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\FileAssociations" Name=".svg" Type="string" Value="CerbenaBrowser.SVG" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\FileAssociations" Name=".xhy" Type="string" Value="CerbenaBrowser.XHTML" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\FileAssociations" Name=".xht" Type="string" Value="CerbenaBrowser.XHTML" />
          <RegistryValue Root="HKCU" Key="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities\FileAssociations" Name=".xhtml" Type="string" Value="CerbenaBrowser.XHTML" />
          <RegistryValue Root="HKCU" Key="Software\RegisteredApplications" Name="Cerbena Browser" Type="string" Value="Software\Clients\StartMenuInternet\Cerbena Browser\Capabilities" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.URL" Type="string" Value="Cerbena Browser" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.URL" Name="URL Protocol" Type="string" Value="" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.URL\DefaultIcon" Type="string" Value="[INSTALLDIR]cerbena.ico" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.URL\shell\open\command" Type="string" Value="&quot;[INSTALLDIR]cerbena.exe&quot; &quot;%1&quot;" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.HTML" Type="string" Value="Cerbena HTML Document" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.HTML\DefaultIcon" Type="string" Value="[INSTALLDIR]cerbena.ico" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.HTML\shell\open\command" Type="string" Value="&quot;[INSTALLDIR]cerbena.exe&quot; &quot;%1&quot;" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.MHTML" Type="string" Value="Cerbena MHTML Document" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.MHTML\DefaultIcon" Type="string" Value="[INSTALLDIR]cerbena.ico" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.MHTML\shell\open\command" Type="string" Value="&quot;[INSTALLDIR]cerbena.exe&quot; &quot;%1&quot;" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.PDF" Type="string" Value="Cerbena PDF Document" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.PDF\DefaultIcon" Type="string" Value="[INSTALLDIR]cerbena.ico" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.PDF\shell\open\command" Type="string" Value="&quot;[INSTALLDIR]cerbena.exe&quot; &quot;%1&quot;" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.SVG" Type="string" Value="Cerbena SVG Document" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.SVG\DefaultIcon" Type="string" Value="[INSTALLDIR]cerbena.ico" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.SVG\shell\open\command" Type="string" Value="&quot;[INSTALLDIR]cerbena.exe&quot; &quot;%1&quot;" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.XHTML" Type="string" Value="Cerbena XHTML Document" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.XHTML\DefaultIcon" Type="string" Value="[INSTALLDIR]cerbena.ico" />
          <RegistryValue Root="HKCU" Key="Software\Classes\CerbenaBrowser.XHTML\shell\open\command" Type="string" Value="&quot;[INSTALLDIR]cerbena.exe&quot; &quot;%1&quot;" />
        </Component>
"@)

    $shortcutComponentGuid = New-StableGuid "msi-shortcuts"
    $shortcutComponent = @"
      <DirectoryRef Id="ProgramMenuFolder">
        <Component Id="CmpStartMenuShortcut" Guid="$shortcutComponentGuid">
          <Shortcut Id="ShortcutStartMenuCerbena" Name="Cerbena Browser" Target="[INSTALLDIR]cerbena.exe" WorkingDirectory="INSTALLDIR" Icon="CerbenaProductIcon" />
          <RegistryValue Root="HKCU" Key="Software\BerkutSolutions\Cerbena Browser\MSI" Name="StartMenuShortcut" Type="integer" Value="1" KeyPath="yes" />
          <RemoveFolder Id="RemoveCerbenaStartMenuFolder" On="uninstall" />
        </Component>
      </DirectoryRef>
      <DirectoryRef Id="DesktopFolder">
        <Component Id="CmpDesktopShortcut" Guid="$(New-StableGuid 'msi-shortcut-desktop')">
          <Shortcut Id="ShortcutDesktopCerbena" Name="Cerbena Browser" Target="[INSTALLDIR]cerbena.exe" WorkingDirectory="INSTALLDIR" Icon="CerbenaProductIcon" />
          <RegistryValue Root="HKCU" Key="Software\BerkutSolutions\Cerbena Browser\MSI" Name="DesktopShortcut" Type="integer" Value="1" KeyPath="yes" />
        </Component>
      </DirectoryRef>
"@

    function Render-DirectoryXml([string]$ParentKey, [int]$IndentLevel) {
        $indent = "  " * $IndentLevel
        $lines = New-Object System.Collections.Generic.List[string]
        if ($componentsByDirectory.ContainsKey($(if ([string]::IsNullOrWhiteSpace($ParentKey)) { "INSTALLDIR" } else { $directories[$ParentKey].Id }))) {
            foreach ($component in $componentsByDirectory[$(if ([string]::IsNullOrWhiteSpace($ParentKey)) { "INSTALLDIR" } else { $directories[$ParentKey].Id })]) {
                [void]$lines.Add(($component -split "`n" | ForEach-Object { if ($_ -ne "") { $indent + $_ } }) -join "`n")
            }
        }
        foreach ($child in @($childrenByParent[$ParentKey] | Sort-Object Name)) {
            [void]$lines.Add("$indent<Directory Id=""$($child.Id)"" Name=""$($child.Name)"">")
            $rendered = Render-DirectoryXml $child.Key ($IndentLevel + 1)
            if (-not [string]::IsNullOrWhiteSpace($rendered)) {
                [void]$lines.Add($rendered)
            }
            [void]$lines.Add("$indent</Directory>")
        }
        return ($lines -join "`n")
    }

    $installDirContent = Render-DirectoryXml "" 4
    $wxs = @"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package
    Name="Cerbena Browser"
    Manufacturer="Berkut Solutions"
    UpgradeCode="$upgradeCode"
    Version="$msiVersion"
    Scope="perUser"
    InstallerVersion="500"
    Compressed="yes">
    <MajorUpgrade DowngradeErrorMessage="A newer version of Cerbena Browser is already installed." />
    <MediaTemplate EmbedCab="yes" CompressionLevel="high" />
    <Icon Id="CerbenaProductIcon" SourceFile="$(Convert-ToInnoPath (Join-Path $PayloadRoot 'cerbena.ico'))" />
    <Property Id="ARPPRODUCTICON" Value="CerbenaProductIcon" />
    <Property Id="ARPNOMODIFY" Value="1" />
    <Property Id="ARPNOREPAIR" Value="0" />

    <StandardDirectory Id="LocalAppDataFolder">
      <Directory Id="INSTALLDIR" Name="Cerbena Browser">
$installDirContent
      </Directory>
    </StandardDirectory>

$shortcutComponent

    <CustomAction Id="RunPostInstallRegistration" FileRef="$mainExecutableFileId" ExeCommand="--reconcile-install-registration" Execute="deferred" Impersonate="yes" Return="ignore" />
    <CustomAction Id="RunMsiCleanup" FileRef="$mainExecutableFileId" ExeCommand="--msi-cleanup" Execute="deferred" Impersonate="yes" Return="ignore" />

    <InstallExecuteSequence>
      <Custom Action="RunPostInstallRegistration" After="InstallFiles" Condition="NOT REMOVE=&quot;ALL&quot;" />
      <Custom Action="RunMsiCleanup" Before="RemoveFiles" Condition="REMOVE=&quot;ALL&quot;" />
    </InstallExecuteSequence>
  </Package>
</Wix>
"@

    [System.IO.File]::WriteAllText($wxsPath, $wxs, $utf8)
    [void](Invoke-Native $wix @(
        "build",
        $wxsPath,
        "-out",
        $msiPath,
        "-intermediatefolder",
        (Join-Path $InstallerRoot "wix-intermediate"),
        "-arch",
        "x64"
    ))
    if (-not (Test-Path $msiPath)) {
        throw "WiX did not produce MSI output: $msiPath"
    }
    return $msiPath
}

function New-ZipArchive([string]$SourceRoot, [string]$ZipPath) {
    if (Test-Path $ZipPath) {
        Remove-Item -LiteralPath $ZipPath -Force
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::CreateFromDirectory($SourceRoot, $ZipPath)
}

function New-CSharpFallbackInstaller([string]$InstallerRoot, [string]$PayloadRoot, [string]$Version) {
    $csharpCompiler = Find-CSharpCompiler
    if ([string]::IsNullOrWhiteSpace($csharpCompiler)) {
        throw "Inno Setup compiler is not installed and csc.exe is unavailable for fallback installer build."
    }

    $packageRoot = Join-Path $InstallerRoot "csharp-fallback"
    New-Item -ItemType Directory -Path $packageRoot -Force | Out-Null
    $payloadArchivePath = Join-Path $packageRoot "cerbena-browser-payload.zip"
    New-ZipArchive -SourceRoot $PayloadRoot -ZipPath $payloadArchivePath
    $brandLogoPath = Join-Path $repoRoot "ui\desktop\web\assets\brand\logo-256.png"
    $setupIconPath = Join-Path $repoRoot "static\img\favicon.ico"
    if (-not (Test-Path $brandLogoPath)) {
        throw "brand logo not found: $brandLogoPath"
    }
    if (-not (Test-Path $setupIconPath)) {
        throw "installer icon not found: $setupIconPath"
    }

    $sourcePath = Join-Path $packageRoot "CerbenaInstaller.cs"
    $targetExe = Join-Path $InstallerRoot ("cerbena-browser-setup-" + $Version + ".exe")
    $installerSource = @"
using System;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Drawing;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32;
using System.Reflection;
using System.Threading.Tasks;
using System.Windows.Forms;

internal static class CerbenaInstallerProgram
{
    private const string ProductName = "Cerbena Browser";
    private const string ShortcutFileName = "Cerbena Browser.lnk";
    private const string Publisher = "Berkut Solutions";
    private const string DisplayVersion = "$Version";
    private const string UninstallerFileName = "Cerbena Browser Uninstall.exe";
    private const string BrowserDescription = "Isolated browser profiles with controlled link routing and network policies.";
    private static readonly string ShortcutIconFileName = "cerbena.ico";
    private static readonly string InstallerLogPath = Path.Combine(Path.GetTempPath(), "cerbena-installer.log");
    private const string BrowserClientSubKey = @"Software\Clients\StartMenuInternet\Cerbena Browser";
    private const string RegisteredApplicationsSubKey = @"Software\RegisteredApplications";
    private const string BrowserUrlProgId = "CerbenaBrowser.URL";
    private const string BrowserHtmlProgId = "CerbenaBrowser.HTML";
    private const string BrowserMhtmlProgId = "CerbenaBrowser.MHTML";
    private const string BrowserPdfProgId = "CerbenaBrowser.PDF";
    private const string BrowserXhtmlProgId = "CerbenaBrowser.XHTML";
    private const string BrowserSvgProgId = "CerbenaBrowser.SVG";
    private static readonly string DefaultInstallRoot = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        @"Cerbena Browser");
    private const string LegacyAmneziaServicePrefix = "AmneziaWGTunnel`$awg-";
    private static readonly Guid FolderIdDesktop = new Guid("B4BFCC3A-DB2C-424C-B029-7FE99A87C641");
    private static readonly Guid FolderIdPrograms = new Guid("A77F5D77-2E2B-44C3-A6A2-ABA601054A51");

    [STAThread]
    private static void Main(string[] args)
    {
        try
        {
            var executableName = Path.GetFileName(Application.ExecutablePath);
            var launchedAsUninstaller = string.Equals(executableName, UninstallerFileName, StringComparison.OrdinalIgnoreCase);
            if (launchedAsUninstaller || args.Any(arg => string.Equals(arg, "--uninstall", StringComparison.OrdinalIgnoreCase)))
            {
                RunUninstaller(args.Any(arg => string.Equals(arg, "--silent", StringComparison.OrdinalIgnoreCase)));
                return;
            }

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            using (var wizard = new InstallerWizardForm())
            {
                Application.Run(wizard);
            }
        }
        catch (Exception ex)
        {
            MessageBox.Show(ex.ToString(), ProductName + " Installer", MessageBoxButtons.OK, MessageBoxIcon.Error);
            Environment.Exit(1);
        }
    }

    private static void RunUninstaller(bool silent)
    {
        var installRoot = AppDomain.CurrentDomain.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar);
        var running = FindRunningProductProcesses(installRoot);
        if (running.Count > 0)
        {
            if (silent)
            {
                if (!TryTerminateProcesses(running))
                {
                    throw new InvalidOperationException("Cerbena Browser is still running and could not be closed automatically.");
                }
            }
            else
            {
                var response = MessageBox.Show(
                    "Cerbena Browser is currently running. Click Yes to close it and continue uninstalling, or No to cancel and close it manually.",
                    ProductName,
                    MessageBoxButtons.YesNo,
                    MessageBoxIcon.Warning);
                if (response != DialogResult.Yes)
                {
                    return;
                }
                if (!TryTerminateProcesses(running))
                {
                    MessageBox.Show(
                        "Cerbena Browser is still running. Close the application and try uninstalling again.",
                        ProductName,
                        MessageBoxButtons.OK,
                        MessageBoxIcon.Error);
                    return;
                }
            }
        }

        if (!silent)
        {
            var confirmation = MessageBox.Show(
                "Remove Cerbena Browser and all installed files?",
                ProductName,
                MessageBoxButtons.YesNo,
                MessageBoxIcon.Question);
            if (confirmation != DialogResult.Yes)
            {
                return;
            }
        }

        RemoveShortcut(Path.Combine(GetKnownFolderPath(FolderIdPrograms), ShortcutFileName));
        RemoveShortcut(Path.Combine(GetKnownFolderPath(FolderIdDesktop), ShortcutFileName));
        RemoveBrowserRegistration();
        RemoveUninstallRegistration();
        CleanupManagedNetworkArtifacts(installRoot);
        CleanupManagedContainerArtifacts();
        CleanupLegacyAmneziaServices(installRoot);

        var commandPath = Path.Combine(Path.GetTempPath(), "cerbena-uninstall-" + Guid.NewGuid().ToString("N") + ".cmd");
        File.WriteAllText(
            commandPath,
            "@echo off\r\n" +
            "ping 127.0.0.1 -n 3 > nul\r\n" +
            "rmdir /s /q \"" + installRoot + "\"\r\n" +
            "del /f /q \"" + commandPath + "\"\r\n");
        Process.Start(new ProcessStartInfo
        {
            FileName = "cmd.exe",
            Arguments = "/c \"" + commandPath + "\"",
            CreateNoWindow = true,
            UseShellExecute = false,
            WindowStyle = ProcessWindowStyle.Hidden
        });
    }

    private static void CleanupLegacyAmneziaServices(string installRoot)
    {
        foreach (var serviceName in DiscoverLegacyAmneziaServices(installRoot))
        {
            TryRunSc("stop", serviceName);
            TryRunSc("delete", serviceName);
        }
    }

    private static void CleanupManagedNetworkArtifacts(string installRoot)
    {
        TryDeleteFile(Path.Combine(installRoot, ".app-secret.dpapi"));
        TryDeleteFile(Path.Combine(installRoot, "identity_store.json"));
        TryDeleteFile(Path.Combine(installRoot, "network_store.json"));
        TryDeleteFile(Path.Combine(installRoot, "network_sandbox_store.json"));
        TryDeleteFile(Path.Combine(installRoot, "extension_library.json"));
        TryDeleteFile(Path.Combine(installRoot, "sync_store.json"));
        TryDeleteFile(Path.Combine(installRoot, "link_routing_store.json"));
        TryDeleteFile(Path.Combine(installRoot, "launch_session_store.json"));
        TryDeleteFile(Path.Combine(installRoot, "device_posture_store.json"));
        TryDeleteFile(Path.Combine(installRoot, "app_update_store.json"));
        TryDeleteFile(Path.Combine(installRoot, "global_security_store.json"));
        TryDeleteFile(Path.Combine(installRoot, "traffic_gateway_log.json"));
        TryDeleteFile(Path.Combine(installRoot, "traffic_gateway_rules.json"));
        TryDeleteDirectory(Path.Combine(installRoot, "profiles"));
        TryDeleteDirectory(Path.Combine(installRoot, "engine-runtime"));
        TryDeleteDirectory(Path.Combine(installRoot, "network-runtime"));
        TryDeleteDirectory(Path.Combine(installRoot, "extension-packages"));
        TryDeleteDirectory(Path.Combine(installRoot, "updates"));
        TryDeleteDirectory(Path.Combine(installRoot, "native-messaging"));
    }

    private static void CleanupManagedContainerArtifacts()
    {
        TryRunDocker("ps -a --filter label=cerbena.kind=network-sandbox-runtime --format \"{{.Names}}\"");
        TryRunDocker(
            "ps -a --filter label=cerbena.kind=network-sandbox-runtime --format \"{{.Names}}\"",
            names =>
            {
                foreach (var name in names)
                {
                    RunProcessCapture("docker.exe", "rm -f \"" + name + "\"", 15000);
                }
            });
        TryRunDocker(
            "network ls --format \"{{.Name}}\"",
            names =>
            {
                foreach (var name in names.Where(value => value.StartsWith("cerbena-profile-", StringComparison.OrdinalIgnoreCase)))
                {
                    RunProcessCapture("docker.exe", "network rm \"" + name + "\"", 15000);
                }
            });
        try
        {
            RunProcessCapture("docker.exe", "image rm -f cerbena/network-sandbox:2026-05-02-r5", 20000);
        }
        catch
        {
        }
    }

    private static string[] DiscoverLegacyAmneziaServices(string installRoot)
    {
        var names = new System.Collections.Generic.HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var profilesRoot = Path.Combine(installRoot, "profiles");
        if (Directory.Exists(profilesRoot))
        {
            foreach (var confPath in Directory.EnumerateFiles(profilesRoot, "awg-*.conf", SearchOption.AllDirectories))
            {
                var tunnelName = Path.GetFileNameWithoutExtension(confPath);
                if (!string.IsNullOrWhiteSpace(tunnelName))
                {
                    names.Add(LegacyAmneziaServicePrefix + tunnelName);
                }
            }
        }

        try
        {
            var output = RunProcessCapture("sc.exe", "query state= all", 5000);
            using (var reader = new StringReader(output))
            {
                string line;
                while ((line = reader.ReadLine()) != null)
                {
                    var trimmed = line.Trim();
                    if (!trimmed.StartsWith("SERVICE_NAME:", StringComparison.OrdinalIgnoreCase))
                    {
                        continue;
                    }
                    var serviceName = trimmed.Substring("SERVICE_NAME:".Length).Trim();
                    if (serviceName.StartsWith(LegacyAmneziaServicePrefix, StringComparison.OrdinalIgnoreCase))
                    {
                        names.Add(serviceName);
                    }
                }
            }
        }
        catch
        {
        }

        return names.ToArray();
    }

    private static void TryRunSc(string action, string serviceName)
    {
        try
        {
            RunProcessCapture("sc.exe", action + " \"" + serviceName + "\"", 10000);
        }
        catch
        {
        }
    }

    private static void TryRunDocker(string arguments, Action<string[]> onSuccess)
    {
        try
        {
            var output = RunProcessCapture("docker.exe", arguments, 15000);
            var items = output
                .Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries)
                .Select(line => line.Trim())
                .Where(line => !string.IsNullOrWhiteSpace(line))
                .ToArray();
            onSuccess(items);
        }
        catch
        {
        }
    }

    private static void TryRunDocker(string arguments)
    {
        try
        {
            RunProcessCapture("docker.exe", arguments, 15000);
        }
        catch
        {
        }
    }

    private static void TryDeleteFile(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }
        }
        catch
        {
        }
    }

    private static void TryDeleteDirectory(string path)
    {
        try
        {
            if (Directory.Exists(path))
            {
                Directory.Delete(path, true);
            }
        }
        catch
        {
        }
    }

    private static string RunProcessCapture(string fileName, string arguments, int timeoutMs)
    {
        using (var process = new Process())
        {
            process.StartInfo = new ProcessStartInfo
            {
                FileName = fileName,
                Arguments = arguments,
                CreateNoWindow = true,
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                WindowStyle = ProcessWindowStyle.Hidden
            };
            process.Start();
            if (!process.WaitForExit(timeoutMs))
            {
                try
                {
                    process.Kill();
                }
                catch
                {
                }
                throw new TimeoutException(fileName + " " + arguments);
            }
            var stdout = process.StandardOutput.ReadToEnd();
            var stderr = process.StandardError.ReadToEnd();
            return stdout + stderr;
        }
    }

    private sealed class InstallerWizardForm : Form
    {
        private readonly Panel welcomePanel;
        private readonly Panel directoryPanel;
        private readonly Panel progressPanel;
        private readonly Panel finishPanel;
        private readonly Button backButton;
        private readonly Button nextButton;
        private readonly Button cancelButton;
        private readonly TextBox installPathTextBox;
        private readonly ProgressBar progressBar;
        private readonly Label progressLabel;
        private readonly CheckBox launchCheckBox;
        private readonly CheckBox desktopShortcutCheckBox;
        private int pageIndex;

        internal InstallerWizardForm()
        {
            Text = ProductName + " Setup";
            ClientSize = new Size(720, 460);
            StartPosition = FormStartPosition.CenterScreen;
            FormBorderStyle = FormBorderStyle.FixedDialog;
            MaximizeBox = false;
            MinimizeBox = false;
            Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath);

            welcomePanel = CreatePagePanel();
            directoryPanel = CreatePagePanel();
            progressPanel = CreatePagePanel();
            finishPanel = CreatePagePanel();

            Controls.Add(welcomePanel);
            Controls.Add(directoryPanel);
            Controls.Add(progressPanel);
            Controls.Add(finishPanel);

            backButton = new Button { Text = "< Back", Width = 96, Height = 30, Left = 410, Top = 400 };
            nextButton = new Button { Text = "Next >", Width = 96, Height = 30, Left = 512, Top = 400 };
            cancelButton = new Button { Text = "Cancel", Width = 96, Height = 30, Left = 614, Top = 400 };

            backButton.Click += (_, __) => NavigateBack();
            nextButton.Click += async (_, __) => await NavigateNextAsync();
            cancelButton.Click += (_, __) => Close();

            Controls.Add(backButton);
            Controls.Add(nextButton);
            Controls.Add(cancelButton);

            var logo = LoadBrandLogo();
            BuildWelcomePage(logo);
            installPathTextBox = BuildDirectoryPage(logo);
            var progressState = BuildProgressPage(logo);
            progressBar = progressState.Item1;
            progressLabel = progressState.Item2;
            var finishState = BuildFinishPage(logo);
            launchCheckBox = finishState.Item1;
            desktopShortcutCheckBox = finishState.Item2;
            installPathTextBox.Text = DefaultInstallRoot;

            ShowPage(0);
        }

        private static Panel CreatePagePanel()
        {
            return new Panel
            {
                Left = 0,
                Top = 0,
                Width = 720,
                Height = 390
            };
        }

        private void BuildWelcomePage(Image logo)
        {
            welcomePanel.Controls.Add(CreateHeaderLogo(logo));
            welcomePanel.Controls.Add(new Label
            {
                Text = "Welcome to the Cerbena Browser Setup Wizard",
                Left = 240,
                Top = 56,
                Width = 420,
                Font = new Font("Segoe UI", 16f, FontStyle.Bold)
            });
            welcomePanel.Controls.Add(new Label
            {
                Text = "This wizard will guide you through the installation of Cerbena Browser.",
                Left = 240,
                Top = 104,
                Width = 420,
                Height = 48,
                Font = new Font("Segoe UI", 10f, FontStyle.Regular)
            });
            welcomePanel.Controls.Add(new Label
            {
                Text = "Click Next to continue.",
                Left = 240,
                Top = 166,
                Width = 300,
                Font = new Font("Segoe UI", 10f, FontStyle.Regular)
            });
        }

        private TextBox BuildDirectoryPage(Image logo)
        {
            directoryPanel.Controls.Add(CreateHeaderLogo(logo));
            directoryPanel.Controls.Add(new Label
            {
                Text = "Choose Install Location",
                Left = 240,
                Top = 56,
                Width = 420,
                Font = new Font("Segoe UI", 16f, FontStyle.Bold)
            });
            directoryPanel.Controls.Add(new Label
            {
                Text = "Cerbena Browser will be installed into the following folder.",
                Left = 240,
                Top = 104,
                Width = 420,
                Height = 36,
                Font = new Font("Segoe UI", 10f, FontStyle.Regular)
            });

            var textBox = new TextBox
            {
                Left = 240,
                Top = 162,
                Width = 330,
                Height = 28
            };
            var browseButton = new Button
            {
                Text = "Browse...",
                Left = 580,
                Top = 160,
                Width = 90,
                Height = 30
            };
            browseButton.Click += (_, __) =>
            {
                using (var dialog = new FolderBrowserDialog())
                {
                    dialog.Description = "Select the Cerbena Browser install folder";
                    dialog.SelectedPath = string.IsNullOrWhiteSpace(textBox.Text) ? DefaultInstallRoot : textBox.Text;
                    if (dialog.ShowDialog(this) == DialogResult.OK)
                    {
                        textBox.Text = dialog.SelectedPath;
                    }
                }
            };
            directoryPanel.Controls.Add(textBox);
            directoryPanel.Controls.Add(browseButton);
            return textBox;
        }

        private Tuple<ProgressBar, Label> BuildProgressPage(Image logo)
        {
            progressPanel.Controls.Add(CreateHeaderLogo(logo));
            progressPanel.Controls.Add(new Label
            {
                Text = "Installing Cerbena Browser",
                Left = 240,
                Top = 56,
                Width = 420,
                Font = new Font("Segoe UI", 16f, FontStyle.Bold)
            });
            var label = new Label
            {
                Text = "Preparing installation...",
                Left = 240,
                Top = 126,
                Width = 420,
                Height = 28,
                Font = new Font("Segoe UI", 10f, FontStyle.Regular)
            };
            var bar = new ProgressBar
            {
                Left = 240,
                Top = 166,
                Width = 430,
                Height = 24,
                Minimum = 0,
                Maximum = 100
            };
            progressPanel.Controls.Add(label);
            progressPanel.Controls.Add(bar);
            return Tuple.Create(bar, label);
        }

        private Tuple<CheckBox, CheckBox> BuildFinishPage(Image logo)
        {
            finishPanel.Controls.Add(CreateHeaderLogo(logo));
            finishPanel.Controls.Add(new Label
            {
                Text = "Completing the Cerbena Browser Setup Wizard",
                Left = 240,
                Top = 56,
                Width = 430,
                Font = new Font("Segoe UI", 16f, FontStyle.Bold)
            });
            finishPanel.Controls.Add(new Label
            {
                Text = "Cerbena Browser has been installed successfully.",
                Left = 240,
                Top = 112,
                Width = 420,
                Height = 32,
                Font = new Font("Segoe UI", 10f, FontStyle.Regular)
            });
            var desktopCheckBox = new CheckBox
            {
                Text = "Create a desktop shortcut",
                Left = 240,
                Top = 164,
                Width = 240,
                Checked = true
            };
            var launchBox = new CheckBox
            {
                Text = "Launch Cerbena Browser",
                Left = 240,
                Top = 194,
                Width = 240,
                Checked = true
            };
            finishPanel.Controls.Add(desktopCheckBox);
            finishPanel.Controls.Add(launchBox);
            return Tuple.Create(launchBox, desktopCheckBox);
        }

        private PictureBox CreateHeaderLogo(Image logo)
        {
            return new PictureBox
            {
                Left = 32,
                Top = 32,
                Width = 176,
                Height = 176,
                SizeMode = PictureBoxSizeMode.Zoom,
                Image = logo
            };
        }

        private static Image LoadBrandLogo()
        {
            using (var resource = Assembly.GetExecutingAssembly().GetManifestResourceStream("BrandLogo"))
            {
                if (resource == null)
                {
                    throw new InvalidOperationException("Embedded brand logo was not found.");
                }
                return Image.FromStream(resource);
            }
        }

        private void ShowPage(int index)
        {
            pageIndex = index;
            welcomePanel.Visible = index == 0;
            directoryPanel.Visible = index == 1;
            progressPanel.Visible = index == 2;
            finishPanel.Visible = index == 3;

            backButton.Enabled = index == 1;
            cancelButton.Enabled = index != 2;

            if (index == 0)
            {
                nextButton.Text = "Next >";
            }
            else if (index == 1)
            {
                nextButton.Text = "Install";
            }
            else if (index == 3)
            {
                nextButton.Text = "Finish";
            }
        }

        private void NavigateBack()
        {
            if (pageIndex == 1)
            {
                ShowPage(0);
            }
        }

        private async Task NavigateNextAsync()
        {
            if (pageIndex == 0)
            {
                ShowPage(1);
                return;
            }

            if (pageIndex == 1)
            {
                var targetRoot = installPathTextBox.Text.Trim();
                if (string.IsNullOrWhiteSpace(targetRoot))
                {
                    MessageBox.Show(this, "Choose an install folder first.", ProductName, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                    return;
                }

                ShowPage(2);
                backButton.Enabled = false;
                nextButton.Enabled = false;
                await InstallAsync(targetRoot);
                ShowPage(3);
                nextButton.Enabled = true;
                return;
            }

            if (pageIndex == 3)
            {
                var desktopShortcutPath = Path.Combine(GetKnownFolderPath(FolderIdDesktop), ShortcutFileName);
                if (desktopShortcutCheckBox.Checked)
                {
                    var executable = Path.Combine(installPathTextBox.Text.Trim(), "cerbena.exe");
                    var shortcutIconPath = Path.Combine(installPathTextBox.Text.Trim(), ShortcutIconFileName);
                    if (File.Exists(executable))
                    {
                        CreateShortcut(
                            desktopShortcutPath,
                            executable,
                            Path.GetDirectoryName(executable),
                            shortcutIconPath);
                    }
                }
                else
                {
                    RemoveShortcut(desktopShortcutPath);
                }
                if (launchCheckBox.Checked)
                {
                    var executable = Path.Combine(installPathTextBox.Text.Trim(), "cerbena.exe");
                    if (File.Exists(executable))
                    {
                        Process.Start(new ProcessStartInfo
                        {
                            FileName = executable,
                            WorkingDirectory = Path.GetDirectoryName(executable),
                            UseShellExecute = true
                        });
                    }
                }
                Close();
            }
        }

        private async Task InstallAsync(string targetRoot)
        {
            await Task.Run(() =>
            {
                ReportProgress(10, "Preparing installation folder...");
                var running = FindRunningProductProcesses(targetRoot);
                if (running.Count > 0 && !TryTerminateProcesses(running))
                {
                    throw new InvalidOperationException("Cerbena Browser is still running and could not be closed automatically before installation.");
                }
                var tempRoot = Path.Combine(Path.GetTempPath(), "CerbenaInstaller_" + Guid.NewGuid().ToString("N"));
                Directory.CreateDirectory(tempRoot);
                Directory.CreateDirectory(targetRoot);

                try
                {
                    ReportProgress(25, "Extracting package...");
                    var archivePath = Path.Combine(tempRoot, "cerbena-browser-payload.zip");
                    using (var resource = Assembly.GetExecutingAssembly().GetManifestResourceStream("PayloadArchive"))
                    {
                        if (resource == null)
                        {
                            throw new InvalidOperationException("Embedded payload archive was not found.");
                        }
                        using (var file = File.Create(archivePath))
                        {
                            resource.CopyTo(file);
                        }
                    }

                    var extractRoot = Path.Combine(tempRoot, "payload");
                    ZipFile.ExtractToDirectory(archivePath, extractRoot);
                    var payloadContentRoot = ResolvePayloadContentRoot(extractRoot);

                    ReportProgress(55, "Copying browser files...");
                    CopyDirectory(payloadContentRoot, targetRoot);

                    var targetExe = EnsureInstalledBrowserExecutable(payloadContentRoot, targetRoot);

                    ReportProgress(68, "Installing icons...");
                    var shortcutIconPath = Path.Combine(targetRoot, ShortcutIconFileName);
                    using (var resource = Assembly.GetExecutingAssembly().GetManifestResourceStream("ShortcutIcon"))
                    {
                        if (resource == null)
                        {
                            throw new InvalidOperationException("Embedded shortcut icon was not found.");
                        }
                        using (var file = File.Create(shortcutIconPath))
                        {
                            resource.CopyTo(file);
                        }
                    }

                    ReportProgress(76, "Writing shortcuts...");
                    CreateShortcut(Path.Combine(GetKnownFolderPath(FolderIdPrograms), ShortcutFileName), targetExe, targetRoot, shortcutIconPath);
                    CreateShortcut(Path.Combine(GetKnownFolderPath(FolderIdDesktop), ShortcutFileName), targetExe, targetRoot, shortcutIconPath);

                    ReportProgress(86, "Registering uninstaller...");
                    var uninstallerPath = Path.Combine(targetRoot, UninstallerFileName);
                    File.Copy(Application.ExecutablePath, uninstallerPath, true);
                    RegisterUninstaller(targetRoot, shortcutIconPath, uninstallerPath);
                    RegisterBrowserCapabilities(targetExe, shortcutIconPath);

                    ReportProgress(100, "Installation completed.");
                }
                finally
                {
                    if (Directory.Exists(tempRoot))
                    {
                        try
                        {
                            Directory.Delete(tempRoot, true);
                        }
                        catch
                        {
                        }
                    }
                }
            });
        }

        private void ReportProgress(int percent, string message)
        {
            if (InvokeRequired)
            {
                Invoke(new Action<int, string>(ReportProgress), percent, message);
                return;
            }
            progressBar.Value = Math.Max(progressBar.Minimum, Math.Min(progressBar.Maximum, percent));
            progressLabel.Text = message;
        }
    }

    private static void CopyDirectory(string sourceRoot, string destinationRoot)
    {
        foreach (var directory in Directory.GetDirectories(sourceRoot, "*", SearchOption.AllDirectories))
        {
            var relative = directory.Substring(sourceRoot.Length).TrimStart(Path.DirectorySeparatorChar);
            Directory.CreateDirectory(Path.Combine(destinationRoot, relative));
        }

        foreach (var file in Directory.GetFiles(sourceRoot, "*", SearchOption.AllDirectories))
        {
            var relative = file.Substring(sourceRoot.Length).TrimStart(Path.DirectorySeparatorChar);
            var destination = Path.Combine(destinationRoot, relative);
            var parent = Path.GetDirectoryName(destination);
            if (!string.IsNullOrEmpty(parent))
            {
                Directory.CreateDirectory(parent);
            }
            File.Copy(file, destination, true);
        }
    }

    private static string EnsureInstalledBrowserExecutable(string payloadContentRoot, string targetRoot)
    {
        var targetExe = Path.Combine(targetRoot, "cerbena.exe");
        if (File.Exists(targetExe))
        {
            return targetExe;
        }

        var sourceExe = Directory
            .GetFiles(payloadContentRoot, "cerbena.exe", SearchOption.AllDirectories)
            .OrderBy(path => path.Length)
            .FirstOrDefault();
        if (string.IsNullOrWhiteSpace(sourceExe))
        {
            throw new FileNotFoundException("Browser executable was not found in installer payload.", targetExe);
        }

        Directory.CreateDirectory(targetRoot);
        File.Copy(sourceExe, targetExe, true);

        for (var attempt = 0; attempt < 20; attempt++)
        {
            if (File.Exists(targetExe))
            {
                return targetExe;
            }
            System.Threading.Thread.Sleep(150);
        }

        throw new FileNotFoundException("Installed browser executable was not found.", targetExe);
    }

    private static string ResolvePayloadContentRoot(string extractRoot)
    {
        var directExe = Path.Combine(extractRoot, "cerbena.exe");
        if (File.Exists(directExe))
        {
            return extractRoot;
        }

        var nestedExe = Directory
            .GetFiles(extractRoot, "cerbena.exe", SearchOption.AllDirectories)
            .OrderBy(path => path.Length)
            .FirstOrDefault();
        if (!string.IsNullOrEmpty(nestedExe))
        {
            var nestedRoot = Path.GetDirectoryName(nestedExe);
            if (!string.IsNullOrEmpty(nestedRoot))
            {
                return nestedRoot;
            }
        }

        return extractRoot;
    }

    private static string GetKnownFolderPath(Guid folderId)
    {
        IntPtr rawPath = IntPtr.Zero;
        try
        {
            var hr = SHGetKnownFolderPath(ref folderId, 0, IntPtr.Zero, out rawPath);
            if (hr != 0 || rawPath == IntPtr.Zero)
            {
                throw new InvalidOperationException("Unable to resolve known folder path. HRESULT=" + hr);
            }
            return Marshal.PtrToStringUni(rawPath);
        }
        finally
        {
            if (rawPath != IntPtr.Zero)
            {
                Marshal.FreeCoTaskMem(rawPath);
            }
        }
    }

    private static void CreateShortcut(string shortcutPath, string targetPath, string workingDirectory, string iconPath)
    {
        var shortcutDirectory = Path.GetDirectoryName(shortcutPath);
        if (string.IsNullOrWhiteSpace(shortcutDirectory))
        {
            throw new InvalidOperationException("Shortcut directory could not be resolved.");
        }

        Directory.CreateDirectory(shortcutDirectory);
        LogInstaller("CreateShortcut requested path=" + shortcutPath);
        var beforeFiles = Directory.GetFiles(shortcutDirectory, "*.lnk", SearchOption.TopDirectoryOnly);
        var tempShortcutPath = Path.Combine(shortcutDirectory, "cerbena-shortcut-" + Guid.NewGuid().ToString("N") + ".lnk");
        var shellLink = (IShellLinkW)new ShellLink();
        try
        {
            shellLink.SetPath(targetPath);
            shellLink.SetWorkingDirectory(workingDirectory);
            shellLink.SetDescription(ProductName);
            shellLink.SetIconLocation(iconPath, 0);
            ((IPersistFile)shellLink).Save(tempShortcutPath, false);
        }
        finally
        {
            Marshal.FinalReleaseComObject(shellLink);
        }

        var actualShortcutPath = tempShortcutPath;
        if (!File.Exists(actualShortcutPath))
        {
            actualShortcutPath = Directory
                .GetFiles(shortcutDirectory, "*.lnk", SearchOption.TopDirectoryOnly)
                .OrderByDescending(path => File.GetLastWriteTimeUtc(path))
                .FirstOrDefault(path => File.GetLastWriteTimeUtc(path) >= DateTime.UtcNow.AddMinutes(-2));
        }
        if (string.IsNullOrWhiteSpace(actualShortcutPath) || !File.Exists(actualShortcutPath))
        {
            var missingFiles = Directory.GetFiles(shortcutDirectory, "*.lnk", SearchOption.TopDirectoryOnly);
            LogInstaller("CreateShortcut failed to detect saved file. Files before=[" + string.Join(", ", beforeFiles.Select(Path.GetFileName)) + "] after=[" + string.Join(", ", missingFiles.Select(Path.GetFileName)) + "]");
            throw new InvalidOperationException("Shortcut file was not created.");
        }

        LogInstaller("CreateShortcut actual saved path=" + actualShortcutPath);

        if (File.Exists(shortcutPath))
        {
            File.Delete(shortcutPath);
        }

        if (!string.Equals(actualShortcutPath, shortcutPath, StringComparison.OrdinalIgnoreCase))
        {
            File.Move(actualShortcutPath, shortcutPath);
        }

        if (!File.Exists(shortcutPath))
        {
            var afterFiles = Directory.GetFiles(shortcutDirectory, "*.lnk", SearchOption.TopDirectoryOnly);
            LogInstaller("CreateShortcut failed to save expected name. Expected=" + shortcutPath + " actualFiles=[" + string.Join(", ", afterFiles.Select(Path.GetFileName)) + "]");
            throw new InvalidOperationException("Shortcut file was not saved with the expected name.");
        }

        var finalFiles = Directory.GetFiles(shortcutDirectory, "*.lnk", SearchOption.TopDirectoryOnly);
        LogInstaller("CreateShortcut final files=[" + string.Join(", ", finalFiles.Select(Path.GetFileName)) + "]");
    }

    private static void RemoveShortcut(string shortcutPath)
    {
        if (File.Exists(shortcutPath))
        {
            File.Delete(shortcutPath);
        }
    }

    private static void RegisterUninstaller(string installRoot, string displayIconPath, string uninstallerPath)
    {
        using (var key = Registry.CurrentUser.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\Cerbena Browser"))
        {
            if (key == null)
            {
                return;
            }

            key.SetValue("DisplayName", ProductName);
            key.SetValue("DisplayVersion", DisplayVersion);
            key.SetValue("Publisher", Publisher);
            key.SetValue("InstallLocation", installRoot);
            key.SetValue("DisplayIcon", displayIconPath);
            key.SetValue("UninstallString", "\"" + uninstallerPath + "\" --uninstall");
            key.SetValue("QuietUninstallString", "\"" + uninstallerPath + "\" --uninstall --silent");
            key.SetValue("NoModify", 1, RegistryValueKind.DWord);
            key.SetValue("NoRepair", 1, RegistryValueKind.DWord);
        }
    }

    private static void RemoveUninstallRegistration()
    {
        Registry.CurrentUser.DeleteSubKeyTree(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\Cerbena Browser", false);
    }

    private static void RegisterBrowserCapabilities(string browserExePath, string displayIconPath)
    {
        var command = "\"" + browserExePath + "\" \"%1\"";
        RegisterProgId(BrowserUrlProgId, ProductName, displayIconPath, command, true);
        RegisterProgId(BrowserHtmlProgId, "Cerbena HTML Document", displayIconPath, command, false);
        RegisterProgId(BrowserMhtmlProgId, "Cerbena MHTML Document", displayIconPath, command, false);
        RegisterProgId(BrowserPdfProgId, "Cerbena PDF Document", displayIconPath, command, false);
        RegisterProgId(BrowserXhtmlProgId, "Cerbena XHTML Document", displayIconPath, command, false);
        RegisterProgId(BrowserSvgProgId, "Cerbena SVG Document", displayIconPath, command, false);

        using (var clientKey = Registry.CurrentUser.CreateSubKey(BrowserClientSubKey))
        {
            if (clientKey != null)
            {
                clientKey.SetValue(null, ProductName);
                clientKey.SetValue("LocalizedString", ProductName);
                using (var iconKey = clientKey.CreateSubKey("DefaultIcon"))
                {
                    if (iconKey != null)
                    {
                        iconKey.SetValue(null, displayIconPath);
                    }
                }
                using (var commandKey = clientKey.CreateSubKey(@"shell\open\command"))
                {
                    if (commandKey != null)
                    {
                        commandKey.SetValue(null, command);
                    }
                }
                using (var capabilitiesKey = clientKey.CreateSubKey("Capabilities"))
                {
                    if (capabilitiesKey != null)
                    {
                        capabilitiesKey.SetValue("ApplicationName", ProductName);
                        capabilitiesKey.SetValue("ApplicationDescription", BrowserDescription);
                        using (var urlKey = capabilitiesKey.CreateSubKey("UrlAssociations"))
                        {
                            if (urlKey != null)
                            {
                                foreach (var scheme in new[] { "http", "https", "irc", "mailto", "mms", "news", "nntp", "sms", "smsto", "snews", "tel", "urn", "webcal" })
                                {
                                    urlKey.SetValue(scheme, BrowserUrlProgId);
                                }
                            }
                        }
                        using (var fileKey = capabilitiesKey.CreateSubKey("FileAssociations"))
                        {
                            if (fileKey != null)
                            {
                                fileKey.SetValue(".htm", BrowserHtmlProgId);
                                fileKey.SetValue(".html", BrowserHtmlProgId);
                                fileKey.SetValue(".shtml", BrowserHtmlProgId);
                                fileKey.SetValue(".mht", BrowserMhtmlProgId);
                                fileKey.SetValue(".mhtml", BrowserMhtmlProgId);
                                fileKey.SetValue(".pdf", BrowserPdfProgId);
                                fileKey.SetValue(".svg", BrowserSvgProgId);
                                fileKey.SetValue(".xhy", BrowserXhtmlProgId);
                                fileKey.SetValue(".xht", BrowserXhtmlProgId);
                                fileKey.SetValue(".xhtml", BrowserXhtmlProgId);
                            }
                        }
                    }
                }
            }
        }

        using (var registeredApps = Registry.CurrentUser.CreateSubKey(RegisteredApplicationsSubKey))
        {
            if (registeredApps != null)
            {
                registeredApps.SetValue(ProductName, BrowserClientSubKey + @"\Capabilities");
            }
        }

        RegisterOpenWith(".htm", BrowserHtmlProgId);
        RegisterOpenWith(".html", BrowserHtmlProgId);
        RegisterOpenWith(".shtml", BrowserHtmlProgId);
        RegisterOpenWith(".mht", BrowserMhtmlProgId);
        RegisterOpenWith(".mhtml", BrowserMhtmlProgId);
        RegisterOpenWith(".pdf", BrowserPdfProgId);
        RegisterOpenWith(".svg", BrowserSvgProgId);
        RegisterOpenWith(".xhy", BrowserXhtmlProgId);
        RegisterOpenWith(".xht", BrowserXhtmlProgId);
        RegisterOpenWith(".xhtml", BrowserXhtmlProgId);
    }

    private static void RegisterProgId(string progId, string displayName, string displayIconPath, string command, bool urlProtocol)
    {
        using (var key = Registry.CurrentUser.CreateSubKey(@"Software\Classes\" + progId))
        {
            if (key == null)
            {
                return;
            }
            key.SetValue(null, displayName);
            if (urlProtocol)
            {
                key.SetValue("URL Protocol", string.Empty);
            }
            using (var iconKey = key.CreateSubKey("DefaultIcon"))
            {
                if (iconKey != null)
                {
                    iconKey.SetValue(null, displayIconPath);
                }
            }
            using (var commandKey = key.CreateSubKey(@"shell\open\command"))
            {
                if (commandKey != null)
                {
                    commandKey.SetValue(null, command);
                }
            }
        }
    }

    private static void RegisterOpenWith(string extension, string progId)
    {
        using (var key = Registry.CurrentUser.CreateSubKey(@"Software\Classes\" + extension + @"\OpenWithProgids"))
        {
            if (key != null)
            {
                key.SetValue(progId, new byte[0], RegistryValueKind.None);
            }
        }
    }

    private static void RemoveBrowserRegistration()
    {
        Registry.CurrentUser.DeleteSubKeyTree(BrowserClientSubKey, false);
        Registry.CurrentUser.DeleteSubKeyTree(@"Software\Classes\" + BrowserUrlProgId, false);
        Registry.CurrentUser.DeleteSubKeyTree(@"Software\Classes\" + BrowserHtmlProgId, false);
        Registry.CurrentUser.DeleteSubKeyTree(@"Software\Classes\" + BrowserMhtmlProgId, false);
        Registry.CurrentUser.DeleteSubKeyTree(@"Software\Classes\" + BrowserPdfProgId, false);
        Registry.CurrentUser.DeleteSubKeyTree(@"Software\Classes\" + BrowserXhtmlProgId, false);
        Registry.CurrentUser.DeleteSubKeyTree(@"Software\Classes\" + BrowserSvgProgId, false);
        RemoveOpenWith(".htm", BrowserHtmlProgId);
        RemoveOpenWith(".html", BrowserHtmlProgId);
        RemoveOpenWith(".shtml", BrowserHtmlProgId);
        RemoveOpenWith(".mht", BrowserMhtmlProgId);
        RemoveOpenWith(".mhtml", BrowserMhtmlProgId);
        RemoveOpenWith(".pdf", BrowserPdfProgId);
        RemoveOpenWith(".xhy", BrowserXhtmlProgId);
        RemoveOpenWith(".xht", BrowserXhtmlProgId);
        RemoveOpenWith(".xhtml", BrowserXhtmlProgId);
        RemoveOpenWith(".svg", BrowserSvgProgId);
        using (var registeredApps = Registry.CurrentUser.OpenSubKey(RegisteredApplicationsSubKey, true))
        {
            if (registeredApps != null)
            {
                try
                {
                    registeredApps.DeleteValue(ProductName, false);
                }
                catch
                {
                }
            }
        }
    }

    private static void RemoveOpenWith(string extension, string progId)
    {
        using (var key = Registry.CurrentUser.OpenSubKey(@"Software\Classes\" + extension + @"\OpenWithProgids", true))
        {
            if (key != null)
            {
                try
                {
                    key.DeleteValue(progId, false);
                }
                catch
                {
                }
            }
        }
    }

    private static System.Collections.Generic.List<Process> FindRunningProductProcesses(string installRoot)
    {
        var matches = new System.Collections.Generic.List<Process>();
        var currentProcessId = Process.GetCurrentProcess().Id;
        foreach (var process in Process.GetProcesses())
        {
            try
            {
                if (process.Id == currentProcessId)
                {
                    continue;
                }
                var module = process.MainModule;
                var path = module != null ? module.FileName : null;
                if (string.IsNullOrWhiteSpace(path))
                {
                    continue;
                }
                if (string.Equals(path, Application.ExecutablePath, StringComparison.OrdinalIgnoreCase))
                {
                    continue;
                }
                if (!path.StartsWith(installRoot, StringComparison.OrdinalIgnoreCase))
                {
                    continue;
                }
                matches.Add(process);
            }
            catch
            {
            }
        }
        return matches;
    }

    private static bool TryTerminateProcesses(System.Collections.Generic.IEnumerable<Process> processes)
    {
        var allStopped = true;
        foreach (var process in processes)
        {
            try
            {
                if (!process.HasExited)
                {
                    process.Kill();
                    process.WaitForExit(5000);
                }
            }
            catch
            {
                allStopped = false;
            }
        }
        return allStopped;
    }

    private static void LogInstaller(string message)
    {
        try
        {
            File.AppendAllText(
                InstallerLogPath,
                DateTime.UtcNow.ToString("o") + " " + message + Environment.NewLine);
        }
        catch
        {
        }
    }

    [DllImport("shell32.dll")]
    private static extern int SHGetKnownFolderPath(
        ref Guid rfid,
        uint dwFlags,
        IntPtr hToken,
        out IntPtr ppszPath);

    [ComImport]
    [Guid("00021401-0000-0000-C000-000000000046")]
    private class ShellLink
    {
    }

    [ComImport]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    [Guid("000214F9-0000-0000-C000-000000000046")]
    private interface IShellLinkW
    {
        void GetPath([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszFile, int cchMaxPath, IntPtr pfd, uint fFlags);
        void GetIDList(out IntPtr ppidl);
        void SetIDList(IntPtr pidl);
        void GetDescription([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszName, int cchMaxName);
        void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string pszName);
        void GetWorkingDirectory([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszDir, int cchMaxPath);
        void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string pszDir);
        void GetArguments([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszArgs, int cchMaxPath);
        void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string pszArgs);
        void GetHotkey(out short pwHotkey);
        void SetHotkey(short wHotkey);
        void GetShowCmd(out int piShowCmd);
        void SetShowCmd(int iShowCmd);
        void GetIconLocation([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszIconPath, int cchIconPath, out int piIcon);
        void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)] string pszIconPath, int iIcon);
        void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)] string pszPathRel, uint dwReserved);
        void Resolve(IntPtr hwnd, uint fFlags);
        void SetPath([MarshalAs(UnmanagedType.LPWStr)] string pszFile);
    }

    [ComImport]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    [Guid("0000010b-0000-0000-C000-000000000046")]
    private interface IPersistFile
    {
        void GetClassID(out Guid pClassID);
        void IsDirty();
        void Load([MarshalAs(UnmanagedType.LPWStr)] string pszFileName, uint dwMode);
        void Save([MarshalAs(UnmanagedType.LPWStr)] string pszFileName, bool fRemember);
        void SaveCompleted([MarshalAs(UnmanagedType.LPWStr)] string pszFileName);
        void GetCurFile([MarshalAs(UnmanagedType.LPWStr)] out string ppszFileName);
    }
}
"@
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($sourcePath, $installerSource, $utf8)

    Invoke-Native $csharpCompiler @(
        "/nologo",
        "/target:winexe",
        "/out:$targetExe",
        "/resource:$payloadArchivePath,PayloadArchive",
        "/resource:$brandLogoPath,BrandLogo",
        "/resource:$setupIconPath,ShortcutIcon",
        "/win32icon:$setupIconPath",
        "/r:System.Drawing.dll",
        "/r:System.IO.Compression.FileSystem.dll",
        "/r:System.Windows.Forms.dll",
        $sourcePath
    )
    if (-not (Test-Path $targetExe)) {
        throw "csharp fallback did not produce installer exe: $targetExe"
    }
    return $targetExe
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$tauriConfig = Read-JsonFile (Join-Path $repoRoot "ui\desktop\src-tauri\tauri.conf.json")
$resolvedVersion = $Version
if ([string]::IsNullOrWhiteSpace($resolvedVersion)) {
    $resolvedVersion = [string]$tauriConfig.version
}
if ([string]::IsNullOrWhiteSpace($resolvedVersion)) {
    throw "unable to resolve version"
}

$releaseBundleRoot = Join-Path $repoRoot ("build\release\" + $resolvedVersion + "\staging\cerbena-windows-x64")
if (-not $SkipReleasePackaging) {
    Invoke-Native "powershell" @(
        "-ExecutionPolicy", "Bypass",
        "-File", (Join-Path $repoRoot "scripts\generate-release-artifacts.ps1"),
        "-Version", $resolvedVersion
    )
}
if (-not (Test-Path $releaseBundleRoot)) {
    throw "release payload not found: $releaseBundleRoot"
}

$installerRoot = Join-Path $repoRoot ("build\installer\" + $resolvedVersion)
$payloadRoot = Join-Path $installerRoot "payload"
$issPath = Join-Path $installerRoot "CerbenaBrowserInstaller.iss"
$outputDir = Join-Path $installerRoot "output"
$exeInstallModeMarkerPath = Join-Path $installerRoot "cerbena-install-mode.exe.txt"

if (Test-Path $installerRoot) {
    Remove-Item -LiteralPath $installerRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $payloadRoot -Force | Out-Null
New-Item -ItemType Directory -Path $outputDir -Force | Out-Null

Copy-Item -Path (Join-Path $releaseBundleRoot "*") -Destination $payloadRoot -Recurse -Force
if (Test-Path (Join-Path $repoRoot "LICENSE.txt")) {
    Copy-Item -LiteralPath (Join-Path $repoRoot "LICENSE.txt") -Destination (Join-Path $payloadRoot "LICENSE.txt") -Force
}
$setupIconPath = Join-Path $repoRoot "static\img\favicon.ico"
if (-not (Test-Path $setupIconPath)) {
    throw "installer icon not found: $setupIconPath"
}
Copy-Item -LiteralPath $setupIconPath -Destination (Join-Path $payloadRoot "cerbena.ico") -Force
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($exeInstallModeMarkerPath, "exe`n", $utf8)
Copy-Item -LiteralPath $exeInstallModeMarkerPath -Destination (Join-Path $payloadRoot "cerbena-install-mode.txt") -Force

$payloadRootInno = Convert-ToInnoPath $payloadRoot
$outputDirInno = Convert-ToInnoPath $outputDir
$setupIconInno = Convert-ToInnoPath $setupIconPath
$iss = @"
#define MyAppName "Cerbena Browser"
#define MyAppVersion "$resolvedVersion"
#define MyAppPublisher "Berkut Solutions"
#define MyAppURL "https://github.com/BerkutSolutions/cerbena-browser"
#define MyAppExeName "cerbena.exe"
#define MyLauncherExeName "cerbena-launcher.exe"

[Setup]
AppId={{0C85D31C-71D2-4B20-8D95-3024E67F4B6C}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={localappdata}\Cerbena Browser
DefaultGroupName={#MyAppName}
DisableDirPage=no
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
SetupIconFile=$setupIconInno
OutputDir=$outputDirInno
OutputBaseFilename=cerbena-browser-setup-$resolvedVersion
Compression=lzma
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "$payloadRootInno\\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autoprograms}\{#MyAppName} Launcher"; Filename: "{app}\{#MyLauncherExeName}"; Check: LauncherExists

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: files; Name: "{app}\.app-secret.dpapi"
Type: files; Name: "{app}\identity_store.json"
Type: files; Name: "{app}\network_store.json"
Type: files; Name: "{app}\network_sandbox_store.json"
Type: files; Name: "{app}\extension_library.json"
Type: files; Name: "{app}\sync_store.json"
Type: files; Name: "{app}\link_routing_store.json"
Type: files; Name: "{app}\launch_session_store.json"
Type: files; Name: "{app}\device_posture_store.json"
Type: files; Name: "{app}\app_update_store.json"
Type: files; Name: "{app}\global_security_store.json"
Type: files; Name: "{app}\traffic_gateway_log.json"
Type: files; Name: "{app}\traffic_gateway_rules.json"
Type: filesandordirs; Name: "{app}\profiles"
Type: filesandordirs; Name: "{app}\engine-runtime"
Type: filesandordirs; Name: "{app}\network-runtime"
Type: filesandordirs; Name: "{app}\extension-packages"
Type: filesandordirs; Name: "{app}\updates"
Type: filesandordirs; Name: "{app}\native-messaging"

[Code]
var
  DesktopShortcutCheckBox: TNewCheckBox;
  LegacyAmneziaServicePrefix: string;

function LauncherExists: Boolean;
begin
  Result := FileExists(ExpandConstant('{app}\{#MyLauncherExeName}'));
end;

procedure InitializeWizard;
begin
  LegacyAmneziaServicePrefix := 'AmneziaWGTunnel`$awg-';
  DesktopShortcutCheckBox := TNewCheckBox.Create(WizardForm);
  DesktopShortcutCheckBox.Parent := WizardForm.FinishedPage.Surface;
  DesktopShortcutCheckBox.Caption := 'Create a desktop shortcut';
  DesktopShortcutCheckBox.Checked := True;
  DesktopShortcutCheckBox.Left := WizardForm.RunList.Left;
  DesktopShortcutCheckBox.Top := WizardForm.RunList.Top - ScaleY(24);
  DesktopShortcutCheckBox.Width := WizardForm.RunList.Width;
end;

function NextButtonClick(CurPageID: Integer): Boolean;
var
  ShortcutPath: string;
begin
  Result := True;
  if CurPageID = wpFinished then
  begin
    ShortcutPath := ExpandConstant('{autodesktop}\{#MyAppName}.lnk');
    if DesktopShortcutCheckBox.Checked then
    begin
      CreateShellLink(
        ShortcutPath,
        '{#MyAppName}',
        ExpandConstant('{app}\{#MyAppExeName}'),
        '',
        ExpandConstant('{app}'),
        '',
        ExpandConstant('{app}\{#MyAppExeName}'),
        0,
        SW_SHOWNORMAL);
    end
    else if FileExists(ShortcutPath) then
    begin
      DeleteFile(ShortcutPath);
    end;
  end;
end;

procedure TryRunSc(const ActionName, ServiceName: string);
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\sc.exe'), ActionName + ' "' + ServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

procedure CleanupLegacyAmneziaServices();
var
  TempFile: string;
  Lines: TArrayOfString;
  I: Integer;
  ServiceName: string;
  ResultCode: Integer;
begin
  TempFile := ExpandConstant('{tmp}\cerbena-amnezia-services.txt');
  if Exec(
    ExpandConstant('{cmd}'),
    '/C sc.exe query state= all > "' + TempFile + '"',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  ) then
  begin
    if LoadStringsFromFile(TempFile, Lines) then
    begin
      for I := 0 to GetArrayLength(Lines) - 1 do
      begin
        if Pos('SERVICE_NAME:', Trim(Lines[I])) = 1 then
        begin
          ServiceName := Trim(Copy(Trim(Lines[I]), Length('SERVICE_NAME:') + 1, MaxInt));
          if Pos(LegacyAmneziaServicePrefix, ServiceName) = 1 then
          begin
            TryRunSc('stop', ServiceName);
            TryRunSc('delete', ServiceName);
          end;
        end;
      end;
    end;
    DeleteFile(TempFile);
  end;
end;

procedure TryRunDocker(const Arguments: string);
var
  ResultCode: Integer;
begin
  Exec('docker.exe', Arguments, '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

procedure CleanupManagedContainerArtifacts();
var
  TempFile: string;
  Lines: TArrayOfString;
  I: Integer;
  Name: string;
  ResultCode: Integer;
begin
  TempFile := ExpandConstant('{tmp}\cerbena-docker-managed.txt');
  if Exec(
    ExpandConstant('{cmd}'),
    '/C docker.exe ps -a --filter label=cerbena.kind=network-sandbox-runtime --format "{{.Names}}" > "' + TempFile + '"',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  ) then
  begin
    if LoadStringsFromFile(TempFile, Lines) then
    begin
      for I := 0 to GetArrayLength(Lines) - 1 do
      begin
        Name := Trim(Lines[I]);
        if Name <> '' then
        begin
          TryRunDocker('rm -f "' + Name + '"');
        end;
      end;
    end;
    DeleteFile(TempFile);
  end;

  TempFile := ExpandConstant('{tmp}\cerbena-docker-networks.txt');
  if Exec(
    ExpandConstant('{cmd}'),
    '/C docker.exe network ls --format "{{.Name}}" > "' + TempFile + '"',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  ) then
  begin
    if LoadStringsFromFile(TempFile, Lines) then
    begin
      for I := 0 to GetArrayLength(Lines) - 1 do
      begin
        Name := Trim(Lines[I]);
        if Pos('cerbena-profile-', Name) = 1 then
        begin
          TryRunDocker('network rm "' + Name + '"');
        end;
      end;
    end;
    DeleteFile(TempFile);
  end;

  TryRunDocker('image rm -f cerbena/network-sandbox:2026-05-02-r5');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    CleanupLegacyAmneziaServices();
    CleanupManagedContainerArtifacts();
  end;
end;
"@

[System.IO.File]::WriteAllText($issPath, $iss, $utf8)

$compiler = Find-InnoSetupCompiler
$builtArtifactSpecs = New-Object System.Collections.Generic.List[hashtable]
if ($GenerateOnly -or [string]::IsNullOrWhiteSpace($compiler)) {
    if (-not [string]::IsNullOrWhiteSpace($compiler)) {
        Write-Host "Installer script generated at $issPath" -ForegroundColor Green
        return
    }

    $fallbackExe = New-CSharpFallbackInstaller -InstallerRoot $installerRoot -PayloadRoot $payloadRoot -Version $resolvedVersion
    Sign-WindowsArtifacts @($fallbackExe)
    [void]$builtArtifactSpecs.Add(@{
        Name = [System.IO.Path]::GetFileName($fallbackExe)
        Target = [System.IO.Path]::GetFileName($fallbackExe)
        Source = $fallbackExe
        Kind = "installer"
        InstallerKind = "exe_fallback"
        UpdaterStrategy = "manual_installer"
        Primary = $false
    })
    Write-Warning "Inno Setup compiler (ISCC.exe) not found. Built C# fallback installer instead: $fallbackExe"
} else {
    Invoke-Native $compiler @($issPath)
    $innoExe = Join-Path $outputDir ("cerbena-browser-setup-" + $resolvedVersion + ".exe")
    if (Test-Path $innoExe) {
        [void]$builtArtifactSpecs.Add(@{
            Name = [System.IO.Path]::GetFileName($innoExe)
            Target = [System.IO.Path]::GetFileName($innoExe)
            Source = $innoExe
            Kind = "installer"
            InstallerKind = "exe_fallback"
            UpdaterStrategy = "manual_installer"
            Primary = $false
        })
    }
}

[void](Remove-Item -LiteralPath (Join-Path $payloadRoot "cerbena-install-mode.txt") -Force -ErrorAction SilentlyContinue)
$msiPath = New-MsiInstaller -InstallerRoot $installerRoot -PayloadRoot $payloadRoot -Version $resolvedVersion -RepoRoot $repoRoot
[void]$builtArtifactSpecs.Add(@{
    Name = [System.IO.Path]::GetFileName($msiPath)
    Target = [System.IO.Path]::GetFileName($msiPath)
    Source = $msiPath
    Kind = "installer"
    InstallerKind = "msi"
    UpdaterStrategy = "direct_msi"
    Primary = $true
})

Sign-WindowsArtifacts @($outputDir, $installerRoot)
$builtArtifacts = @(
    $builtArtifactSpecs | ForEach-Object {
        New-ReleaseMetadataEntry `
            -Name $_.Name `
            -Target $_.Target `
            -Source $_.Source `
            -Kind $_.Kind `
            -InstallerKind $_.InstallerKind `
            -UpdaterStrategy $_.UpdaterStrategy `
            -Primary ([bool]$_.Primary)
    }
)
Update-ReleaseMetadataWithInstallerAssets -RepoRoot $repoRoot -Version $resolvedVersion -InstallerArtifacts $builtArtifacts

if ($GenerateOnly -and -not [string]::IsNullOrWhiteSpace($compiler)) {
    Write-Host "Installer script generated at $issPath" -ForegroundColor Green
    return
}
Write-Host "Installer built in $outputDir" -ForegroundColor Green
