Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-ReleaseSigningPublicKeyPath([string]$RepoRoot) {
    return Join-Path $RepoRoot "config\release\release-signing-public-key.xml"
}

function Get-LatestLocalSigningMaterialDirectory([string]$RepoRoot) {
    $root = Join-Path $RepoRoot "build\operator-secrets\release-signing"
    if (-not (Test-Path $root)) {
        return $null
    }
    $candidates = Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTimeUtc -Descending
    foreach ($candidate in $candidates) {
        $privateKeyPath = Join-Path $candidate.FullName "release-signing-private-key.xml"
        $publicKeyPath = Join-Path $candidate.FullName "release-signing-public-key.xml"
        $pfxPath = Join-Path $candidate.FullName "release-signing-authenticode.pfx"
        $passwordPath = Join-Path $candidate.FullName "pfx-password.txt"
        if ((Test-Path $privateKeyPath) -and (Test-Path $publicKeyPath) -and (Test-Path $pfxPath) -and (Test-Path $passwordPath)) {
            return $candidate.FullName
        }
    }
    return $null
}

function Read-Utf8TrimmedFile([string]$Path) {
    return ([System.IO.File]::ReadAllText($Path, [System.Text.Encoding]::UTF8)).Trim()
}

function Assert-LocalSigningMaterialMatchesCommittedPublicKey([string]$RepoRoot, [string]$MaterialDirectory) {
    if ([string]::IsNullOrWhiteSpace($MaterialDirectory)) {
        return
    }
    $committedPublicKeyPath = Get-ReleaseSigningPublicKeyPath $RepoRoot
    $localPublicKeyPath = Join-Path $MaterialDirectory "release-signing-public-key.xml"
    if (-not (Test-Path $committedPublicKeyPath) -or -not (Test-Path $localPublicKeyPath)) {
        return
    }

    $committed = Read-Utf8TrimmedFile $committedPublicKeyPath
    $local = Read-Utf8TrimmedFile $localPublicKeyPath
    if ($committed -ne $local) {
        throw @"
local signing material does not match the committed public verification key.
Committed: $committedPublicKeyPath
Local bundle: $MaterialDirectory

Either:
1. point CERBENA_* signing variables to the matching current signing material; or
2. rotate trust intentionally by replacing config/release/release-signing-public-key.xml with the local release-signing-public-key.xml and commit that change.
"@
    }
}

