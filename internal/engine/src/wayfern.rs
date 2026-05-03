use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    artifact::{download_with_curl, install_binary, verify_sha256, ArtifactSpec},
    contract::{EngineAdapter, EngineError, EngineKind, LaunchPlan, LaunchRequest},
};

#[derive(Debug, Clone)]
pub struct WayfernAdapter {
    pub install_root: PathBuf,
    pub cache_dir: PathBuf,
    pub tos_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct WayfernTosAck {
    tos_version: String,
    accepted_at_epoch: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct WayfernRuntimeAcceptance {
    tos_version: String,
    engine_version: String,
    accepted_at_epoch: u64,
}

impl WayfernAdapter {
    fn legacy_tos_ack_path(profile_root: &Path) -> PathBuf {
        profile_root.join("policy").join("wayfern_tos_ack.json")
    }

    pub fn tos_ack_path(profile_root: &Path) -> PathBuf {
        let Some(parent) = profile_root.parent() else {
            return Self::legacy_tos_ack_path(profile_root);
        };
        let launcher_root = if parent
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("profiles"))
        {
            parent.parent().unwrap_or(parent)
        } else {
            parent
        };
        launcher_root.join("wayfern_tos_ack.json")
    }

    fn runtime_acceptance_path(profile_root: &Path) -> PathBuf {
        let Some(parent) = profile_root.parent() else {
            return profile_root
                .join("tmp")
                .join("wayfern_runtime_acceptance.json");
        };
        let launcher_root = if parent
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("profiles"))
        {
            parent.parent().unwrap_or(parent)
        } else {
            parent
        };
        launcher_root.join("wayfern_runtime_acceptance.json")
    }

    pub fn acknowledge_tos(
        &self,
        profile_root: &Path,
        profile_id: Uuid,
    ) -> Result<(), EngineError> {
        let _ = profile_id;
        let ack = WayfernTosAck {
            tos_version: self.tos_version.clone(),
            accepted_at_epoch: now_epoch(),
        };
        let path = Self::tos_ack_path(profile_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_vec_pretty(&ack)?)?;
        persist_global_license_acceptance(ack.accepted_at_epoch, None)?;
        let legacy_path = Self::legacy_tos_ack_path(profile_root);
        if legacy_path != path && legacy_path.exists() {
            let _ = fs::remove_file(legacy_path);
        }
        Ok(())
    }

    pub fn is_tos_acknowledged(
        &self,
        profile_root: &Path,
        profile_id: Uuid,
    ) -> Result<bool, EngineError> {
        let _ = profile_id;
        let path = Self::tos_ack_path(profile_root);
        if !path.exists() {
            return Ok(false);
        }
        let bytes = fs::read(path)?;
        let ack: WayfernTosAck = serde_json::from_slice(&bytes)?;
        Ok(ack.tos_version == self.tos_version)
    }

    fn ensure_tos_ack(&self, profile_root: &Path, profile_id: Uuid) -> Result<(), EngineError> {
        let path = Self::tos_ack_path(profile_root);
        if !self.is_tos_acknowledged(profile_root, profile_id)? {
            return Err(EngineError::LaunchBlocked(
                if path.exists() || Self::legacy_tos_ack_path(profile_root).exists() {
                    "wayfern_terms_ack_stale".to_string()
                } else {
                    "wayfern_terms_not_acknowledged".to_string()
                },
            ));
        }
        let bytes = fs::read(&path)?;
        let ack: WayfernTosAck = serde_json::from_slice(&bytes)?;
        persist_global_license_acceptance(ack.accepted_at_epoch, None)?;
        Ok(())
    }

    pub fn finalize_runtime_acceptance(
        &self,
        profile_root: &Path,
        profile_id: Uuid,
        binary_path: &Path,
        engine_version: &str,
    ) -> Result<(), EngineError> {
        self.ensure_tos_ack(profile_root, profile_id)?;
        if !needs_runtime_acceptance(profile_root, &self.tos_version, engine_version)? {
            return Ok(());
        }

        let ack_path = Self::tos_ack_path(profile_root);
        let bytes = fs::read(&ack_path)?;
        let ack: WayfernTosAck = serde_json::from_slice(&bytes)?;

        let log_dir = profile_root.join("tmp");
        fs::create_dir_all(&log_dir)?;
        let acceptance_profile = log_dir.join("wayfern-acceptance-profile");
        fs::create_dir_all(&acceptance_profile)?;
        let stdout_log = log_dir.join("wayfern-stdout.log");
        let stderr_log = log_dir.join("wayfern-stderr.log");
        let stdout = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stdout_log)?;
        let stderr = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_log)?;

        let mut command = Command::new(binary_path);
        command
            .current_dir(binary_path.parent().unwrap_or(profile_root))
            .arg(format!(
                "--user-data-dir={}",
                acceptance_profile.to_string_lossy()
            ))
            .arg("--accept-terms-and-conditions")
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let status = command.status().map_err(|e| {
            EngineError::Launch(format!("wayfern acceptance preflight failed: {e}"))
        })?;
        if !status.success() {
            return Err(EngineError::Launch(format!(
                "wayfern acceptance preflight exited with status {:?}",
                status.code()
            )));
        }

        persist_global_license_acceptance(ack.accepted_at_epoch, None)?;
        let runtime_acceptance = WayfernRuntimeAcceptance {
            tos_version: self.tos_version.clone(),
            engine_version: engine_version.to_string(),
            accepted_at_epoch: ack.accepted_at_epoch,
        };
        let path = Self::runtime_acceptance_path(profile_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(&runtime_acceptance)?)?;
        Ok(())
    }
}

