use std::{path::Path, process::Command};

fn hidden_powershell(command: &str) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;

    let mut process = Command::new("powershell.exe");
    process.args(["-NoProfile", "-Command", command]);
    process.creation_flags(0x08000000);
    process
        .output()
        .map_err(|error| format!("powershell certificate command failed: {error}"))
}

fn powershell_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

pub fn load_certificate_metadata(path: &Path) -> Result<(Option<String>, Option<String>), String> {
    let literal_path = powershell_single_quote(&path.to_string_lossy());
    let script = format!(
        "$cert = Get-PfxCertificate -FilePath '{literal_path}'; \
if ($null -eq $cert) {{ return }}; \
[pscustomobject]@{{ Subject = $cert.Subject; Issuer = $cert.Issuer }} | ConvertTo-Json -Compress"
    );
    let output = hidden_powershell(&script)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("failed to read certificate metadata {}", path.display())
        } else {
            format!(
                "failed to read certificate metadata {}: {stderr}",
                path.display()
            )
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok((None, None));
    }
    let value = serde_json::from_str::<serde_json::Value>(&stdout)
        .map_err(|error| format!("certificate metadata parse failed: {error}"))?;
    let subject = value
        .get("Subject")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let issuer = value
        .get("Issuer")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    Ok((subject, issuer))
}
