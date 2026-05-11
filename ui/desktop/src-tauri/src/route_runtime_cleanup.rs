use super::*;

pub(crate) fn cleanup_legacy_route_runtime_impl(app_handle: &AppHandle) {
    cleanup_legacy_amnezia_tunnels(app_handle);
    let active_profiles = {
        let state = app_handle.state::<AppState>();
        state
            .launched_processes
            .lock()
            .ok()
            .map(|value| {
                value
                    .iter()
                    .filter_map(|(profile_id, pid)| {
                        if is_process_running(*pid) {
                            Some(*profile_id)
                        } else {
                            None
                        }
                    })
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default()
    };
    cleanup_stale_container_route_runtimes(app_handle, &active_profiles);
}

pub(crate) fn cleanup_stale_route_runtime_artifacts_impl(
    app_handle: &AppHandle,
    active_profiles: &BTreeSet<Uuid>,
) {
    let state = app_handle.state::<AppState>();
    if let Ok(entries) = fs::read_dir(&state.profile_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let Ok(profile_id) = Uuid::parse_str(name) else {
                continue;
            };
            if active_profiles.contains(&profile_id) {
                continue;
            }
            cleanup_profile_runtime_artifacts(path.join("runtime"));
        }
    }
}

pub(crate) fn terminate_pid_impl(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let _ = hidden_command("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}

pub(crate) fn is_container_runtime_active_impl(container_name: &str) -> bool {
    hidden_command("docker")
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

fn cleanup_legacy_amnezia_tunnels(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    let mut tunnel_names = BTreeSet::new();
    let mut artifact_paths = Vec::new();
    if let Ok(entries) = fs::read_dir(&state.profile_root) {
        for profile_entry in entries.flatten() {
            let runtime_dir = profile_entry.path().join("runtime");
            if !runtime_dir.is_dir() {
                continue;
            }
            if let Ok(runtime_entries) = fs::read_dir(&runtime_dir) {
                for runtime_entry in runtime_entries.flatten() {
                    let path = runtime_entry.path();
                    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                        continue;
                    };
                    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                        continue;
                    };
                    if stem.starts_with("awg-") {
                        tunnel_names.insert(stem.to_string());
                        if file_name.ends_with(".conf") {
                            artifact_paths.push(path);
                        }
                    }
                }
            }
        }
    }

    for tunnel_name in tunnel_names {
        cleanup_legacy_amnezia_tunnel(app_handle, &tunnel_name);
    }
    for artifact_path in artifact_paths {
        let _ = fs::remove_file(artifact_path);
    }
}

fn cleanup_profile_runtime_artifacts(runtime_dir: PathBuf) {
    if !runtime_dir.is_dir() {
        return;
    }
    if let Ok(entries) = fs::read_dir(&runtime_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if should_remove_runtime_artifact_impl(file_name) {
                let _ = fs::remove_file(&path);
            }
        }
    }
    let is_empty = fs::read_dir(&runtime_dir)
        .ok()
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false);
    if is_empty {
        let _ = fs::remove_dir(&runtime_dir);
    }
}

pub(crate) fn should_remove_runtime_artifact_impl(file_name: &str) -> bool {
    matches!(
        file_name,
        "sing-box-route.json"
            | "sing-box-route.log"
            | "openvpn-route.log"
            | "openvpn-route.ovpn"
            | "container-openvpn.log"
            | "container-openvpn.ovpn"
    ) || file_name.starts_with("awg-")
        || file_name.starts_with("openvpn-auth-")
}

fn cleanup_legacy_amnezia_tunnel(app_handle: &AppHandle, tunnel_name: &str) {
    if !amnezia::amnezia_tunnel_service_exists_impl(tunnel_name) {
        return;
    }
    let _ = amnezia::stop_amnezia_tunnel_service_impl(tunnel_name);
    let _ = amnezia::wait_amnezia_tunnel_state_impl(tunnel_name, false, 8_000);
    if let Ok(binary) = crate::network_runtime::resolve_amneziawg_binary_path(app_handle) {
        if amnezia::uninstall_amnezia_tunnel_impl(&binary, tunnel_name).is_ok() {
            return;
        }
    }
    let _ = amnezia::delete_amnezia_tunnel_service_impl(tunnel_name);
}
