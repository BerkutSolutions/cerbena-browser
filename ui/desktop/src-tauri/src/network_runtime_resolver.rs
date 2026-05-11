use super::*;

pub(crate) fn resolve_sing_box_binary_path_impl(app_handle: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("BROWSER_SINGBOX_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone())?;
    if let Some(installed) = runtime.installed(NetworkTool::SingBox)? {
        return Ok(installed.primary);
    }
    if let Some(found) = find_path_binary_impl(if cfg!(target_os = "windows") {
        &["sing-box.exe", "sing-box"]
    } else {
        &["sing-box"]
    }) {
        return Ok(found);
    }
    Err("sing-box binary is unavailable".to_string())
}

pub(crate) fn resolve_openvpn_binary_path_impl(app_handle: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("BROWSER_OPENVPN_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone())?;
    if let Some(installed) = runtime.installed(NetworkTool::OpenVpn)? {
        return Ok(installed.primary);
    }
    if let Some(found) = find_path_binary_impl(if cfg!(target_os = "windows") {
        &["openvpn.exe", "openvpn"]
    } else {
        &["openvpn"]
    }) {
        return Ok(found);
    }
    Err("openvpn binary is unavailable".to_string())
}

pub(crate) fn resolve_amneziawg_binary_path_impl(app_handle: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("BROWSER_AMNEZIAWG_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone())?;
    if let Some(installed) = runtime.installed(NetworkTool::AmneziaWg)? {
        return Ok(installed.primary);
    }
    if let Some(found) = find_path_binary_impl(if cfg!(target_os = "windows") {
        &["amneziawg.exe", "amneziawg", "wireguard.exe", "wireguard"]
    } else {
        &["amneziawg", "wireguard"]
    }) {
        return Ok(found);
    }
    Err("amneziawg binary is unavailable".to_string())
}

pub(crate) fn resolve_tor_binary_path_impl(app_handle: &AppHandle) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("BROWSER_TOR_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone()).ok()?;
    if let Some(installed) = runtime.installed(NetworkTool::TorBundle).ok().flatten() {
        return Some(installed.primary);
    }
    find_path_binary_impl(if cfg!(target_os = "windows") { &["tor.exe", "tor"] } else { &["tor"] })
}

pub(crate) fn resolve_tor_pt_binary_path_impl(app_handle: &AppHandle, protocol: &str) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("BROWSER_TOR_PT_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone()).ok()?;
    if let Some(installed) = runtime.installed(NetworkTool::TorBundle).ok().flatten() {
        match protocol {
            "snowflake" => {
                if let Some(path) = installed.extras.get("snowflake-client").cloned() {
                    return Some(path);
                }
            }
            _ => {
                if let Some(path) = installed.extras.get("lyrebird").cloned() {
                    return Some(path);
                }
            }
        }
        if let Some(path) = installed.extras.values().next().cloned() {
            return Some(path);
        }
    }
    find_path_binary_impl(if cfg!(target_os = "windows") {
        &["lyrebird.exe", "snowflake-client.exe", "lyrebird", "snowflake-client"]
    } else {
        &["lyrebird", "snowflake-client"]
    })
}

pub(crate) fn ensure_network_runtime_tools_impl(
    app_handle: &AppHandle,
    required: &BTreeSet<NetworkTool>,
) -> Result<(), String> {
    eprintln!(
        "[network-runtime][trace] step=ensure-tools-enter required={:?}",
        required
    );
    if required.is_empty() {
        return Ok(());
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone())?;
    for tool in required {
        eprintln!("[network-runtime][trace] step=tool-check tool={}", tool.as_key());
        if tool_is_resolved_without_download_impl(app_handle, *tool) {
            eprintln!(
                "[network-runtime][trace] step=tool-resolved-without-download tool={}",
                tool.as_key()
            );
            continue;
        }
        eprintln!(
            "[network-runtime][trace] step=tool-ensure-start tool={}",
            tool.as_key()
        );
        ensure_network_tool_with_lock_impl(app_handle, &state, &runtime, *tool)?;
        eprintln!(
            "[network-runtime][trace] step=tool-ensure-done tool={}",
            tool.as_key()
        );
    }
    eprintln!("[network-runtime][trace] step=ensure-tools-exit");
    Ok(())
}

pub(crate) fn tool_is_resolved_without_download_impl(app_handle: &AppHandle, tool: NetworkTool) -> bool {
    match tool {
        NetworkTool::SingBox => resolve_sing_box_binary_path_impl(app_handle).is_ok(),
        NetworkTool::OpenVpn => resolve_openvpn_binary_path_impl(app_handle).is_ok(),
        NetworkTool::AmneziaWg => resolve_amneziawg_binary_path_impl(app_handle).is_ok(),
        NetworkTool::TorBundle => {
            resolve_tor_binary_path_impl(app_handle).is_some()
                && resolve_tor_pt_binary_path_impl(app_handle, "obfs4").is_some()
                && resolve_tor_pt_binary_path_impl(app_handle, "snowflake").is_some()
        }
    }
}

fn ensure_network_tool_with_lock_impl(
    app_handle: &AppHandle,
    state: &AppState,
    runtime: &NetworkRuntime,
    tool: NetworkTool,
) -> Result<(), String> {
    let key = tool.as_key().to_string();
    loop {
        let started_here = {
            let mut active = state
                .active_network_downloads
                .lock()
                .map_err(|_| "network download lock poisoned".to_string())?;
            if active.contains(&key) {
                false
            } else {
                active.insert(key.clone());
                true
            }
        };
        if started_here {
            let handle = app_handle.clone();
            let ensure_result = runtime.ensure_ready(tool, |progress| {
                let _ = handle.emit("network-runtime-progress", progress);
            });
            if let Err(error) = &ensure_result {
                let _ = app_handle.emit(
                    "network-runtime-progress",
                    NetworkRuntimeProgress::stage(tool, "error", Some(error.to_string())),
                );
            }
            let mut active = state
                .active_network_downloads
                .lock()
                .map_err(|_| "network download lock poisoned".to_string())?;
            active.remove(&key);
            return ensure_result.map(|_| ());
        }
        if runtime.installed(tool)?.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn find_path_binary_impl(candidates: &[&str]) -> Option<PathBuf> {
    for candidate in candidates {
        if let Ok(output) = hidden_command(candidate).arg("--version").output() {
            if output.status.success() {
                return Some(PathBuf::from(candidate));
            }
        }
    }
    None
}
