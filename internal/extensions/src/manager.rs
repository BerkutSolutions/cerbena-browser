use std::collections::BTreeMap;

use thiserror::Error;
use uuid::Uuid;

use crate::{
    audit::{ExtensionAuditEntry, ExtensionAuditLog},
    import_sources::{ImportSource, SourceValidationError, SourceValidator},
    model::{
        ExtensionImportState, ExtensionRecord, ExtensionStatus, ExtensionUpdatePolicy,
        ProfileExtensionState,
    },
};

#[derive(Debug, Error)]
pub enum ExtensionManagerError {
    #[error("profile extension state not found")]
    ProfileNotFound,
    #[error("extension not found")]
    ExtensionNotFound,
    #[error("duplicate extension id")]
    DuplicateExtensionId,
    #[error("source validation failed: {0}")]
    SourceValidation(#[from] SourceValidationError),
}

#[derive(Debug, Default)]
pub struct ExtensionManager {
    states: BTreeMap<Uuid, ProfileExtensionState>,
    source_validator: SourceValidator,
    audit_log: ExtensionAuditLog,
}

impl ExtensionManager {
    pub fn create_profile_state(&mut self, profile_id: Uuid) {
        self.states
            .entry(profile_id)
            .or_insert_with(|| ProfileExtensionState {
                profile_id,
                extensions: Vec::new(),
            });
    }

    pub fn profile_state(
        &self,
        profile_id: Uuid,
    ) -> Result<&ProfileExtensionState, ExtensionManagerError> {
        self.states
            .get(&profile_id)
            .ok_or(ExtensionManagerError::ProfileNotFound)
    }

    pub fn install(
        &mut self,
        profile_id: Uuid,
        extension_id: &str,
        display_name: &str,
        version: &str,
        source: ImportSource,
        package_path: &str,
        update_policy: ExtensionUpdatePolicy,
    ) -> Result<(), ExtensionManagerError> {
        self.source_validator.validate(&source)?;
        let state = self
            .states
            .get_mut(&profile_id)
            .ok_or(ExtensionManagerError::ProfileNotFound)?;
        if state
            .extensions
            .iter()
            .any(|e| e.extension_id.eq_ignore_ascii_case(extension_id))
        {
            return Err(ExtensionManagerError::DuplicateExtensionId);
        }
        state.extensions.push(ExtensionRecord {
            profile_id,
            extension_id: extension_id.to_string(),
            display_name: display_name.to_string(),
            version: version.to_string(),
            source: source.value,
            package_path: package_path.to_string(),
            status: ExtensionStatus::PendingFirstLaunchInstall,
            import_state: ExtensionImportState::Pending,
            update_policy,
            first_launch_attempts: 0,
            diagnostics: Vec::new(),
        });
        self.audit_log.push(ExtensionAuditEntry {
            profile_id,
            extension_id: extension_id.to_string(),
            action: "extension.install".to_string(),
            outcome: "pending_first_launch".to_string(),
            details: Some("queued for first-launch install".to_string()),
        });
        Ok(())
    }

    pub fn enable(
        &mut self,
        profile_id: Uuid,
        extension_id: &str,
    ) -> Result<(), ExtensionManagerError> {
        let ext = self.find_extension_mut(profile_id, extension_id)?;
        ext.status = ExtensionStatus::Enabled;
        self.audit_log.push(ExtensionAuditEntry {
            profile_id,
            extension_id: extension_id.to_string(),
            action: "extension.enable".to_string(),
            outcome: "success".to_string(),
            details: None,
        });
        Ok(())
    }

    pub fn disable(
        &mut self,
        profile_id: Uuid,
        extension_id: &str,
    ) -> Result<(), ExtensionManagerError> {
        let ext = self.find_extension_mut(profile_id, extension_id)?;
        ext.status = ExtensionStatus::Disabled;
        self.audit_log.push(ExtensionAuditEntry {
            profile_id,
            extension_id: extension_id.to_string(),
            action: "extension.disable".to_string(),
            outcome: "success".to_string(),
            details: None,
        });
        Ok(())
    }

    pub fn update(
        &mut self,
        profile_id: Uuid,
        extension_id: &str,
        new_version: &str,
        diagnostics: Option<&str>,
    ) -> Result<(), ExtensionManagerError> {
        let ext = self.find_extension_mut(profile_id, extension_id)?;
        ext.version = new_version.to_string();
        if let Some(item) = diagnostics {
            ext.diagnostics.push(item.to_string());
        }
        self.audit_log.push(ExtensionAuditEntry {
            profile_id,
            extension_id: extension_id.to_string(),
            action: "extension.update".to_string(),
            outcome: "success".to_string(),
            details: Some(format!("version={}", new_version)),
        });
        Ok(())
    }

    pub fn diagnostics(
        &self,
        profile_id: Uuid,
        extension_id: &str,
    ) -> Result<Vec<String>, ExtensionManagerError> {
        let state = self
            .states
            .get(&profile_id)
            .ok_or(ExtensionManagerError::ProfileNotFound)?;
        let ext = state
            .extensions
            .iter()
            .find(|e| e.extension_id.eq_ignore_ascii_case(extension_id))
            .ok_or(ExtensionManagerError::ExtensionNotFound)?;
        Ok(ext.diagnostics.clone())
    }

    pub fn audit_entries(&self) -> &[ExtensionAuditEntry] {
        self.audit_log.entries()
    }

    fn find_extension_mut(
        &mut self,
        profile_id: Uuid,
        extension_id: &str,
    ) -> Result<&mut ExtensionRecord, ExtensionManagerError> {
        let state = self
            .states
            .get_mut(&profile_id)
            .ok_or(ExtensionManagerError::ProfileNotFound)?;
        state
            .extensions
            .iter_mut()
            .find(|e| e.extension_id.eq_ignore_ascii_case(extension_id))
            .ok_or(ExtensionManagerError::ExtensionNotFound)
    }
}
