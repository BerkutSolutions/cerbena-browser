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

fn emit_container_launch_progress(
    app_handle: &AppHandle,
    profile_id: Uuid,
    stage_key: &str,
    message_key: &str,
) {
    let _ = app_handle.emit(
        "profile-launch-progress",
        serde_json::json!({
            "profileId": profile_id.to_string(),
            "stageKey": stage_key,
            "messageKey": message_key,
            "done": false,
            "error": serde_json::Value::Null,
        }),
    );
}

pub fn launch_sing_box_container_runtime(
    app_handle: &AppHandle,
    profile_id: Uuid,
    runtime_dir: &Path,
    host_proxy_port: u16,
    config_json: &str,
) -> Result<ContainerRouteLaunch, String> {
    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-image",
        "profile.launchProgress.containerImage",
    );
    let image_tag = ensure_container_helper_image(app_handle)?;
    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-network",
        "profile.launchProgress.containerNetwork",
    );
    let network_name = ensure_profile_container_environment(app_handle, profile_id)?;
    let container_name = container_runtime_name(profile_id);
    stop_container_runtime(&container_name);

    let config_path = runtime_dir.join("container-sing-box.json");
    let log_path = runtime_dir.join("container-sing-box.log");
    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-config",
        "profile.launchProgress.containerConfig",
    );
    fs::write(&config_path, config_json)
        .map_err(|e| format!("write container sing-box config: {e}"))?;
    let _ = fs::remove_file(&log_path);
    fs::File::create(&log_path).map_err(|e| format!("create container sing-box log file: {e}"))?;

    let run_output = docker_command()
        .args([
            "run",
            "--detach",
            "--name",
            &container_name,
            "--network",
            &network_name,
            "--memory",
            CONTAINER_MEMORY_LIMIT,
            "--cpus",
            CONTAINER_CPU_LIMIT,
            "--pids-limit",
            CONTAINER_PIDS_LIMIT,
            "--restart",
            "no",
            "--label",
            "cerbena.managed=true",
            "--label",
            "cerbena.kind=network-sandbox-runtime",
            "--label",
            &format!("cerbena.profile_id={profile_id}"),
            "--publish",
            &format!("127.0.0.1:{host_proxy_port}:{CONTAINER_PROXY_PORT}"),
            "--volume",
            &format!("{}:/work/sing-box.json:ro", config_path.display()),
            "--volume",
            &format!("{}:/work/container-route.log", log_path.display()),
            "--env",
            "CERBENA_RUNTIME_KIND=sing-box",
            "--env",
            "CERBENA_SINGBOX_CONFIG=/work/sing-box.json",
            "--env",
            "CERBENA_ROUTE_LOG=/work/container-route.log",
            image_tag.as_str(),
        ])
        .output()
        .map_err(|e| format!("start sing-box container runtime: {e}"))?;
    if !run_output.status.success() {
        return Err(format!(
            "start sing-box container runtime failed: {}",
            command_error_message(&run_output)
        ));
    }

    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-proxy",
        "profile.launchProgress.containerProxy",
    );
    wait_for_container_proxy(&container_name, host_proxy_port, &log_path)?;
    Ok(ContainerRouteLaunch {
        container_name,
        host_proxy_port,
        config_path,
        cleanup_paths: vec![log_path],
    })
}

