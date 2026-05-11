use super::*;

pub(crate) fn launch_amnezia_container_runtime_impl(
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

