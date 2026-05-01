use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::ProfileError;

#[derive(Debug, Clone)]
pub struct AuditLogger {
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: String,
    pub action: String,
    pub actor: String,
    pub profile_id: Uuid,
    pub outcome: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    pub actor: Option<String>,
    pub action_prefix: Option<String>,
    pub profile_id: Option<Uuid>,
    pub outcome: Option<String>,
}

impl AuditLogger {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(Self { path })
    }

    pub fn record(
        &self,
        action: &str,
        actor: &str,
        profile_id: Uuid,
        outcome: &str,
        details: Option<&str>,
    ) -> Result<(), ProfileError> {
        let event = AuditEvent {
            timestamp: crate::manager::utc_now(),
            action: action.to_string(),
            actor: actor.to_string(),
            profile_id,
            outcome: outcome.to_string(),
            details: details.map(|v| v.to_string()),
        };

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(serde_json::to_string(&event)?.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }

    pub fn record_system(
        &self,
        action: &str,
        profile_id: Uuid,
        outcome: &str,
    ) -> Result<(), ProfileError> {
        self.record(action, "system", profile_id, outcome, None)
    }

    pub fn read_events(&self, filter: AuditFilter) -> Result<Vec<AuditEvent>, ProfileError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let mut text = String::new();
        File::open(&self.path)?.read_to_string(&mut text)?;
        let mut out = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let event: AuditEvent = serde_json::from_str(line)?;
            if let Some(actor) = &filter.actor {
                if &event.actor != actor {
                    continue;
                }
            }
            if let Some(prefix) = &filter.action_prefix {
                if !event.action.starts_with(prefix) {
                    continue;
                }
            }
            if let Some(id) = filter.profile_id {
                if event.profile_id != id {
                    continue;
                }
            }
            if let Some(outcome) = &filter.outcome {
                if &event.outcome != outcome {
                    continue;
                }
            }
            out.push(event);
        }
        Ok(out)
    }
}
