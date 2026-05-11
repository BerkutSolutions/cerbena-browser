use super::*;

pub(crate) fn launch_host_singbox_runtime_impl(
    app_handle: &AppHandle,
    runtime_dir: &PathBuf,
    nodes: &[NormalizedNode],
    sing_box_log_path: &PathBuf,
) -> Result<RouteRuntimeSession, String> {
    let local_port = reserve_local_port()?;
    let _ = fs::remove_file(sing_box_log_path);
    let config = build_runtime_config(
        app_handle,
        nodes,
        local_port,
        sing_box_log_path,
        RuntimeExecutionTarget::Host,
    )?;
    let config_path = runtime_dir.join("sing-box-route.json");
    let config_bytes = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&config_path, config_bytes).map_err(|e| format!("write route runtime config: {e}"))?;

    let binary = resolve_sing_box_binary_path(app_handle)?
        .to_string_lossy()
        .to_string();
    run_sing_box_check(&binary, &config_path, sing_box_log_path)?;
    let mut command = hidden_command(&binary);
    command.arg("run").arg("-c").arg(&config_path);
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn sing-box route runtime failed: {e}"))?;
    let pid = child.id();
    if pid == 0 {
        return Err("route runtime spawn failed: empty pid".to_string());
    }
    thread::sleep(Duration::from_millis(300));
    if let Some(status) = child
        .try_wait()
        .map_err(|e| format!("route runtime process status check failed: {e}"))?
    {
        let code = status
            .code()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "terminated".to_string());
        let log = fs::read_to_string(sing_box_log_path).unwrap_or_default();
        let summary = tail_lines(&log, 20);
        return Err(format!(
            "route runtime exited immediately (code {code}){}",
            if summary.is_empty() {
                String::new()
            } else {
                format!(": {summary}")
            }
        ));
    }

    Ok(RouteRuntimeSession {
        signature: String::new(),
        pid: Some(pid),
        backend: RouteRuntimeBackend::SingBox,
        listen_port: Some(local_port),
        config_path,
        cleanup_paths: vec![sing_box_log_path.clone()],
        tunnel_name: None,
        container_name: None,
    })
}
