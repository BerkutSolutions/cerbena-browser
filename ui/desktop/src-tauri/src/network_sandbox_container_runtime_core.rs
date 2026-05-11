use std::{
    fs,
    io::Write,
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::{network_sandbox_container::ensure_profile_container_environment, state::AppState};

const CONTAINER_IMAGE_REVISION: &str = "2026-05-03-r7";
const CONTAINER_IMAGE_TAG: &str = "cerbena/network-sandbox:2026-05-03-r7";
pub const CONTAINER_PROXY_PORT: u16 = 17890;
const CONTAINER_PROXY_ADDR: &str = "0.0.0.0:17890";
const CONTAINER_MEMORY_LIMIT: &str = "192m";
const CONTAINER_CPU_LIMIT: &str = "1.0";
const CONTAINER_PIDS_LIMIT: &str = "128";

fn docker_command() -> Command {
    #[cfg(target_os = "windows")]
    let mut command = Command::new("docker");
    #[cfg(not(target_os = "windows"))]
    let command = Command::new("docker");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

const CONTAINER_DOCKERFILE: &str = include_str!("../runtime/network-sandbox-container/Dockerfile");
const CONTAINER_ENTRYPOINT: &str =
    include_str!("../runtime/network-sandbox-container/entrypoint.sh");
const CONTAINER_PROXY_SOURCE: &str =
    include_str!("../runtime/network-sandbox-container/container_socks_proxy.go");
const CONTAINER_SYSCTL_WRAPPER: &str =
    include_str!("../runtime/network-sandbox-container/sysctl_wrapper.sh");
const CONTAINER_OPENVPN_DNS_SYNC: &str =
    include_str!("../runtime/network-sandbox-container/openvpn_dns_sync.sh");

#[derive(Debug, Clone)]
pub struct ContainerRouteLaunch {
    pub container_name: String,
    pub host_proxy_port: u16,
    pub config_path: PathBuf,
    pub cleanup_paths: Vec<PathBuf>,
}


#[path = "network_sandbox_container_runtime_core_ops.rs"]
mod ops;
pub(crate) use ops::{
    cleanup_stale_container_route_runtimes,
    launch_amnezia_container_runtime,
    launch_openvpn_container_runtime,
    launch_sing_box_container_runtime,
    stop_container_runtime,
};

fn route_container_name(profile_id: Uuid) -> String {
    format!("cerbena-route-{}", profile_id.as_simple())
}

fn ensure_container_helper_image(app_handle: &AppHandle) -> Result<String, String> {
    if docker_image_exists(CONTAINER_IMAGE_TAG) {
        return Ok(CONTAINER_IMAGE_TAG.to_string());
    }
    let state = app_handle.state::<AppState>();
    let context_dir = state
        .network_runtime_root
        .join("container-sandbox")
        .join(CONTAINER_IMAGE_REVISION);
    write_container_build_context(&context_dir)?;
    let build_output = docker_command()
        .args([
            "build",
            "--tag",
            CONTAINER_IMAGE_TAG,
            context_dir.to_string_lossy().as_ref(),
        ])
        .output()
        .map_err(|e| format!("build container sandbox image: {e}"))?;
    if build_output.status.success() {
        return Ok(CONTAINER_IMAGE_TAG.to_string());
    }
    Err(format!(
        "build container sandbox image failed: {}",
        command_error_message(&build_output)
    ))
}

fn docker_image_exists(tag: &str) -> bool {
    docker_command()
        .args(["image", "inspect", tag])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn write_container_build_context(context_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(context_dir).map_err(|e| format!("create container build context: {e}"))?;
    write_utf8_file(&context_dir.join("Dockerfile"), CONTAINER_DOCKERFILE)?;
    write_utf8_file(&context_dir.join("entrypoint.sh"), CONTAINER_ENTRYPOINT)?;
    write_utf8_file(
        &context_dir.join("container_socks_proxy.go"),
        CONTAINER_PROXY_SOURCE,
    )?;
    write_utf8_file(
        &context_dir.join("sysctl_wrapper.sh"),
        CONTAINER_SYSCTL_WRAPPER,
    )?;
    write_utf8_file(
        &context_dir.join("openvpn_dns_sync.sh"),
        CONTAINER_OPENVPN_DNS_SYNC,
    )?;
    Ok(())
}

fn write_utf8_file(path: &Path, value: &str) -> Result<(), String> {
    let mut file = fs::File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    file.write_all(value.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))
}

fn sanitize_container_amnezia_config(value: &str) -> String {
    let mut cleaned = Vec::new();
    for raw_line in value.replace('\r', "").lines() {
        let trimmed = raw_line.trim();
        if trimmed.to_ascii_lowercase().starts_with("dns =") {
            continue;
        }
        cleaned.push(raw_line.trim_end().to_string());
    }
    let mut text = cleaned.join("\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn wait_for_container_proxy(
    container_name: &str,
    host_proxy_port: u16,
    log_path: &Path,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        if TcpStream::connect(("127.0.0.1", host_proxy_port)).is_ok() {
            return Ok(());
        }
        if !is_container_running(container_name) {
            let log_tail = fs::read_to_string(log_path)
                .ok()
                .map(|value| tail_lines(&value, 20))
                .unwrap_or_default();
            let docker_logs = container_logs(container_name);
            return Err(format!(
                "container sandbox runtime exited before proxy became ready{}{}",
                if docker_logs.is_empty() {
                    String::new()
                } else {
                    format!(": {docker_logs}")
                },
                if log_tail.is_empty() {
                    String::new()
                } else {
                    format!(" | {log_tail}")
                }
            ));
        }
        if Instant::now() >= deadline {
            let docker_logs = container_logs(container_name);
            return Err(format!(
                "container sandbox proxy did not become ready in time{}",
                if docker_logs.is_empty() {
                    String::new()
                } else {
                    format!(": {docker_logs}")
                }
            ));
        }
        thread::sleep(Duration::from_millis(300));
    }
}

fn is_container_running(container_name: &str) -> bool {
    docker_command()
        .args(["inspect", "--format", "{{.State.Running}}", container_name])
        .output()
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

fn container_logs(container_name: &str) -> String {
    docker_command()
        .args(["logs", "--tail", "50", container_name])
        .output()
        .ok()
        .map(|output| command_error_message(&output))
        .unwrap_or_default()
}

fn command_error_message(output: &Output) -> String {
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

fn extract_profile_label(labels: &str) -> Option<Uuid> {
    for label in labels.split(',') {
        let (key, value) = label.split_once('=')?;
        if key.trim() == "cerbena.profile_id" {
            return Uuid::parse_str(value.trim()).ok();
        }
    }
    None
}

fn tail_lines(value: &str, limit: usize) -> String {
    let lines = value.lines().rev().take(limit).collect::<Vec<_>>();
    lines.into_iter().rev().collect::<Vec<_>>().join(" | ")
}