impl EngineAdapter for WayfernAdapter {
    fn engine_kind(&self) -> EngineKind {
        EngineKind::Wayfern
    }

    fn prepare(&self, spec: &ArtifactSpec) -> Result<PathBuf, EngineError> {
        let path = download_with_curl(spec, &self.cache_dir)?;
        verify_sha256(&path, &spec.sha256_hex)?;
        Ok(path)
    }

    fn install(&self, downloaded_path: &Path) -> Result<PathBuf, EngineError> {
        install_binary(downloaded_path, &self.install_root, "wayfern", "current")
    }

    fn build_launch_plan(&self, request: LaunchRequest) -> Result<LaunchPlan, EngineError> {
        self.ensure_tos_ack(&request.profile_root, request.profile_id)?;
        let cwd = request
            .binary_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| request.profile_root.clone());
        Ok(LaunchPlan {
            engine: EngineKind::Wayfern,
            binary_path: request.binary_path.clone(),
            args: request.args.clone(),
            cwd,
        })
    }

    fn launch(&self, request: LaunchRequest) -> Result<u32, EngineError> {
        let log_dir = request.profile_root.join("tmp");
        let _ = fs::create_dir_all(&log_dir);
        let stdout_log = log_dir.join("wayfern-stdout.log");
        let stderr_log = log_dir.join("wayfern-stderr.log");
        let stdout = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stdout_log)?;
        let stderr = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_log)?;
        let plan = self.build_launch_plan(request)?;
        let child = Command::new(&plan.binary_path)
            .current_dir(&plan.cwd)
            .args(&plan.args)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|e| EngineError::Launch(format!("spawn failed: {e}")))?;
        Ok(child.id())
    }
}

fn needs_runtime_acceptance(
    profile_root: &Path,
    tos_version: &str,
    engine_version: &str,
) -> Result<bool, EngineError> {
    let runtime_path = WayfernAdapter::runtime_acceptance_path(profile_root);
    if !runtime_path.exists() {
        return Ok(true);
    }
    let bytes = fs::read(runtime_path)?;
    let acceptance: WayfernRuntimeAcceptance = serde_json::from_slice(&bytes)?;
    Ok(acceptance.tos_version != tos_version || acceptance.engine_version != engine_version)
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn persist_global_license_acceptance(
    accepted_at_epoch: u64,
    appdata_override: Option<&Path>,
) -> Result<(), EngineError> {
    let Some(path) = global_license_acceptance_path(appdata_override) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, accepted_at_epoch.to_string())?;
    Ok(())
}

fn global_license_acceptance_path(appdata_override: Option<&Path>) -> Option<PathBuf> {
    let appdata = appdata_override
        .map(Path::to_path_buf)
        .or_else(|| env::var_os("APPDATA").map(PathBuf::from))?;
    Some(appdata.join("Wayfern").join("license-accepted"))
}

#[cfg(test)]
mod tests {
    use super::{
        global_license_acceptance_path, needs_runtime_acceptance,
        persist_global_license_acceptance, WayfernRuntimeAcceptance,
    };
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn persists_global_license_marker_under_appdata() {
        let temp = tempdir().expect("tempdir");
        persist_global_license_acceptance(12345, Some(temp.path())).expect("persist marker");
        let path =
            global_license_acceptance_path(Some(temp.path())).expect("global marker path exists");
        assert_eq!(fs::read_to_string(path).expect("read marker"), "12345");
    }

    #[test]
    fn runtime_acceptance_is_not_needed_for_matching_marker() {
        let temp = tempdir().expect("tempdir");
        let profile_root = temp.path().join("profiles").join("profile");
        fs::create_dir_all(&profile_root).expect("profile dir");
        let marker_path = super::WayfernAdapter::runtime_acceptance_path(&profile_root);
        fs::create_dir_all(marker_path.parent().expect("marker parent")).expect("marker parent");
        let marker = WayfernRuntimeAcceptance {
            tos_version: "2026-04".to_string(),
            engine_version: "146.0.7680.166".to_string(),
            accepted_at_epoch: 12345,
        };
        fs::write(
            &marker_path,
            serde_json::to_vec_pretty(&marker).expect("encode marker"),
        )
        .expect("write marker");

        let needed =
            needs_runtime_acceptance(&profile_root, "2026-04", "146.0.7680.166").expect("check");
        assert!(!needed);
    }
}
