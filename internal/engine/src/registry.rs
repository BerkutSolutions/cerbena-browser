use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    contract::{EngineError, EngineKind},
    runtime::EngineInstallation,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RegistryState {
    installs: BTreeMap<String, EngineInstallation>,
}

#[derive(Debug, Clone)]
pub struct EngineRegistry {
    path: PathBuf,
}

impl EngineRegistry {
    pub fn new(path: PathBuf) -> Result<Self, EngineError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(Self { path })
    }

    pub fn get(&self, engine: EngineKind) -> Result<Option<EngineInstallation>, EngineError> {
        let state = self.load()?;
        Ok(state.installs.get(engine.as_key()).cloned())
    }

    pub fn put(&self, install: EngineInstallation) -> Result<(), EngineError> {
        let mut state = self.load()?;
        state
            .installs
            .insert(install.engine.as_key().to_string(), install);
        self.save(&state)
    }

    fn load(&self) -> Result<RegistryState, EngineError> {
        if !self.path.exists() {
            return Ok(RegistryState::default());
        }
        let raw = fs::read(&self.path)?;
        Ok(serde_json::from_slice(&raw)?)
    }

    fn save(&self, state: &RegistryState) -> Result<(), EngineError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(state)?;
        fs::write(&self.path, bytes)?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
