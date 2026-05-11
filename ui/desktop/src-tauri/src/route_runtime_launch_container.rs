use super::*;

pub(crate) fn try_launch_container_runtime_impl(
    app_handle: &AppHandle,
    profile_id: Uuid,
    runtime_dir: &PathBuf,
    nodes: &[NormalizedNode],
    uses_container_runtime: bool,
    uses_openvpn: bool,
    uses_amnezia_container: bool,
) -> Result<Option<RouteRuntimeSession>, String> {
    if uses_amnezia_container {
        let node = nodes
            .first()
            .ok_or_else(|| "amnezia container runtime requires one node".to_string())?;
        let key = node
            .settings
            .get("amneziaKey")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "amnezia key is required".to_string())?;
        let config_text = amnezia::build_amnezia_native_config_text_impl(key)?;
        let host_proxy_port = reserve_local_port()?;
        let launch = launch_amnezia_container_runtime(
            app_handle,
            profile_id,
            runtime_dir,
            host_proxy_port,
            &config_text,
        )?;
        return Ok(Some(RouteRuntimeSession {
            signature: String::new(),
            pid: None,
            backend: RouteRuntimeBackend::ContainerSocks,
            listen_port: Some(launch.host_proxy_port),
            config_path: launch.config_path,
            cleanup_paths: launch.cleanup_paths,
            tunnel_name: None,
            container_name: Some(launch.container_name),
        }));
    }

    if !uses_container_runtime {
        return Ok(None);
    }

    let host_proxy_port = reserve_local_port()?;
    if uses_openvpn {
        let node = nodes
            .first()
            .ok_or_else(|| "openvpn container runtime requires one node".to_string())?;
        let auth_path = build_openvpn_auth_file(node, runtime_dir, profile_id)?;
        let container_log_path = PathBuf::from("/work/route.log");
        let container_auth_path = auth_path
            .as_ref()
            .map(|_| PathBuf::from("/work/openvpn-auth.txt"));
        let config_text =
            build_openvpn_config_text(node, container_auth_path.as_ref(), &container_log_path)?;
        let launch = launch_openvpn_container_runtime(
            app_handle,
            profile_id,
            runtime_dir,
            host_proxy_port,
            &config_text,
            auth_path.as_ref(),
        )?;
        return Ok(Some(RouteRuntimeSession {
            signature: String::new(),
            pid: None,
            backend: RouteRuntimeBackend::ContainerSocks,
            listen_port: Some(launch.host_proxy_port),
            config_path: launch.config_path,
            cleanup_paths: launch.cleanup_paths,
            tunnel_name: None,
            container_name: Some(launch.container_name),
        }));
    }

    let container_log_path = PathBuf::from("/work/container-route.log");
    let config = build_runtime_config(
        app_handle,
        nodes,
        CONTAINER_PROXY_PORT,
        &container_log_path,
        RuntimeExecutionTarget::Container,
    )?;
    let config_json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    let launch = launch_sing_box_container_runtime(
        app_handle,
        profile_id,
        runtime_dir,
        host_proxy_port,
        &config_json,
    )?;
    Ok(Some(RouteRuntimeSession {
        signature: String::new(),
        pid: None,
        backend: RouteRuntimeBackend::ContainerSocks,
        listen_port: Some(launch.host_proxy_port),
        config_path: launch.config_path,
        cleanup_paths: launch.cleanup_paths,
        tunnel_name: None,
        container_name: Some(launch.container_name),
    }))
}
