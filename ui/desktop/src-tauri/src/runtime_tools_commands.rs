use std::{collections::BTreeSet, path::PathBuf, process::Command};

use browser_engine::{EngineKind, EngineRuntime};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    envelope::{ok, UiEnvelope},
    network_runtime::{
        ensure_network_runtime_tools, resolve_amneziawg_binary_path, resolve_openvpn_binary_path,
        resolve_sing_box_binary_path, resolve_tor_binary_path, NetworkRuntime, NetworkTool,
    },
    network_sandbox_container::probe_container_runtime,
    profile_commands::ensure_engine_ready,
    state::AppState,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRuntimeToolRequest {
    pub tool_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeToolStatusView {
    pub id: String,
    pub name_key: String,
    pub status: String,
    pub version: Option<String>,
    pub action: String,
    pub detail_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeToolId {
    Docker,
    Wayfern,
    Camoufox,
    SingBox,
    OpenVpn,
    AmneziaWg,
    TorBundle,
}

impl RuntimeToolId {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "docker" => Some(Self::Docker),
            "wayfern" => Some(Self::Wayfern),
            "camoufox" => Some(Self::Camoufox),
            "sing-box" => Some(Self::SingBox),
            "openvpn" => Some(Self::OpenVpn),
            "amneziawg" => Some(Self::AmneziaWg),
            "tor-bundle" => Some(Self::TorBundle),
            _ => None,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Wayfern => "wayfern",
            Self::Camoufox => "camoufox",
            Self::SingBox => "sing-box",
            Self::OpenVpn => "openvpn",
            Self::AmneziaWg => "amneziawg",
            Self::TorBundle => "tor-bundle",
        }
    }

    fn name_key(self) -> &'static str {
        match self {
            Self::Docker => "settings.tools.docker",
            Self::Wayfern => "settings.tools.wayfern",
            Self::Camoufox => "settings.tools.camoufox",
            Self::SingBox => "settings.tools.singBox",
            Self::OpenVpn => "settings.tools.openvpn",
            Self::AmneziaWg => "settings.tools.amneziawg",
            Self::TorBundle => "settings.tools.torBundle",
        }
    }
}

#[derive(Debug, Clone)]
struct DockerStatus {
    installed: bool,
    client_version: Option<String>,
    runtime_available: bool,
}

#[tauri::command]
pub fn get_runtime_tools_status(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<RuntimeToolStatusView>>, String> {
    Ok(ok(correlation_id, collect_runtime_tools_status(&state)?))
}

#[tauri::command]
pub async fn install_runtime_tool(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    request: InstallRuntimeToolRequest,
    correlation_id: String,
) -> Result<UiEnvelope<RuntimeToolStatusView>, String> {
    let tool = RuntimeToolId::parse(&request.tool_id)
        .ok_or_else(|| format!("unsupported runtime tool id: {}", request.tool_id))?;
    match tool {
        RuntimeToolId::Docker => return Err("docker must be installed manually".to_string()),
        RuntimeToolId::Wayfern | RuntimeToolId::Camoufox => {
            let engine = if matches!(tool, RuntimeToolId::Wayfern) {
                EngineKind::Wayfern
            } else {
                EngineKind::Camoufox
            };
            let runtime =
                EngineRuntime::new(state.engine_runtime_root.clone()).map_err(|e| e.to_string())?;
            let _ = ensure_engine_ready(&app_handle, &state, &runtime, engine).await?;
        }
        RuntimeToolId::SingBox
        | RuntimeToolId::OpenVpn
        | RuntimeToolId::AmneziaWg
        | RuntimeToolId::TorBundle => {
            let network_tool = match tool {
                RuntimeToolId::SingBox => NetworkTool::SingBox,
                RuntimeToolId::OpenVpn => NetworkTool::OpenVpn,
                RuntimeToolId::AmneziaWg => NetworkTool::AmneziaWg,
                RuntimeToolId::TorBundle => NetworkTool::TorBundle,
                _ => unreachable!(),
            };
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let required = BTreeSet::from([network_tool]);
                ensure_network_runtime_tools(&app_handle_clone, &required)
            })
            .await
            .map_err(|e| format!("network runtime task join failed: {e}"))??;
        }
    }
    let status = collect_runtime_tools_status(&state)?
        .into_iter()
        .find(|item| item.id == tool.id())
        .ok_or_else(|| format!("runtime tool status was not found after install: {}", tool.id()))?;
    Ok(ok(correlation_id, status))
}

