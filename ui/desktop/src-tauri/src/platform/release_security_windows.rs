use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use std::process::Command;

pub fn verify_release_checksums_signature(
    checksums_bytes: &[u8],
    signature_b64: &str,
    public_key_xml: &str,
    checksums_env: &str,
    signature_env: &str,
) -> Result<(), String> {
    let script = r#"
$checksumsB64 = [Environment]::GetEnvironmentVariable('__CHECKSUMS_ENV__')
$signatureB64 = [Environment]::GetEnvironmentVariable('__SIGNATURE_ENV__')
if ([string]::IsNullOrWhiteSpace($checksumsB64) -or [string]::IsNullOrWhiteSpace($signatureB64)) {
  throw 'missing release verification environment'
}
$checksums = [Convert]::FromBase64String($checksumsB64)
$signature = [Convert]::FromBase64String($signatureB64)
$rsa = [System.Security.Cryptography.RSA]::Create()
$rsa.FromXmlString(@"
__PUBLIC_KEY_XML__
"@)
$sha = [System.Security.Cryptography.HashAlgorithmName]::SHA256
$padding = [System.Security.Cryptography.RSASignaturePadding]::Pkcs1
if (-not $rsa.VerifyData($checksums, $signature, $sha, $padding)) {
  throw 'release checksum signature verification failed'
}
"#
    .replace("__PUBLIC_KEY_XML__", public_key_xml)
    .replace("__CHECKSUMS_ENV__", checksums_env)
    .replace("__SIGNATURE_ENV__", signature_env);

    let mut command = Command::new("powershell");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &script,
    ]);
    command.env(checksums_env, B64.encode(checksums_bytes));
    command.env(signature_env, signature_b64);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .map_err(|e| format!("run checksum signature verification: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(format!(
        "release checksum signature verification failed (code {:?}){}{}",
        output.status.code(),
        if stderr.is_empty() {
            String::new()
        } else {
            format!(" stderr: {stderr}")
        },
        if stdout.is_empty() {
            String::new()
        } else {
            format!(" stdout: {stdout}")
        }
    ))
}
