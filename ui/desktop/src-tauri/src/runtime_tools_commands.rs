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
#[path = "runtime_tools_commands_status.rs"]
mod status;

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
    LinuxBrowserSandbox,
    Chromium,
    UngoogledChromium,
    FirefoxEsr,
    Librewolf,
    SingBox,
    OpenVpn,
    AmneziaWg,
    TorBundle,
}

impl RuntimeToolId {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "docker" => Some(Self::Docker),
            "linux-browser-sandbox" => Some(Self::LinuxBrowserSandbox),
            "chromium" => Some(Self::Chromium),
            "ungoogled-chromium" => Some(Self::UngoogledChromium),
            "firefox-esr" => Some(Self::FirefoxEsr),
            "librewolf" => Some(Self::Librewolf),
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
            Self::LinuxBrowserSandbox => "linux-browser-sandbox",
            Self::Chromium => "chromium",
            Self::UngoogledChromium => "ungoogled-chromium",
            Self::FirefoxEsr => "firefox-esr",
            Self::Librewolf => "librewolf",
            Self::SingBox => "sing-box",
            Self::OpenVpn => "openvpn",
            Self::AmneziaWg => "amneziawg",
            Self::TorBundle => "tor-bundle",
        }
    }

    fn name_key(self) -> &'static str {
        match self {
            Self::Docker => "settings.tools.docker",
            Self::LinuxBrowserSandbox => "settings.tools.linuxBrowserSandbox",
            Self::Chromium => "settings.tools.chromium",
            Self::UngoogledChromium => "settings.tools.ungoogledChromium",
            Self::FirefoxEsr => "settings.tools.firefoxEsr",
            Self::Librewolf => "settings.tools.librewolf",
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
        RuntimeToolId::LinuxBrowserSandbox => {
            return Err(
                "linux browser sandbox is configured at OS level (sysctl/AppArmor) and is not auto-installed by launcher"
                    .to_string(),
            )
        }
        RuntimeToolId::Chromium
        | RuntimeToolId::UngoogledChromium
        | RuntimeToolId::FirefoxEsr
        | RuntimeToolId::Librewolf => {
            let engine = match tool {
                RuntimeToolId::Chromium => EngineKind::Chromium,
                RuntimeToolId::UngoogledChromium => EngineKind::UngoogledChromium,
                RuntimeToolId::FirefoxEsr => EngineKind::FirefoxEsr,
                RuntimeToolId::Librewolf => EngineKind::Librewolf,
                _ => unreachable!(),
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
        .ok_or_else(|| {
            format!(
                "runtime tool status was not found after install: {}",
                tool.id()
            )
        })?;
    Ok(ok(correlation_id, status))
}

fn collect_runtime_tools_status(state: &AppState) -> Result<Vec<RuntimeToolStatusView>, String> {
    status::collect_runtime_tools_status(state)
}