fn collect_runtime_tools_status(state: &AppState) -> Result<Vec<RuntimeToolStatusView>, String> {
    let docker = docker_status(state);
    Ok(vec![
        docker_view(&docker),
        engine_view(state, EngineKind::Wayfern)?,
        engine_view(state, EngineKind::Camoufox)?,
        network_tool_view(state, NetworkTool::SingBox, &docker)?,
        network_tool_view(state, NetworkTool::OpenVpn, &docker)?,
        network_tool_view(state, NetworkTool::AmneziaWg, &docker)?,
        network_tool_view(state, NetworkTool::TorBundle, &docker)?,
    ])
}

fn docker_view(docker: &DockerStatus) -> RuntimeToolStatusView {
    RuntimeToolStatusView {
        id: RuntimeToolId::Docker.id().to_string(),
        name_key: RuntimeToolId::Docker.name_key().to_string(),
        status: if docker.installed {
            "installed".to_string()
        } else {
            "missing".to_string()
        },
        version: docker.client_version.clone(),
        action: if docker.installed {
            "none".to_string()
        } else {
            "external".to_string()
        },
        detail_key: if docker.installed {
            if docker.runtime_available {
                Some("settings.tools.detail.dockerReady".to_string())
            } else {
                Some("settings.tools.detail.dockerStopped".to_string())
            }
        } else {
            Some("settings.tools.detail.dockerMissing".to_string())
        },
    }
}

fn engine_view(state: &AppState, engine: EngineKind) -> Result<RuntimeToolStatusView, String> {
    let runtime = EngineRuntime::new(state.engine_runtime_root.clone()).map_err(|e| e.to_string())?;
    let installation = runtime.installed(engine).map_err(|e| e.to_string())?;
    Ok(RuntimeToolStatusView {
        id: engine.as_key().to_string(),
        name_key: match engine {
            EngineKind::Wayfern => RuntimeToolId::Wayfern.name_key(),
            EngineKind::Camoufox => RuntimeToolId::Camoufox.name_key(),
        }
        .to_string(),
        status: if installation.is_some() {
            "installed".to_string()
        } else {
            "missing".to_string()
        },
        version: installation.map(|item| item.version),
        action: if runtime.installed(engine).map_err(|e| e.to_string())?.is_some() {
            "none".to_string()
        } else {
            "internal".to_string()
        },
        detail_key: None,
    })
}

fn network_tool_view(
    state: &AppState,
    tool: NetworkTool,
    docker: &DockerStatus,
) -> Result<RuntimeToolStatusView, String> {
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone())?;
    if runtime.installed(tool)?.is_some() {
        return Ok(RuntimeToolStatusView {
            id: tool.as_key().to_string(),
            name_key: tool_name_key(tool).to_string(),
            status: "installed".to_string(),
            version: Some(tool_version(tool).to_string()),
            action: "none".to_string(),
            detail_key: None,
        });
    }

    if let Some(version) = detect_network_tool_version(&state.app_handle, tool) {
        return Ok(RuntimeToolStatusView {
            id: tool.as_key().to_string(),
            name_key: tool_name_key(tool).to_string(),
            status: "installed".to_string(),
            version: Some(version),
            action: "none".to_string(),
            detail_key: None,
        });
    }

    if supports_docker_fallback(tool) && docker.runtime_available {
        return Ok(RuntimeToolStatusView {
            id: tool.as_key().to_string(),
            name_key: tool_name_key(tool).to_string(),
            status: "docker".to_string(),
            version: docker
                .client_version
                .as_ref()
                .map(|version| format!("Docker {version}"))
                .or_else(|| Some("Docker".to_string())),
            action: "none".to_string(),
            detail_key: Some("settings.tools.detail.dockerBacked".to_string()),
        });
    }

    Ok(RuntimeToolStatusView {
        id: tool.as_key().to_string(),
        name_key: tool_name_key(tool).to_string(),
        status: "missing".to_string(),
        version: None,
        action: "internal".to_string(),
        detail_key: if supports_docker_fallback(tool) {
            Some("settings.tools.detail.localOrDocker".to_string())
        } else {
            None
        },
    })
}

