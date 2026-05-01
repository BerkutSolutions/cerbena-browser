use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
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
    profile_id: Uuid,
    tos_version: String,
    accepted_at_epoch: u64,
}

impl WayfernAdapter {
    pub fn tos_ack_path(profile_root: &Path) -> PathBuf {
        profile_root.join("policy").join("wayfern_tos_ack.json")
    }

    pub fn acknowledge_tos(
        &self,
        profile_root: &Path,
        profile_id: Uuid,
    ) -> Result<(), EngineError> {
        let ack = WayfernTosAck {
            profile_id,
            tos_version: self.tos_version.clone(),
            accepted_at_epoch: now_epoch(),
        };
        let path = Self::tos_ack_path(profile_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(&ack)?)?;
        Ok(())
    }

    fn ensure_tos_ack(&self, profile_root: &Path, profile_id: Uuid) -> Result<(), EngineError> {
        let path = Self::tos_ack_path(profile_root);
        if !path.exists() {
            return Err(EngineError::LaunchBlocked(
                "wayfern_terms_not_acknowledged".to_string(),
            ));
        }
        let bytes = fs::read(path)?;
        let ack: WayfernTosAck = serde_json::from_slice(&bytes)?;
        if ack.profile_id != profile_id || ack.tos_version != self.tos_version {
            return Err(EngineError::LaunchBlocked(
                "wayfern_terms_ack_stale".to_string(),
            ));
        }
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
        Ok(LaunchPlan {
            engine: EngineKind::Wayfern,
            binary_path: request.binary_path.clone(),
            args: request.args.clone(),
            cwd: request.profile_root,
        })
    }

    fn launch(&self, request: LaunchRequest) -> Result<u32, EngineError> {
        let plan = self.build_launch_plan(request)?;
        let child = Command::new(&plan.binary_path)
            .current_dir(&plan.cwd)
            .args(&plan.args)
            .spawn()
            .map_err(|e| EngineError::Launch(format!("spawn failed: {e}")))?;
        Ok(child.id())
    }
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
