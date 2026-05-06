use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EngineKind {
    Wayfern,
    Librewolf,
}

#[derive(Debug, Clone)]
pub struct LaunchRequest {
    pub profile_id: Uuid,
    pub profile_root: PathBuf,
    pub binary_path: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchPlan {
    pub engine: EngineKind,
    pub binary_path: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("artifact validation failed: {0}")]
    Validation(String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("install failed: {0}")]
    Install(String),
    #[error("launch blocked: {0}")]
    LaunchBlocked(String),
    #[error("launch failed: {0}")]
    Launch(String),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub trait EngineAdapter {
    fn engine_kind(&self) -> EngineKind;
    fn prepare(&self, spec: &crate::artifact::ArtifactSpec) -> Result<PathBuf, EngineError>;
    fn install(&self, downloaded_path: &std::path::Path) -> Result<PathBuf, EngineError>;
    fn build_launch_plan(&self, request: LaunchRequest) -> Result<LaunchPlan, EngineError>;
    fn launch(&self, request: LaunchRequest) -> Result<u32, EngineError>;
}