fn tool_name_key(tool: NetworkTool) -> &'static str {
    match tool {
        NetworkTool::SingBox => RuntimeToolId::SingBox.name_key(),
        NetworkTool::OpenVpn => RuntimeToolId::OpenVpn.name_key(),
        NetworkTool::AmneziaWg => RuntimeToolId::AmneziaWg.name_key(),
        NetworkTool::TorBundle => RuntimeToolId::TorBundle.name_key(),
    }
}

fn tool_version(tool: NetworkTool) -> &'static str {
    match tool {
        NetworkTool::SingBox => "1.12.0",
        NetworkTool::OpenVpn => "2.6.16-I001",
        NetworkTool::AmneziaWg => "2.0.0",
        NetworkTool::TorBundle => "15.0.9",
    }
}

fn supports_docker_fallback(tool: NetworkTool) -> bool {
    matches!(
        tool,
        NetworkTool::SingBox | NetworkTool::OpenVpn | NetworkTool::AmneziaWg
    )
}

fn docker_status(state: &AppState) -> DockerStatus {
    let client_version = docker_client_version();
    let probe = probe_container_runtime(state, None);
    DockerStatus {
        installed: client_version.is_some(),
        client_version,
        runtime_available: probe.runtime_version.is_some(),
    }
}

fn docker_client_version() -> Option<String> {
    let output = hidden_command("docker").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_docker_client_version(&String::from_utf8_lossy(&output.stdout))
}

fn detect_network_tool_version(app_handle: &AppHandle, tool: NetworkTool) -> Option<String> {
    match tool {
        NetworkTool::SingBox => {
            let path = resolve_sing_box_binary_path(app_handle).ok()?;
            probe_command_version(&path, &[&["version"], &["--version"]])
        }
        NetworkTool::OpenVpn => {
            let path = resolve_openvpn_binary_path(app_handle).ok()?;
            probe_command_version(&path, &[&["--version"], &["version"]])
        }
        NetworkTool::AmneziaWg => {
            let path = resolve_amneziawg_binary_path(app_handle).ok()?;
            probe_command_version(&path, &[&["--version"], &["version"]])
        }
        NetworkTool::TorBundle => {
            let path = resolve_tor_binary_path(app_handle)?;
            probe_command_version(&path, &[&["--version"], &["version"]])
        }
    }
}

fn probe_command_version(path: &PathBuf, candidates: &[&[&str]]) -> Option<String> {
    for args in candidates {
        let output = hidden_command(path.as_os_str()).args(*args).output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(version) = first_semver_like_token(&stdout) {
            return Some(version);
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(version) = first_semver_like_token(&stderr) {
            return Some(version);
        }
        if output.status.success() {
            return Some("installed".to_string());
        }
    }
    None
}

fn hidden_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

fn parse_docker_client_version(text: &str) -> Option<String> {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| line.to_ascii_lowercase().starts_with("docker version "))?;
    let raw = line
        .trim_start_matches("Docker version ")
        .split(',')
        .next()?
        .trim();
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    }
}

fn first_semver_like_token(text: &str) -> Option<String> {
    for token in text.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';' || ch == '(' || ch == ')') {
        let trimmed =
            token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.' && ch != '-');
        if looks_like_version(trimmed) {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn looks_like_version(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let mut has_digit = false;
    let mut dot_count = 0usize;
    let mut has_separator = false;
    let mut previous_was_separator = false;
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            has_digit = true;
            previous_was_separator = false;
        } else if ch == '.' {
            dot_count += 1;
            has_separator = true;
            if previous_was_separator {
                return false;
            }
            previous_was_separator = true;
        } else if ch == '-' {
            has_separator = true;
            if previous_was_separator {
                return false;
            }
            previous_was_separator = true;
        } else if ch.is_ascii_alphabetic() {
            previous_was_separator = false;
        } else {
            return false;
        }
    }
    has_digit
        && dot_count >= 1
        && has_separator
        && !previous_was_separator
        && value.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{first_semver_like_token, parse_docker_client_version};

    #[test]
    fn docker_client_version_is_parsed() {
        assert_eq!(
            parse_docker_client_version("Docker version 29.2.1, build 123456"),
            Some("29.2.1".to_string())
        );
    }

    #[test]
    fn semver_like_tokens_are_detected() {
        assert_eq!(
            first_semver_like_token("OpenVPN 2.6.16-I001 amd64"),
            Some("2.6.16-I001".to_string())
        );
        assert_eq!(
            first_semver_like_token("sing-box version 1.12.0"),
            Some("1.12.0".to_string())
        );
    }
}
