use serde::{Deserialize, Serialize};

use crate::{
    consistency::{validate_consistency, ConsistencyIssue, ConsistencyLevel},
    model::{IdentityPreset, IdentityPresetMode},
    validate::validate_identity_preset,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityPreview {
    pub mode: IdentityPresetMode,
    pub payload_json: String,
    pub blocking_issues: Vec<ConsistencyIssue>,
    pub warning_issues: Vec<ConsistencyIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityTabSaveOutcome {
    pub allowed_to_save: bool,
    pub validation_error: Option<String>,
    pub preview: IdentityPreview,
}

pub fn build_identity_preview(
    preset: &IdentityPreset,
    active_route: Option<&str>,
) -> Result<IdentityPreview, String> {
    let payload_json =
        serde_json::to_string_pretty(preset).map_err(|err| format!("serialize:{}", err))?;
    let issues = validate_consistency(preset, active_route);
    let mut blocking_issues = Vec::new();
    let mut warning_issues = Vec::new();
    for issue in issues {
        match issue.level {
            ConsistencyLevel::Blocking => blocking_issues.push(issue),
            ConsistencyLevel::Warning => warning_issues.push(issue),
        }
    }
    Ok(IdentityPreview {
        mode: preset.mode,
        payload_json,
        blocking_issues,
        warning_issues,
    })
}

pub fn validate_identity_tab_save(
    preset: &IdentityPreset,
    active_route: Option<&str>,
) -> IdentityTabSaveOutcome {
    let validation_error = validate_identity_preset(preset)
        .err()
        .map(|e| e.to_string());
    let fallback_preview = IdentityPreview {
        mode: preset.mode,
        payload_json: "{}".to_string(),
        blocking_issues: Vec::new(),
        warning_issues: Vec::new(),
    };
    let preview = build_identity_preview(preset, active_route).unwrap_or(fallback_preview);
    let has_blocking = !preview.blocking_issues.is_empty();

    IdentityTabSaveOutcome {
        allowed_to_save: validation_error.is_none() && !has_blocking,
        validation_error,
        preview,
    }
}