function Initialize-ReleaseSigningEnvironment([string]$RepoRoot) {
    $candidateDirectory = Get-LatestLocalSigningMaterialDirectory $RepoRoot
    $usedLocalCandidate = $false

    $privateKeyXml = [Environment]::GetEnvironmentVariable("CERBENA_RELEASE_SIGNING_PRIVATE_KEY_XML")
    if ([string]::IsNullOrWhiteSpace($privateKeyXml)) {
        $privateKeyPath = [Environment]::GetEnvironmentVariable("CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH")
        if ([string]::IsNullOrWhiteSpace($privateKeyPath) -and -not [string]::IsNullOrWhiteSpace($candidateDirectory)) {
            [Environment]::SetEnvironmentVariable(
                "CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH",
                (Join-Path $candidateDirectory "release-signing-private-key.xml")
            )
            $usedLocalCandidate = $true
        }
    }

    $pfxPath = [Environment]::GetEnvironmentVariable("CERBENA_AUTHENTICODE_PFX_PATH")
    if ([string]::IsNullOrWhiteSpace($pfxPath) -and -not [string]::IsNullOrWhiteSpace($candidateDirectory)) {
        [Environment]::SetEnvironmentVariable(
            "CERBENA_AUTHENTICODE_PFX_PATH",
            (Join-Path $candidateDirectory "release-signing-authenticode.pfx")
        )
        $usedLocalCandidate = $true
    }

    $pfxPassword = [Environment]::GetEnvironmentVariable("CERBENA_AUTHENTICODE_PFX_PASSWORD")
    if ([string]::IsNullOrWhiteSpace($pfxPassword) -and -not [string]::IsNullOrWhiteSpace($candidateDirectory)) {
        $passwordPath = Join-Path $candidateDirectory "pfx-password.txt"
        if (Test-Path $passwordPath) {
            [Environment]::SetEnvironmentVariable(
                "CERBENA_AUTHENTICODE_PFX_PASSWORD",
                (Read-Utf8TrimmedFile $passwordPath)
            )
            $usedLocalCandidate = $true
        }
    }

    if ($usedLocalCandidate) {
        Assert-LocalSigningMaterialMatchesCommittedPublicKey -RepoRoot $RepoRoot -MaterialDirectory $candidateDirectory
    }

    $hasPrivateKey = -not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable("CERBENA_RELEASE_SIGNING_PRIVATE_KEY_XML")) `
        -or -not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable("CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH"))
    $hasPfxPath = -not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable("CERBENA_AUTHENTICODE_PFX_PATH"))
    $hasPfxPassword = -not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable("CERBENA_AUTHENTICODE_PFX_PASSWORD"))
    if ($hasPrivateKey -and $hasPfxPath -and $hasPfxPassword) {
        return
    }

    $bootstrapHint = "powershell -ExecutionPolicy Bypass -File .\scripts\new-release-signing-material.ps1"
    throw @"
release signing material is incomplete.

Required:
- CERBENA_RELEASE_SIGNING_PRIVATE_KEY_XML or CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH
- CERBENA_AUTHENTICODE_PFX_PATH
- CERBENA_AUTHENTICODE_PFX_PASSWORD

Recommended local bootstrap:
$bootstrapHint

After bootstrap, either:
- set the CERBENA_* environment variables explicitly; or
- keep the generated bundle under build/operator-secrets/release-signing so the local release flow can auto-discover it.
"@
}

function Get-ReleaseSigningPrivateKeyXml() {
    $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
    Initialize-ReleaseSigningEnvironment $repoRoot
    $inlineValue = [Environment]::GetEnvironmentVariable("CERBENA_RELEASE_SIGNING_PRIVATE_KEY_XML")
    if (-not [string]::IsNullOrWhiteSpace($inlineValue)) {
        return $inlineValue.Trim()
    }

    $pathValue = [Environment]::GetEnvironmentVariable("CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH")
    if (-not [string]::IsNullOrWhiteSpace($pathValue)) {
        $resolved = [Environment]::ExpandEnvironmentVariables($pathValue.Trim())
        if (-not (Test-Path $resolved)) {
            throw "CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH does not exist: $resolved"
        }
        return ([System.IO.File]::ReadAllText($resolved, [System.Text.Encoding]::UTF8)).Trim()
    }

    throw "release checksum signing key is missing; set CERBENA_RELEASE_SIGNING_PRIVATE_KEY_XML or CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH"
}

function New-ReleaseChecksumSignature([byte[]]$ChecksumBytes) {
    $privateKeyXml = Get-ReleaseSigningPrivateKeyXml
    $rsa = New-Object System.Security.Cryptography.RSACryptoServiceProvider
    $rsa.PersistKeyInCsp = $false
    $rsa.FromXmlString($privateKeyXml)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $signature = $rsa.SignData($ChecksumBytes, $sha)
        return [Convert]::ToBase64String($signature)
    } finally {
        $rsa.Dispose()
        $sha.Dispose()
    }
}

function Verify-ReleaseChecksumSignature(
    [string]$ChecksumsPath,
    [string]$SignaturePath,
    [string]$PublicKeyPath = ""
) {
    if ([string]::IsNullOrWhiteSpace($PublicKeyPath)) {
        $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
        $PublicKeyPath = Get-ReleaseSigningPublicKeyPath $repoRoot
    }
    if (-not (Test-Path $ChecksumsPath)) {
        throw "checksums file does not exist: $ChecksumsPath"
    }
    if (-not (Test-Path $SignaturePath)) {
        throw "checksums signature does not exist: $SignaturePath"
    }
    if (-not (Test-Path $PublicKeyPath)) {
        throw "release signing public key does not exist: $PublicKeyPath"
    }

    $checksumsBytes = [System.IO.File]::ReadAllBytes((Resolve-Path $ChecksumsPath).Path)
    $signatureText = ([System.IO.File]::ReadAllText((Resolve-Path $SignaturePath).Path, [System.Text.Encoding]::UTF8)).Trim()
    $publicKeyXml = ([System.IO.File]::ReadAllText((Resolve-Path $PublicKeyPath).Path, [System.Text.Encoding]::UTF8)).Trim()
    $signatureBytes = [Convert]::FromBase64String($signatureText)

    $rsa = New-Object System.Security.Cryptography.RSACryptoServiceProvider
    $rsa.PersistKeyInCsp = $false
    $rsa.FromXmlString($publicKeyXml)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        if (-not $rsa.VerifyData($checksumsBytes, $sha, $signatureBytes)) {
            throw "release checksum signature verification failed for $ChecksumsPath"
        }
        return $true
    } finally {
        $rsa.Dispose()
        $sha.Dispose()
    }
}

function Get-AuthenticodePfxPath() {
    $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
    Initialize-ReleaseSigningEnvironment $repoRoot
    $pathValue = [Environment]::GetEnvironmentVariable("CERBENA_AUTHENTICODE_PFX_PATH")
    if ([string]::IsNullOrWhiteSpace($pathValue)) {
        throw "Authenticode signing is required; set CERBENA_AUTHENTICODE_PFX_PATH"
    }
    $resolved = [Environment]::ExpandEnvironmentVariables($pathValue.Trim())
    if (-not (Test-Path $resolved)) {
        throw "CERBENA_AUTHENTICODE_PFX_PATH does not exist: $resolved"
    }
    return $resolved
}

function Get-AuthenticodePfxPassword() {
    $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
    Initialize-ReleaseSigningEnvironment $repoRoot
    $password = [Environment]::GetEnvironmentVariable("CERBENA_AUTHENTICODE_PFX_PASSWORD")
    if ([string]::IsNullOrWhiteSpace($password)) {
        throw "Authenticode signing is required; set CERBENA_AUTHENTICODE_PFX_PASSWORD"
    }
    return $password
}

function Get-AuthenticodeTimestampServer() {
    $value = [Environment]::GetEnvironmentVariable("CERBENA_AUTHENTICODE_TIMESTAMP_URL")
    if ([string]::IsNullOrWhiteSpace($value)) {
        return ""
    }
    return $value.Trim()
}

function Import-AuthenticodeCertificate() {
    $pfxPath = Get-AuthenticodePfxPath
    $password = Get-AuthenticodePfxPassword
    $flagCandidates = @(
        ([System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::Exportable `
            -bor [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::EphemeralKeySet),
        ([System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::Exportable `
            -bor [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::PersistKeySet),
        ([System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::Exportable)
    )

    $certificate = $null
    $errors = New-Object System.Collections.Generic.List[string]
    foreach ($flags in $flagCandidates) {
        try {
            $certificate = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(
                $pfxPath,
                $password,
                $flags
            )
            break
        } catch {
            [void]$errors.Add("flags=$flags :: $($_.Exception.Message)")
        }
    }

    if ($null -eq $certificate) {
        throw "failed to import Authenticode certificate from $pfxPath. Attempts: $($errors -join '; ')"
    }

    if (-not $certificate.HasPrivateKey) {
        $certificate.Dispose()
        throw "Authenticode certificate does not expose a private key: $pfxPath"
    }
    return $certificate
}

function Get-SignableWindowsArtifacts([string[]]$Roots) {
    $supported = @(".exe", ".dll", ".msi", ".sys")
    $results = New-Object System.Collections.Generic.List[string]
    foreach ($root in $Roots) {
        if ([string]::IsNullOrWhiteSpace($root) -or -not (Test-Path $root)) {
            continue
        }
        $resolvedRoot = (Resolve-Path $root).Path
        if ((Get-Item -LiteralPath $resolvedRoot).PSIsContainer) {
            $items = Get-ChildItem -LiteralPath $resolvedRoot -Recurse -File
        } else {
            $items = @(Get-Item -LiteralPath $resolvedRoot)
        }
        foreach ($item in $items) {
            $extension = $item.Extension.ToLowerInvariant()
            if ($supported -contains $extension -and -not $results.Contains($item.FullName)) {
                $results.Add($item.FullName)
            }
        }
    }
    return @($results | Sort-Object)
}

function Test-AuthenticodeSignatureAcceptable($Signature, [string]$ExpectedThumbprint) {
    if ($null -eq $Signature -or $null -eq $Signature.SignerCertificate) {
        return $false
    }

    $actualThumbprint = [string]$Signature.SignerCertificate.Thumbprint
    if ($actualThumbprint.Trim().ToUpperInvariant() -ne $ExpectedThumbprint.Trim().ToUpperInvariant()) {
        return $false
    }

    $status = [string]$Signature.Status
    return @("Valid", "NotTrusted", "UnknownError") -contains $status
}

function Assert-AuthenticodeSignatureValid([string]$ArtifactPath, [string]$ExpectedThumbprint) {
    $signature = Get-AuthenticodeSignature -FilePath $ArtifactPath
    if (Test-AuthenticodeSignatureAcceptable -Signature $signature -ExpectedThumbprint $ExpectedThumbprint) {
        return
    }

    $status = if ($null -eq $signature) { "Unknown" } else { [string]$signature.Status }
    $message = if ($null -eq $signature) { "" } else { [string]$signature.StatusMessage }
    $signer = if ($null -eq $signature.SignerCertificate) {
        "<missing>"
    } else {
        [string]$signature.SignerCertificate.Thumbprint
    }
    throw "Authenticode verification failed for $ArtifactPath (status=$status, signer=$signer, expected=$ExpectedThumbprint, detail=$message)"
}

function Sign-WindowsArtifacts([string[]]$ArtifactPaths) {
    $resolvedPaths = @(Get-SignableWindowsArtifacts -Roots $ArtifactPaths)
    if ($resolvedPaths.Count -eq 0) {
        throw "no Windows artifacts found for Authenticode signing"
    }

    $certificate = Import-AuthenticodeCertificate
    $thumbprint = [string]$certificate.Thumbprint
    $timestampServer = Get-AuthenticodeTimestampServer
    try {
        foreach ($path in $resolvedPaths) {
            if ([string]::IsNullOrWhiteSpace($timestampServer)) {
                $result = Set-AuthenticodeSignature -FilePath $path -Certificate $certificate
            } else {
                $result = Set-AuthenticodeSignature -FilePath $path -Certificate $certificate -TimestampServer $timestampServer
            }
            $status = [string]$result.Status
            if (-not (@("Valid", "NotTrusted", "UnknownError") -contains $status)) {
                throw "Authenticode signing failed for $path (status=$status, detail=$($result.StatusMessage))"
            }
            Assert-AuthenticodeSignatureValid -ArtifactPath $path -ExpectedThumbprint $thumbprint
        }
    } finally {
        $certificate.Dispose()
    }
}

function Verify-WindowsArtifacts([string[]]$ArtifactPaths) {
    $resolvedPaths = @(Get-SignableWindowsArtifacts -Roots $ArtifactPaths)
    if ($resolvedPaths.Count -eq 0) {
        throw "no Windows artifacts found for Authenticode verification"
    }

    $certificate = Import-AuthenticodeCertificate
    $thumbprint = [string]$certificate.Thumbprint
    try {
        foreach ($path in $resolvedPaths) {
            Assert-AuthenticodeSignatureValid -ArtifactPath $path -ExpectedThumbprint $thumbprint
        }
    } finally {
        $certificate.Dispose()
    }
}