pub fn launch_openvpn_container_runtime(
    app_handle: &AppHandle,
    profile_id: Uuid,
    runtime_dir: &Path,
    host_proxy_port: u16,
    config_text: &str,
    auth_path: Option<&PathBuf>,
) -> Result<ContainerRouteLaunch, String> {
    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-image",
        "profile.launchProgress.containerImage",
    );
    let image_tag = ensure_container_helper_image(app_handle)?;
    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-network",
        "profile.launchProgress.containerNetwork",
    );
    let network_name = ensure_profile_container_environment(app_handle, profile_id)?;
    let container_name = container_runtime_name(profile_id);
    stop_container_runtime(&container_name);

    let config_path = runtime_dir.join("container-openvpn.ovpn");
    let log_path = runtime_dir.join("container-openvpn.log");
    let container_auth_path = auth_path.map(|_| runtime_dir.join("container-openvpn-auth.txt"));
    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-config",
        "profile.launchProgress.containerConfig",
    );
    fs::write(&config_path, config_text)
        .map_err(|e| format!("write container openvpn config: {e}"))?;
    let _ = fs::remove_file(&log_path);
    fs::File::create(&log_path).map_err(|e| format!("create container openvpn log file: {e}"))?;
    if let (Some(source_path), Some(target_path)) = (auth_path, container_auth_path.as_ref()) {
        fs::copy(source_path, target_path)
            .map_err(|e| format!("copy container openvpn auth file: {e}"))?;
    }

    let config_mount = format!("{}:/work/openvpn.ovpn:ro", config_path.display());
    let log_mount = format!("{}:/work/route.log", log_path.display());
    let mut command = docker_command();
    command.args([
        "run",
        "--detach",
        "--name",
        &container_name,
        "--network",
        &network_name,
        "--cap-add",
        "NET_ADMIN",
        "--cap-add",
        "NET_RAW",
        "--device",
        "/dev/net/tun",
        "--memory",
        CONTAINER_MEMORY_LIMIT,
        "--cpus",
        CONTAINER_CPU_LIMIT,
        "--pids-limit",
        CONTAINER_PIDS_LIMIT,
        "--restart",
        "no",
        "--label",
        "cerbena.managed=true",
        "--label",
        "cerbena.kind=network-sandbox-runtime",
        "--label",
        &format!("cerbena.profile_id={profile_id}"),
        "--publish",
        &format!("127.0.0.1:{host_proxy_port}:{CONTAINER_PROXY_PORT}"),
        "--volume",
        &config_mount,
        "--volume",
        &log_mount,
        "--env",
        "CERBENA_RUNTIME_KIND=openvpn",
        "--env",
        "CERBENA_OPENVPN_CONFIG=/work/openvpn.ovpn",
        "--env",
        "CERBENA_ROUTE_LOG=/work/route.log",
        "--env",
        &format!("CERBENA_PROXY_PORT={CONTAINER_PROXY_PORT}"),
        "--env",
        &format!("CERBENA_PROXY_LISTEN={CONTAINER_PROXY_ADDR}"),
    ]);
    if let Some(path) = container_auth_path.as_ref() {
        let auth_mount = format!("{}:/work/openvpn-auth.txt:ro", path.display());
        command.args([
            "--volume",
            auth_mount.as_str(),
            "--env",
            "CERBENA_OPENVPN_AUTH=/work/openvpn-auth.txt",
        ]);
    }
    let run_output = command
        .arg(image_tag.as_str())
        .output()
        .map_err(|e| format!("start openvpn container runtime: {e}"))?;
    if !run_output.status.success() {
        return Err(format!(
            "start openvpn container runtime failed: {}",
            command_error_message(&run_output)
        ));
    }

    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-proxy",
        "profile.launchProgress.containerProxy",
    );
    wait_for_container_proxy(&container_name, host_proxy_port, &log_path)?;
    Ok(ContainerRouteLaunch {
        container_name,
        host_proxy_port,
        config_path,
        cleanup_paths: {
            let mut paths = vec![log_path];
            if let Some(path) = container_auth_path {
                paths.push(path);
            }
            paths
        },
    })
}

