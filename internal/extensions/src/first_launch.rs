use crate::model::{ExtensionImportState, ExtensionStatus, ProfileExtensionState};

#[derive(Debug, Clone)]
pub struct ExtensionInstallResult {
    pub extension_id: String,
    pub installed: bool,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FirstLaunchInstaller {
    pub max_attempts: u8,
}

impl FirstLaunchInstaller {
    pub fn process(
        &self,
        state: &mut ProfileExtensionState,
        results: &[ExtensionInstallResult],
    ) -> Vec<String> {
        let mut events = Vec::new();
        let max_attempts = if self.max_attempts == 0 {
            3
        } else {
            self.max_attempts
        };

        for item in results {
            let Some(ext) = state
                .extensions
                .iter_mut()
                .find(|e| e.extension_id.eq_ignore_ascii_case(&item.extension_id))
            else {
                continue;
            };
            ext.first_launch_attempts = ext.first_launch_attempts.saturating_add(1);
            if item.installed {
                ext.status = ExtensionStatus::Installed;
                ext.import_state = ExtensionImportState::Installed;
                events.push(format!(
                    "extension.first_launch_install.ok:{}",
                    ext.extension_id
                ));
                continue;
            }

            let reason = item
                .details
                .clone()
                .unwrap_or_else(|| "unknown error".to_string());
            if ext.first_launch_attempts >= max_attempts {
                ext.status = ExtensionStatus::Failed;
                ext.import_state = ExtensionImportState::Failed;
                ext.diagnostics
                    .push(format!("first-launch install failed: {}", reason));
                events.push(format!(
                    "extension.first_launch_install.failed:{}",
                    ext.extension_id
                ));
            } else {
                ext.status = ExtensionStatus::PendingFirstLaunchInstall;
                ext.import_state = ExtensionImportState::Pending;
                ext.diagnostics
                    .push(format!("first-launch retry scheduled: {}", reason));
                events.push(format!(
                    "extension.first_launch_install.retry:{}",
                    ext.extension_id
                ));
            }
        }

        events
    }
}
