use std::{
    collections::BTreeSet,
    process::Command,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::state::AppState;

const CONTAINER_NETWORK_PREFIX: &str = "cerbena-profile-";

fn docker_command() -> Command {
    let mut command = Command::new("docker");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSandboxRuntimeProbe {
    pub available: bool,
    pub runtime_kind: String,
    pub runtime_version: Option<String>,
    pub runtime_platform: Option<String>,
    pub active_sandboxes: u8,
    pub max_active_sandboxes: u8,
    pub supports_native_isolation: bool,
    pub reason: String,
}

pub fn probe_container_runtime(
    state: &AppState,
    profile_id: Option<Uuid>,
) -> ContainerSandboxRuntimeProbe {
    let max_active_sandboxes = state
        .network_sandbox_store
        .lock()
        .ok()
        .map(|store| store.global.max_active_sandboxes.max(1))
        .unwrap_or(2);
    let active_sandboxes = active_container_sandbox_count(state, profile_id);
    match docker_version_details() {
        Ok(details) => ContainerSandboxRuntimeProbe {
            available: active_sandboxes < max_active_sandboxes,
            runtime_kind: "docker-desktop".to_string(),
            runtime_version: Some(details.version),
            runtime_platform: Some(format!("{}/{}", details.os, details.arch)),
            active_sandboxes,
            max_active_sandboxes,
            supports_native_isolation: true,
            reason: if active_sandboxes < max_active_sandboxes {
                "Docker Desktop container runtime is available and can build a profile-scoped isolated route helper on first launch".to_string()
            } else {
                format!(
                    "Container sandbox capacity is exhausted ({active_sandboxes}/{max_active_sandboxes} active)"
                )
            },
        },
        Err(error) => ContainerSandboxRuntimeProbe {
            available: false,
            runtime_kind: "docker-desktop".to_string(),
            runtime_version: None,
            runtime_platform: None,
            active_sandboxes,
            max_active_sandboxes,
            supports_native_isolation: true,
            reason: error,
        },
    }
}

pub fn ensure_profile_container_environment(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    let probe = probe_container_runtime(state.inner(), Some(profile_id));
    if !probe.available {
        return Err(format!(
            "container sandbox runtime is unavailable: {}",
            probe.reason
        ));
    }
    let network_name = container_network_name(profile_id);
    let inspect = docker_command()
        .args(["network", "inspect", &network_name])
        .output()
        .map_err(|e| format!("inspect container sandbox network: {e}"))?;
    if inspect.status.success() {
        return Ok(network_name);
    }
    let profile_label = format!("cerbena.profile_id={profile_id}");
    let create = docker_command()
        .args([
            "network",
            "create",
            "--driver",
            "bridge",
            "--label",
            "cerbena.managed=true",
            "--label",
            &profile_label,
            &network_name,
        ])
        .output()
        .map_err(|e| format!("create container sandbox network: {e}"))?;
    if create.status.success() {
        Ok(network_name)
    } else {
        Err(format!(
            "create container sandbox network failed: {}",
            command_error_message(&create)
        ))
    }
}

pub fn remove_profile_container_environment(_app_handle: &AppHandle, profile_id: Uuid) {
    let network_name = container_network_name(profile_id);
    let remove = docker_command()
        .args(["network", "rm", &network_name])
        .output();
    if let Ok(output) = remove {
        if !output.status.success() {
            let message = command_error_message(&output);
            if !message.to_lowercase().contains("no such network") {
                eprintln!(
                    "[network-sandbox] remove container network {} failed: {}",
                    network_name, message
                );
            }
        }
    }
}

pub fn cleanup_stale_container_environments(
    app_handle: &AppHandle,
    active_profiles: &BTreeSet<Uuid>,
) {
    let output = match docker_command()
        .args(["network", "ls", "--format", "{{.Name}}"])
        .output()
    {
        Ok(value) => value,
        Err(_) => return,
    };
    if !output.status.success() {
        return;
    }
    let names = String::from_utf8_lossy(&output.stdout);
    for name in names.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some(profile_id) = profile_id_from_network_name(name) else {
            continue;
        };
        if !active_profiles.contains(&profile_id) {
            remove_profile_container_environment(app_handle, profile_id);
        }
    }
}

fn active_container_sandbox_count(state: &AppState, profile_id: Option<Uuid>) -> u8 {
    let current_profile = profile_id.map(|value| value.to_string());
    state
        .network_sandbox_lifecycle
        .lock()
        .ok()
        .map(|lifecycle| {
            lifecycle
                .active_profiles
                .iter()
                .filter(|(key, record)| {
                    record.adapter.adapter_kind == "container-vm"
                        && current_profile
                            .as_ref()
                            .map(|profile| profile != *key)
                            .unwrap_or(true)
                })
                .count()
        })
        .unwrap_or(0)
        .min(u8::MAX as usize) as u8
}

pub fn container_network_name(profile_id: Uuid) -> String {
    format!("{CONTAINER_NETWORK_PREFIX}{profile_id}")
}

fn profile_id_from_network_name(value: &str) -> Option<Uuid> {
    let profile_part = value.strip_prefix(CONTAINER_NETWORK_PREFIX)?;
    Uuid::parse_str(profile_part).ok()
}

fn command_error_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }
    format!("exit status {}", output.status)
}

