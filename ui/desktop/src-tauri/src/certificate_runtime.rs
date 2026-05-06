use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(target_os = "windows")]
use std::process::Command;

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    launcher_commands::load_global_security_record,
    state::AppState,
};

const LIBREWOLF_PROFILE_CERTIFICATES_DIR: &str = "librewolf-certificates";
const SUPPORTED_CERTIFICATE_EXTENSIONS: &[&str] = &["pem", "crt", "cer"];

pub fn clear_librewolf_profile_certificates(app_handle: &AppHandle, profile_id: Uuid) {
    let state = app_handle.state::<AppState>();
    let _ = clear_librewolf_profile_certificates_for_state(state.inner(), profile_id);
}

pub fn clear_librewolf_profile_certificates_for_state(
    state: &AppState,
    profile_id: Uuid,
) -> Result<(), String> {
    let root = profile_certificate_runtime_dir(state, profile_id);
    if root.exists() {
        fs::remove_dir_all(&root)
            .map_err(|error| format!("remove LibreWolf profile certificates {}: {error}", root.display()))?;
    }
    Ok(())
}

pub fn prepare_librewolf_profile_certificates_for_state(
    state: &AppState,
    profile_id: Uuid,
    tags: &[String],
) -> Result<Vec<String>, String> {
    let certificate_paths = resolve_profile_certificate_paths(state, profile_id, tags);
    if certificate_paths.is_empty() {
        clear_librewolf_profile_certificates_for_state(state, profile_id)?;
        return Ok(Vec::new());
    }

    let runtime_dir = profile_certificate_runtime_dir(state, profile_id);
    if runtime_dir.exists() {
        fs::remove_dir_all(&runtime_dir)
            .map_err(|error| format!("reset LibreWolf certificate runtime {}: {error}", runtime_dir.display()))?;
    }
    fs::create_dir_all(&runtime_dir)
        .map_err(|error| format!("create LibreWolf certificate runtime {}: {error}", runtime_dir.display()))?;

    let mut materialized = Vec::new();
    for source_path in certificate_paths {
        let source = PathBuf::from(source_path.trim());
        validate_certificate_source(&source)?;
        let destination = runtime_dir.join(materialized_certificate_name(&source)?);
        fs::copy(&source, &destination).map_err(|error| {
            format!(
                "copy certificate {} into {}: {error}",
                source.display(),
                destination.display()
            )
        })?;
        materialized.push(destination.to_string_lossy().to_string());
    }

    materialized.sort();
    materialized.dedup();
    Ok(materialized)
}

fn profile_certificate_runtime_dir(state: &AppState, profile_id: Uuid) -> PathBuf {
    state
        .profile_root
        .join(profile_id.to_string())
        .join("policy")
        .join(LIBREWOLF_PROFILE_CERTIFICATES_DIR)
}

fn resolve_profile_certificate_paths(
    state: &AppState,
    profile_id: Uuid,
    tags: &[String],
) -> Vec<String> {
    let selected_ids = tags
        .iter()
        .filter_map(|tag| tag.strip_prefix("cert-id:").map(|value| value.to_string()))
        .collect::<std::collections::BTreeSet<_>>();
    let mut paths = std::collections::BTreeSet::new();
    if let Ok(record) = load_global_security_record(state) {
        for item in record.certificates {
            let assigned = item
                .profile_ids
                .iter()
                .any(|value| value == &profile_id.to_string());
            if !item.path.trim().is_empty()
                && (item.apply_globally || assigned || selected_ids.contains(&item.id))
            {
                paths.insert(item.path.trim().to_string());
            }
        }
    }
    for tag in tags {
        if let Some(path) = tag.strip_prefix("cert:") {
            if path != "global" && !path.trim().is_empty() {
                paths.insert(path.trim().to_string());
            }
        }
    }
    paths.into_iter().collect()
}

fn validate_certificate_source(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("certificate file not found: {}", path.display()));
    }
    if !path.is_file() {
        return Err(format!("certificate path is not a file: {}", path.display()));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| format!("certificate file has no supported extension: {}", path.display()))?;
    if !SUPPORTED_CERTIFICATE_EXTENSIONS
        .iter()
        .any(|expected| extension == *expected)
    {
        return Err(format!(
            "unsupported certificate format for {} (expected PEM/CRT/CER)",
            path.display()
        ));
    }
    let bytes = fs::read(path).map_err(|error| format!("read certificate {}: {error}", path.display()))?;
    if bytes.is_empty() {
        return Err(format!("certificate file is empty: {}", path.display()));
    }
    if extension == "pem" {
        let text = String::from_utf8(bytes)
            .map_err(|_| format!("PEM certificate is not valid UTF-8 text: {}", path.display()))?;
        if !text.contains("-----BEGIN CERTIFICATE-----") {
            return Err(format!(
                "PEM certificate does not contain BEGIN CERTIFICATE marker: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn materialized_certificate_name(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("read certificate {}: {error}", path.display()))?;
    let digest = Sha256::digest(&bytes);
    let short_hash = format!("{:x}", digest);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| format!("certificate file has no extension: {}", path.display()))?;
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_certificate_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "certificate".to_string());
    Ok(format!("{}-{}.{}", stem, &short_hash[..16], extension))
}

fn sanitize_certificate_name(value: &str) -> String {
    value.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

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

#[cfg(test)]
mod tests {
    use super::{materialized_certificate_name, sanitize_certificate_name, validate_certificate_source};
    use std::fs;

    #[test]
    fn validates_pem_certificate_marker() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("cert.pem");
        fs::write(&path, "-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----\n")
            .expect("write pem");
        validate_certificate_source(&path).expect("validate pem");
    }

    #[test]
    fn rejects_unknown_certificate_extension() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("cert.txt");
        fs::write(&path, "-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----\n")
            .expect("write cert");
        assert!(validate_certificate_source(&path).is_err());
    }

    #[test]
    fn materialized_name_is_stable_and_sanitized() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("My Cert!.cer");
        fs::write(&path, b"\x01\x02\x03").expect("write cert");
        let name = materialized_certificate_name(&path).expect("name");
        assert!(name.starts_with("My-Cert-"));
        assert!(name.ends_with(".cer"));
        assert_eq!(sanitize_certificate_name("A B/C"), "A-B-C");
    }
}
