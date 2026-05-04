param(
    [string]$OutputDir = "",
    [string]$PfxPassword = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Utf8NoBomFile([string]$Path, [string]$Content) {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Convert-BytesToPem([byte[]]$Bytes, [string]$Label) {
    $base64 = [Convert]::ToBase64String($Bytes)
    $chunks = New-Object System.Collections.Generic.List[string]
    for ($index = 0; $index -lt $base64.Length; $index += 64) {
        $length = [Math]::Min(64, $base64.Length - $index)
        $chunks.Add($base64.Substring($index, $length))
    }
    return @(
        "-----BEGIN $Label-----"
        $chunks
        "-----END $Label-----"
    ) -join "`n"
}

function New-RandomPassword() {
    $bytes = New-Object byte[] 24
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    try {
        $rng.GetBytes($bytes)
    } finally {
        $rng.Dispose()
    }
    return [Convert]::ToBase64String($bytes)
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = Join-Path $repoRoot ".work\release-signing"
} elseif (-not [System.IO.Path]::IsPathRooted($OutputDir)) {
    $OutputDir = Join-Path $repoRoot $OutputDir
}
if ([string]::IsNullOrWhiteSpace($PfxPassword)) {
    $PfxPassword = New-RandomPassword
}

New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null

$rsa = [System.Security.Cryptography.RSA]::Create(3072)
$privateParameters = $rsa.ExportParameters($true)
$publicParameters = $rsa.ExportParameters($false)
$privateXml = $rsa.ToXmlString($true)
$publicXml = $rsa.ToXmlString($false)

$subject = New-Object System.Security.Cryptography.X509Certificates.X500DistinguishedName("CN=Cerbena Internal Release Signing")
$request = New-Object System.Security.Cryptography.X509Certificates.CertificateRequest(
    $subject,
    $rsa,
    [System.Security.Cryptography.HashAlgorithmName]::SHA256,
    [System.Security.Cryptography.RSASignaturePadding]::Pkcs1
)
$usage = [System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature
$request.CertificateExtensions.Add((New-Object System.Security.Cryptography.X509Certificates.X509KeyUsageExtension($usage, $true)))
$enhancedUsage = New-Object System.Security.Cryptography.OidCollection
[void]$enhancedUsage.Add((New-Object System.Security.Cryptography.Oid("1.3.6.1.5.5.7.3.3")))
$request.CertificateExtensions.Add((New-Object System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension($enhancedUsage, $true)))
$certificate = $request.CreateSelfSigned(
    [DateTimeOffset]::UtcNow.AddDays(-1),
    [DateTimeOffset]::UtcNow.AddYears(3)
)

$privateKeyPath = Join-Path $OutputDir "release-signing-private-key.xml"
$publicKeyPath = Join-Path $OutputDir "release-signing-public-key.xml"
$publicCertPath = Join-Path $OutputDir "release-signing-authenticode-public.cer.pem"
$pfxPath = Join-Path $OutputDir "release-signing-authenticode.pfx"
$passwordPath = Join-Path $OutputDir "pfx-password.txt"
$instructionsPath = Join-Path $OutputDir "README.txt"

[System.IO.File]::WriteAllBytes(
    $pfxPath,
    $certificate.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $PfxPassword)
)
Write-Utf8NoBomFile $privateKeyPath $privateXml
Write-Utf8NoBomFile $publicKeyPath $publicXml
Write-Utf8NoBomFile $publicCertPath (Convert-BytesToPem -Bytes $certificate.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Cert) -Label "CERTIFICATE")
Write-Utf8NoBomFile $passwordPath $PfxPassword
Write-Utf8NoBomFile $instructionsPath @"
Cerbena release signing bootstrap output

Files:
- release-signing-private-key.xml: detached checksum signing private key (secret)
- release-signing-public-key.xml: detached checksum signing public key (commit this into config/release after rotation)
- release-signing-authenticode.pfx: Authenticode signing certificate + private key (secret)
- release-signing-authenticode-public.cer.pem: exported public certificate for operator verification
- pfx-password.txt: password for the generated PFX (secret)

Release-time environment:
- CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH=$privateKeyPath
- CERBENA_AUTHENTICODE_PFX_PATH=$pfxPath
- CERBENA_AUTHENTICODE_PFX_PASSWORD=<read from pfx-password.txt>
- CERBENA_AUTHENTICODE_TIMESTAMP_URL=<optional RFC3161 or Authenticode timestamp URL>

Rotation flow:
1. Keep this directory local-only under .work/release-signing and never commit or publish the secret files.
2. Replace config/release/release-signing-public-key.xml with release-signing-public-key.xml when rotating trust intentionally.
3. Ship the next release only after all new artifacts are signed with the matching private material.
4. Keep old signed releases available for audit, but do not reuse rotated private keys.
"@

Write-Host "Generated release signing material at $OutputDir" -ForegroundColor Green