#[derive(Debug, Clone)]
struct DockerVersionDetails {
    version: String,
    os: String,
    arch: String,
}

fn docker_version_details() -> Result<DockerVersionDetails, String> {
    let output = docker_command()
        .arg("version")
        .output()
        .map_err(|e| format!("docker runtime is not installed or not reachable: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    parse_docker_version_output(&combined).ok_or_else(|| {
        let message = combined
            .lines()
            .map(str::trim)
            .find(|line| {
                !line.is_empty()
                    && !line.starts_with("Client:")
                    && !line.starts_with("Server:")
                    && !line.starts_with("Version:")
                    && !line.starts_with("API version:")
                    && !line.starts_with("Go version:")
                    && !line.starts_with("Git commit:")
                    && !line.starts_with("Built:")
                    && !line.starts_with("OS/Arch:")
                    && !line.starts_with("Context:")
                    && !line.starts_with("Engine:")
                    && !line.starts_with("containerd:")
                    && !line.starts_with("runc:")
                    && !line.starts_with("docker-init:")
            })
            .unwrap_or("Docker Desktop server runtime is unavailable");
        format!("container runtime probe failed: {message}")
    })
}

fn parse_docker_version_output(output: &str) -> Option<DockerVersionDetails> {
    let mut in_server = false;
    let mut version: Option<String> = None;
    let mut os_arch: Option<String> = None;
    for line in output.lines().map(str::trim) {
        if line.starts_with("Server:") {
            in_server = true;
            continue;
        }
        if line == "Client:" {
            in_server = false;
            continue;
        }
        if !in_server {
            continue;
        }
        if line.starts_with("Version:") && version.is_none() {
            version = Some(line.trim_start_matches("Version:").trim().to_string());
        }
        if line.starts_with("OS/Arch:") && os_arch.is_none() {
            os_arch = Some(line.trim_start_matches("OS/Arch:").trim().to_string());
        }
    }
    let version = version?;
    let os_arch = os_arch?;
    let (os, arch) = os_arch
        .split_once('/')
        .map(|(os, arch)| (os.trim().to_string(), arch.trim().to_string()))
        .unwrap_or_else(|| (os_arch.clone(), "unknown".to_string()));
    Some(DockerVersionDetails { version, os, arch })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_server_details_from_docker_version_output() {
        let sample = r#"
Client:
 Version:           29.2.1
 Context:           desktop-linux

Server: Docker Desktop 4.63.0 (220185)
 Engine:
  Version:          29.2.1
  API version:      1.53 (minimum version 1.44)
  OS/Arch:          linux/amd64
"#;
        let details = parse_docker_version_output(sample).expect("details");
        assert_eq!(details.version, "29.2.1");
        assert_eq!(details.os, "linux");
        assert_eq!(details.arch, "amd64");
    }

    #[test]
    fn ignores_client_only_probe_output() {
        let sample = r#"
Client:
 Version:           29.2.1
 Context:           default
permission denied while trying to connect to the docker API at npipe:////./pipe/docker_engine
"#;
        assert!(parse_docker_version_output(sample).is_none());
    }
}
