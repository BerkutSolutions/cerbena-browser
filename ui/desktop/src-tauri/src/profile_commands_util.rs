use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

use browser_engine::{EngineDownloadProgress, EngineInstallation, EngineKind, EngineRuntime};
use browser_profile::{Engine, ProfileMetadata};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use uuid::Uuid;

use crate::{launcher_commands::load_global_security_record, state::AppState};

use super::ERR_CHROMIUM_PROFILE_CERTIFICATES_UNSUPPORTED;

pub(crate) fn parse_nullable_string_field(
    raw: NullableStringField,
    _field_name: &str,
) -> Result<Option<Option<String>>, String> {
    match raw {
        NullableStringField::Missing => Ok(None),
        NullableStringField::Null => Ok(Some(None)),
        NullableStringField::String(text) => Ok(Some(Some(text))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum NullableStringField {
    #[default]
    Missing,
    Null,
    String(String),
}

impl<'de> Deserialize<'de> for NullableStringField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(Self::Null);
        }
        match value {
            serde_json::Value::String(text) => Ok(Self::String(text)),
            other => Err(serde::de::Error::custom(format!(
                "expected string or null, got {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LockedAppPolicyRecord {
    pub start_url: String,
    pub allowed_hosts: Vec<String>,
}

fn tags_request_isolated_certificates(tags: &[String]) -> bool {
    tags.iter().any(|tag| {
        tag.strip_prefix("cert-id:")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
            || tag
                .strip_prefix("cert:")
                .map(|value| value != "global" && !value.trim().is_empty())
                .unwrap_or(false)
    })
}

fn has_global_isolated_certificates(state: &State<'_, AppState>) -> bool {
    load_global_security_record(state)
        .map(|record| {
            record
                .certificates
                .into_iter()
                .any(|item| item.apply_globally && !item.path.trim().is_empty())
        })
        .unwrap_or(false)
}

fn profile_uses_isolated_certificates(
    state: &State<'_, AppState>,
    profile_id: Option<Uuid>,
    tags: &[String],
) -> bool {
    if let Some(profile_id) = profile_id {
        has_global_isolated_certificates(state)
            || tags_request_isolated_certificates(tags)
            || load_global_security_record(state)
                .map(|record| {
                    record.certificates.into_iter().any(|item| {
                        item.profile_ids
                            .iter()
                            .any(|value| value == &profile_id.to_string())
                            && !item.path.trim().is_empty()
                    })
                })
                .unwrap_or(false)
    } else {
        tags_request_isolated_certificates(tags) || has_global_isolated_certificates(state)
    }
}

pub(crate) fn ensure_engine_supports_isolated_certificates(
    state: &State<'_, AppState>,
    profile_id: Option<Uuid>,
    engine: &Engine,
    tags: &[String],
) -> Result<(), String> {
    if engine.is_chromium_family() && profile_uses_isolated_certificates(state, profile_id, tags) {
        return Err(ERR_CHROMIUM_PROFILE_CERTIFICATES_UNSUPPORTED.to_string());
    }
    Ok(())
}

pub(crate) fn parse_engine(engine: &str) -> Result<Engine, String> {
    match engine {
        "chromium" => Ok(Engine::Chromium),
        "ungoogled-chromium" | "ungoogled_chromium" => Ok(Engine::UngoogledChromium),
        "firefox-esr" | "firefox_esr" | "firefox" => Ok(Engine::FirefoxEsr),
        "librewolf" => Ok(Engine::Librewolf),
        _ => Err(format!("unsupported engine: {engine}")),
    }
}

pub(crate) fn engine_session_key(engine: &Engine) -> &'static str {
    engine.as_key()
}

pub(crate) fn engine_kind(engine: Engine) -> EngineKind {
    match engine {
        Engine::Chromium => EngineKind::Chromium,
        Engine::UngoogledChromium => EngineKind::UngoogledChromium,
        Engine::FirefoxEsr => EngineKind::FirefoxEsr,
        Engine::Librewolf => EngineKind::Librewolf,
    }
}

pub(crate) fn open_url_in_running_profile(
    state: &AppState,
    profile: &ProfileMetadata,
    profile_root: &Path,
    launch_url: &str,
) -> Result<(), String> {
    let runtime =
        EngineRuntime::new(state.engine_runtime_root.clone()).map_err(|e| e.to_string())?;
    runtime
        .open_url_in_existing_profile(
            engine_kind(profile.engine.clone()),
            profile_root.to_path_buf(),
            launch_url.trim().to_string(),
        )
        .map_err(|e| e.to_string())
}

pub(crate) async fn ensure_engine_ready(
    app_handle: &tauri::AppHandle,
    state: &AppState,
    runtime: &EngineRuntime,
    engine: EngineKind,
) -> Result<EngineInstallation, String> {
    let key = engine.as_key().to_string();
    loop {
        let started_here = {
            if let Ok(mut cancelled) = state.cancelled_engine_downloads.lock() {
                cancelled.remove(&key);
            }
            let mut active = state
                .active_engine_downloads
                .lock()
                .map_err(|_| "engine download lock poisoned".to_string())?;
            if active.contains(&key) {
                false
            } else {
                active.insert(key.clone());
                true
            }
        };

        if started_here {
            let app_handle = app_handle.clone();
            let progress_handle = app_handle.clone();
            let runtime = runtime.clone();
            let cancel_state = state.cancelled_engine_downloads.clone();
            let key_for_cancel = key.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                runtime.ensure_ready(
                    engine,
                    |progress| {
                        let _ = progress_handle.emit("engine-download-progress", progress);
                    },
                    || {
                        cancel_state
                            .lock()
                            .map(|cancelled| cancelled.contains(&key_for_cancel))
                            .unwrap_or(false)
                    },
                )
            })
            .await
            .map_err(|e| e.to_string())?;
            let mut active = state
                .active_engine_downloads
                .lock()
                .map_err(|_| "engine download lock poisoned".to_string())?;
            active.remove(&key);
            if let Err(error) = &result {
                let is_cancelled = error
                    .to_string()
                    .to_lowercase()
                    .contains("interrupted by user");
                let _ = app_handle.emit(
                    "engine-download-progress",
                    EngineDownloadProgress {
                        engine,
                        version: "pending".to_string(),
                        stage: if is_cancelled {
                            "cancelled".to_string()
                        } else {
                            "error".to_string()
                        },
                        host: None,
                        downloaded_bytes: 0,
                        total_bytes: None,
                        percentage: 0.0,
                        speed_bytes_per_sec: 0.0,
                        eta_seconds: None,
                        message: Some(if is_cancelled {
                            "Download interrupted by user.".to_string()
                        } else {
                            error.to_string()
                        }),
                    },
                );
            }
            return result.map_err(|e| e.to_string());
        }

        let wait_started = Instant::now();
        let timeout = Duration::from_secs(90);
        loop {
            thread::sleep(Duration::from_millis(150));
            let (ready, owner_missing) = {
                let active = state
                    .active_engine_downloads
                    .lock()
                    .map_err(|_| "engine download lock poisoned".to_string())?;
                (!active.contains(&key), active.is_empty())
            };
            if ready {
                break;
            }
            if owner_missing && wait_started.elapsed() > timeout {
                break;
            }
            if wait_started.elapsed() > timeout {
                break;
            }
        }

        if let Some(existing) = runtime.installed(engine).map_err(|e| e.to_string())? {
            return Ok(existing);
        }
    }
}
