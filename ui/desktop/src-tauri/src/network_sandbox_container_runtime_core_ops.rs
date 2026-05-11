use super::*;
#[path = "network_sandbox_container_runtime_core_ops_amnezia.rs"]
mod amnezia;
#[path = "network_sandbox_container_runtime_core_ops_openvpn.rs"]
mod openvpn;
#[path = "network_sandbox_container_runtime_core_ops_singbox.rs"]
mod singbox;

pub(crate) fn emit_container_launch_progress(
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
    singbox::launch_sing_box_container_runtime_impl(
        app_handle,
        profile_id,
        runtime_dir,
        host_proxy_port,
        config_json,
    )
}

pub fn launch_openvpn_container_runtime(
    app_handle: &AppHandle,
    profile_id: Uuid,
    runtime_dir: &Path,
    host_proxy_port: u16,
    config_text: &str,
    auth_path: Option<&PathBuf>,
) -> Result<ContainerRouteLaunch, String> {
    openvpn::launch_openvpn_container_runtime_impl(
        app_handle,
        profile_id,
        runtime_dir,
        host_proxy_port,
        config_text,
        auth_path,
    )
}

pub fn launch_amnezia_container_runtime(
    app_handle: &AppHandle,
    profile_id: Uuid,
    runtime_dir: &Path,
    host_proxy_port: u16,
    config_text: &str,
) -> Result<ContainerRouteLaunch, String> {
    amnezia::launch_amnezia_container_runtime_impl(
        app_handle,
        profile_id,
        runtime_dir,
        host_proxy_port,
        config_text,
    )
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
    route_container_name(profile_id)
}