pub fn launch_amnezia_container_runtime(
    app_handle: &AppHandle,
    profile_id: Uuid,
    runtime_dir: &Path,
    host_proxy_port: u16,
    config_text: &str,
) -> Result<ContainerRouteLaunch, String> {
    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-image",
        "profile.launchProgress.containerImage",
    );
    let image_tag = ensure_container_helper_image(app_handle)?;
    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-network",
        "profile.launchProgress.containerNetwork",
    );
    let network_name = ensure_profile_container_environment(app_handle, profile_id)?;
    let container_name = container_runtime_name(profile_id);
    stop_container_runtime(&container_name);

    let config_path = runtime_dir.join("amnezia-container.conf");
    let log_path = runtime_dir.join("amnezia-container.log");
    let container_config = sanitize_container_amnezia_config(config_text);
    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-config",
        "profile.launchProgress.containerConfig",
    );
    fs::write(&config_path, container_config)
        .map_err(|e| format!("write container amnezia config: {e}"))?;
    let _ = fs::remove_file(&log_path);
    fs::File::create(&log_path).map_err(|e| format!("create container log file: {e}"))?;

    let config_mount = format!("{}:/work/amnezia.conf:ro", config_path.display());
    let log_mount = format!("{}:/work/route.log", log_path.display());
    let publish = format!("127.0.0.1:{host_proxy_port}:{CONTAINER_PROXY_PORT}");
    let run_output = docker_command()
        .args([
            "run",
            "--detach",
            "--name",
            &container_name,
            "--network",
            &network_name,
            "--cap-add",
            "NET_ADMIN",
            "--cap-add",
            "NET_RAW",
            "--device",
            "/dev/net/tun",
            "--memory",
            CONTAINER_MEMORY_LIMIT,
            "--cpus",
            CONTAINER_CPU_LIMIT,
            "--pids-limit",
            CONTAINER_PIDS_LIMIT,
            "--restart",
            "no",
            "--label",
            "cerbena.managed=true",
            "--label",
            "cerbena.kind=network-sandbox-runtime",
            "--label",
            &format!("cerbena.profile_id={profile_id}"),
            "--publish",
            &publish,
            "--volume",
            &config_mount,
            "--volume",
            &log_mount,
            "--env",
            &format!("CERBENA_PROFILE_ID={profile_id}"),
            "--env",
            &format!("CERBENA_PROXY_PORT={CONTAINER_PROXY_PORT}"),
            "--env",
            &format!("CERBENA_PROXY_LISTEN={CONTAINER_PROXY_ADDR}"),
            "--env",
            "CERBENA_AMNEZIA_CONFIG=/work/amnezia.conf",
            image_tag.as_str(),
        ])
        .output()
        .map_err(|e| format!("start container sandbox runtime: {e}"))?;
    if !run_output.status.success() {
        return Err(format!(
            "start container sandbox runtime failed: {}",
            command_error_message(&run_output)
        ));
    }

    emit_container_launch_progress(
        app_handle,
        profile_id,
        "container-proxy",
        "profile.launchProgress.containerProxy",
    );
    wait_for_container_proxy(&container_name, host_proxy_port, &log_path)?;

    Ok(ContainerRouteLaunch {
        container_name,
        host_proxy_port,
        config_path,
        cleanup_paths: vec![log_path],
    })
}

pub fn stop_container_runtime(container_name: &str) {
    let _ = docker_command().args(["rm", "-f", container_name]).output();
}

pub fn cleanup_stale_container_route_runtimes(
    _app_handle: &AppHandle,
    active_profile_ids: &std::collections::BTreeSet<Uuid>,
) {
    let output = match docker_command()
        .args([
            "ps",
            "-a",
            "--filter",
            "label=cerbena.kind=network-sandbox-runtime",
            "--format",
            "{{.Names}}|{{.Labels}}",
        ])
        .output()
    {
        Ok(value) => value,
        Err(_) => return,
    };
    if !output.status.success() {
        return;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some((name, labels)) = line.split_once('|') else {
            continue;
        };
        let Some(profile_id) = extract_profile_label(labels) else {
            continue;
        };
        if !active_profile_ids.contains(&profile_id) {
            stop_container_runtime(name);
        }
    }
}

pub fn container_runtime_name(profile_id: Uuid) -> String {
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
