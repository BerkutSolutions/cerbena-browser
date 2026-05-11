use super::*;

pub(crate) fn launch_openvpn_container_runtime_impl(
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

