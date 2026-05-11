use super::*;

pub(crate) fn launch_sing_box_container_runtime_impl(
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

