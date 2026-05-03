use std::path::Path;
#[cfg(target_os = "windows")]
use std::process::Command;

use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::state::AppState;

#[cfg(target_os = "windows")]
fn hidden_powershell(command: &str) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;

    let mut process = Command::new("powershell.exe");
    process.args(["-NoProfile", "-Command", command]);
    process.creation_flags(0x08000000);
    process
        .output()
        .map_err(|error| format!("powershell certificate command failed: {error}"))
}

#[cfg(target_os = "windows")]
fn powershell_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

pub fn clear_wayfern_profile_certificates(app_handle: &AppHandle, profile_id: Uuid) {
    let state = app_handle.state::<AppState>();
    let _ = clear_wayfern_profile_certificates_for_state(state.inner(), profile_id);
}

pub fn clear_wayfern_profile_certificates_for_state(
    state: &AppState,
    profile_id: Uuid,
) -> Result<(), String> {
    let _ = (state, profile_id);
    Ok(())
}

#[cfg(target_os = "windows")]
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

#[cfg(not(target_os = "windows"))]
pub fn load_certificate_metadata(path: &Path) -> Result<(Option<String>, Option<String>), String> {
    let _ = path;
    Ok((None, None))
}

pub fn display_certificate_issuer(
    issuer_name: Option<String>,
    subject_name: Option<String>,
) -> Option<String> {
    issuer_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            subject_name
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}
