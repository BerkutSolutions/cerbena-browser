use serde::{Deserialize, Serialize};

use crate::contract::EngineKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineDownloadProgress {
    pub engine: EngineKind,
    pub version: String,
    pub stage: String,
    pub host: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percentage: f64,
    pub speed_bytes_per_sec: f64,
    pub eta_seconds: Option<f64>,
    pub message: Option<String>,
}

impl EngineDownloadProgress {
    pub fn stage(engine: EngineKind, version: impl Into<String>, stage: impl Into<String>) -> Self {
        Self {
            engine,
            version: version.into(),
            stage: stage.into(),
            host: None,
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 0.0,
            speed_bytes_per_sec: 0.0,
            eta_seconds: None,
            message: None,
        }
    }
}
