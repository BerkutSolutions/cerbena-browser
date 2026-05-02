#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::contract::EngineError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSpec {
    pub url: String,
    pub file_name: String,
    pub sha256_hex: String,
    pub version: String,
}

pub fn download_with_curl(spec: &ArtifactSpec, cache_dir: &Path) -> Result<PathBuf, EngineError> {
    fs::create_dir_all(cache_dir)?;
    let out_path = cache_dir.join(&spec.file_name);
    let mut command = Command::new("curl");
    command
        .arg("-L")
        .arg("--fail")
        .arg("--output")
        .arg(&out_path)
        .arg(&spec.url);
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(0x08000000);
    }
    let status = command
        .status()
        .map_err(|e| EngineError::Download(format!("curl unavailable: {e}")))?;
    if !status.success() {
        return Err(EngineError::Download(format!(
            "curl exited with status: {status}"
        )));
    }
    Ok(out_path)
}

pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), EngineError> {
    let data = fs::read(path)?;
    let hash = Sha256::digest(&data);
    let actual = hex_lower(&hash);
    if actual != expected_hex.to_lowercase() {
        return Err(EngineError::Validation(format!(
            "checksum mismatch: expected {expected_hex}, got {actual}"
        )));
    }
    Ok(())
}

pub fn install_binary(
    artifact_path: &Path,
    install_root: &Path,
    engine_dir: &str,
    version: &str,
) -> Result<PathBuf, EngineError> {
    let target_dir = install_root.join(engine_dir).join(version);
    fs::create_dir_all(&target_dir)?;
    let file_name = artifact_path
        .file_name()
        .ok_or_else(|| EngineError::Install("artifact file name is missing".to_string()))?;
    let target = target_dir.join(file_name);
    fs::copy(artifact_path, &target)?;
    Ok(target)
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{:02x}", b);
    }
    s
}
