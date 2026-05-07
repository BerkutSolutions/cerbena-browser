use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    artifact::{download_with_curl, install_binary, verify_sha256, ArtifactSpec},
    contract::{EngineAdapter, EngineError, EngineKind, LaunchPlan, LaunchRequest},
};

#[derive(Debug, Clone)]
pub struct UngoogledChromiumAdapter {
    pub install_root: PathBuf,
    pub cache_dir: PathBuf,
}

impl EngineAdapter for UngoogledChromiumAdapter {
    fn engine_kind(&self) -> EngineKind {
        EngineKind::UngoogledChromium
    }

    fn prepare(&self, spec: &ArtifactSpec) -> Result<PathBuf, EngineError> {
        let path = download_with_curl(spec, &self.cache_dir)?;
        verify_sha256(&path, &spec.sha256_hex)?;
        Ok(path)
    }

    fn install(&self, downloaded_path: &Path) -> Result<PathBuf, EngineError> {
        install_binary(
            downloaded_path,
            &self.install_root,
            "ungoogled-chromium",
            "current",
        )
    }

    fn build_launch_plan(&self, request: LaunchRequest) -> Result<LaunchPlan, EngineError> {
        Ok(LaunchPlan {
            engine: EngineKind::UngoogledChromium,
            binary_path: request.binary_path,
            args: request.args,
            env: request.env,
            cwd: request.profile_root,
        })
    }

    fn launch(&self, request: LaunchRequest) -> Result<u32, EngineError> {
        let plan = self.build_launch_plan(request)?;
        let child = Command::new(&plan.binary_path)
            .current_dir(&plan.cwd)
            .args(&plan.args)
            .envs(plan.env.iter().map(|(key, value)| (key, value)))
            .spawn()
            .map_err(|e| EngineError::Launch(format!("spawn failed: {e}")))?;
        Ok(child.id())
    }
}
