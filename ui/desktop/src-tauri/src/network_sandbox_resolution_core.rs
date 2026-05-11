use super::*;

pub(crate) fn record_resolved_mode(
    state: &AppState,
    profile_key: &str,
    mode: &str,
    reason: &str,
) -> Result<(), String> {
    let mut store = state
        .network_sandbox_store
        .lock()
        .map_err(|_| "network sandbox store lock poisoned".to_string())?;
    let entry = store.profiles.entry(profile_key.to_string()).or_default();
    let mut changed = false;
    if entry.last_resolved_mode.as_deref() != Some(mode) {
        entry.last_resolved_mode = Some(mode.to_string());
        changed = true;
    }
    if entry.last_resolution_reason.as_deref() != Some(reason) {
        entry.last_resolution_reason = Some(reason.to_string());
        changed = true;
    }
    if changed {
        let path = state.network_sandbox_store_path(&state.app_handle)?;
        persist_network_sandbox_store(&path, &store)?;
    }
    Ok(())
}

pub(crate) fn normalize_global_settings(settings: &mut NetworkSandboxGlobalSettings) {
    settings.default_mode = normalize_mode(Some(settings.default_mode.clone()))
        .unwrap_or_else(|| MODE_AUTO.to_string());
    if settings.target_runtime.trim().is_empty() {
        settings.target_runtime = "launcher-managed".to_string();
    }
    if settings.max_active_sandboxes == 0 {
        settings.max_active_sandboxes = 2;
    }
}

pub(crate) fn resolve_requested_mode(
    global: &NetworkSandboxGlobalSettings,
    profile: &NetworkSandboxProfileSettings,
) -> String {
    profile.preferred_mode.clone().unwrap_or_else(|| {
        if global.enabled {
            normalize_mode(Some(global.default_mode.clone()))
                .unwrap_or_else(|| MODE_AUTO.to_string())
        } else {
            MODE_AUTO.to_string()
        }
    })
}

pub(crate) fn resolve_network_sandbox_strategy_for_modes(
    global: &NetworkSandboxGlobalSettings,
    profile: &NetworkSandboxProfileSettings,
    requested_mode: String,
    requires_native: bool,
    container_supported: bool,
) -> ResolvedNetworkSandboxStrategy {
    if requested_mode == MODE_CONTAINER && !container_supported {
        return ResolvedNetworkSandboxStrategy {
            mode: ResolvedNetworkSandboxMode::Blocked,
            requested_mode,
            requires_native_backend: requires_native,
            available: false,
            reason: "Selected route is not compatible with container isolation yet".to_string(),
        };
    }
    let (mode, available, reason) = if !requires_native {
        let resolved = if requested_mode == MODE_CONTAINER {
            ResolvedNetworkSandboxMode::Container
        } else {
            ResolvedNetworkSandboxMode::IsolatedUserspace
        };
        (
            resolved,
            true,
            "Template is compatible with isolated userspace runtime".to_string(),
        )
    } else {
        match requested_mode.as_str() {
            MODE_COMPAT_NATIVE => (
                ResolvedNetworkSandboxMode::CompatibilityNative,
                true,
                "Profile is pinned to compatibility-native mode".to_string(),
            ),
            MODE_CONTAINER => (
                ResolvedNetworkSandboxMode::Container,
                true,
                "Container sandbox mode is selected; launcher will validate the host runtime and per-profile sandbox capacity during launch".to_string(),
            ),
            MODE_AUTO if profile.migrated_legacy_native => (
                ResolvedNetworkSandboxMode::CompatibilityNative,
                true,
                "Legacy profile was auto-adapted to compatibility-native mode".to_string(),
            ),
            MODE_AUTO if global.enabled && global.allow_native_compatibility_fallback => (
                ResolvedNetworkSandboxMode::CompatibilityNative,
                true,
                "Global sandbox policy allows compatibility-native fallback".to_string(),
            ),
            _ => (
                ResolvedNetworkSandboxMode::Blocked,
                false,
                "This Amnezia profile requires a machine-wide compatibility backend; isolated mode forbids that path".to_string(),
            ),
        }
    };
    ResolvedNetworkSandboxStrategy {
        mode,
        requested_mode,
        requires_native_backend: requires_native,
        available,
        reason,
    }
}

pub(crate) fn normalize_mode(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let normalized = raw.trim().to_lowercase();
        match normalized.as_str() {
            MODE_AUTO | MODE_ISOLATED | MODE_COMPAT_NATIVE | MODE_CONTAINER => Some(normalized),
            "" => None,
            _ => Some(MODE_AUTO.to_string()),
        }
    })
}

pub(crate) fn compatible_modes_for_template(requires_native: bool, container_supported: bool) -> Vec<String> {
    if requires_native {
        let mut modes = vec![MODE_COMPAT_NATIVE.to_string()];
        if container_supported {
            modes.push(MODE_CONTAINER.to_string());
        }
        modes
    } else {
        let mut modes = vec![MODE_ISOLATED.to_string()];
        if container_supported {
            modes.push(MODE_CONTAINER.to_string());
        }
        modes
    }
}

pub(crate) fn resolve_effective_route_selection(
    store: &NetworkStore,
    profile_key: &str,
) -> (String, Option<String>) {
    let profile_route_mode = store
        .vpn_proxy
        .get(profile_key)
        .map(|value| value.route_mode.trim().to_lowercase())
        .unwrap_or_else(|| "direct".to_string());
    if profile_route_mode == "direct" {
        return ("direct".to_string(), None);
    }
    if store.global_route_settings.global_vpn_enabled {
        let template_id = store
            .global_route_settings
            .default_template_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        return ("vpn".to_string(), template_id);
    }
    let template_id = store.profile_template_selection.get(profile_key).cloned();
    (profile_route_mode, template_id)
}

pub(crate) fn sandbox_view_for_preview(
    state: &AppState,
    profile_id: Option<&str>,
    preferred_mode: Option<String>,
    template: Option<&ConnectionTemplate>,
    global_scope: bool,
) -> Result<NetworkSandboxPreviewView, String> {
    let sandbox_store = state
        .network_sandbox_store
        .lock()
        .map_err(|_| "network sandbox store lock poisoned".to_string())?
        .clone();
    let profile_key = profile_id.unwrap_or("__preview__").to_string();
    let mut profile_settings = profile_id
        .and_then(|id| sandbox_store.profiles.get(id))
        .cloned()
        .unwrap_or_default();
    profile_settings.preferred_mode = normalize_mode(preferred_mode);
    let requires_native = template
        .map(template_requires_native_compatibility)
        .transpose()?
        .unwrap_or(false);
    let container_supported = template
        .map(template_supports_container_mode)
        .transpose()?
        .unwrap_or(true);
    let requested_mode = if global_scope {
        if sandbox_store.global.enabled {
            normalize_mode(
                profile_settings
                    .preferred_mode
                    .clone()
                    .or_else(|| Some(sandbox_store.global.default_mode.clone())),
            )
            .unwrap_or_else(|| MODE_AUTO.to_string())
        } else {
            MODE_AUTO.to_string()
        }
    } else {
        resolve_requested_mode(&sandbox_store.global, &profile_settings)
    };
    let resolution = resolve_network_sandbox_strategy_for_modes(
        &sandbox_store.global,
        &profile_settings,
        requested_mode,
        requires_native,
        container_supported,
    );
    let view = sandbox_profile_view(
        state,
        &sandbox_store,
        &profile_key,
        profile_id.and_then(|id| Uuid::parse_str(id).ok()),
        Some(&resolution),
    );
    Ok(NetworkSandboxPreviewView {
        sandbox: view,
        compatible_modes: compatible_modes_for_template(requires_native, container_supported),
        active_template_id: template.map(|item| item.id.clone()),
        route_mode: template
            .map(|_| "vpn".to_string())
            .unwrap_or_else(|| "direct".to_string()),
    })
}

pub(crate) fn profile_requires_legacy_native_compatibility(
    store: &NetworkStore,
    profile_key: &str,
) -> Result<bool, String> {
    let payload = store.vpn_proxy.get(profile_key);
    let route_mode = payload
        .map(|value| value.route_mode.trim().to_lowercase())
        .unwrap_or_else(|| "direct".to_string());
    if route_mode == "direct" {
        return Ok(false);
    }
    let Some(template_id) = store.profile_template_selection.get(profile_key) else {
        return Ok(false);
    };
    let Some(template) = store.connection_templates.get(template_id) else {
        return Ok(false);
    };
    template_requires_native_compatibility(template)
}

pub(crate) fn template_supports_container_mode(template: &ConnectionTemplate) -> Result<bool, String> {
    let nodes = if !template.nodes.is_empty() {
        template.nodes.clone()
    } else if !template.connection_type.trim().is_empty() && !template.protocol.trim().is_empty() {
        vec![crate::state::ConnectionNode {
            id: template.id.clone(),
            connection_type: template.connection_type.clone(),
            protocol: template.protocol.clone(),
            host: template.host.clone(),
            port: template.port,
            username: template.username.clone(),
            password: template.password.clone(),
            bridges: template.bridges.clone(),
            settings: BTreeMap::new(),
        }]
    } else {
        Vec::new()
    };
    if nodes.is_empty() {
        return Ok(false);
    }
    let single_node = nodes.len() == 1;
    Ok(nodes.iter().all(|node| {
        match (
            node.connection_type.trim().to_ascii_lowercase().as_str(),
            node.protocol.trim().to_ascii_lowercase().as_str(),
        ) {
            ("proxy", "http" | "socks4" | "socks5") => true,
            ("v2ray", "vmess" | "vless" | "trojan" | "shadowsocks") => true,
            ("vpn", "wireguard" | "amnezia") => true,
            ("vpn", "openvpn") => single_node,
            ("tor", "none" | "obfs4" | "snowflake" | "meek") => true,
            _ => false,
        }
    }))
}

pub(crate) fn template_requires_native_compatibility(template: &ConnectionTemplate) -> Result<bool, String> {
    let nodes = if !template.nodes.is_empty() {
        template.nodes.clone()
    } else if template.connection_type.trim().eq_ignore_ascii_case("vpn")
        && template.protocol.trim().eq_ignore_ascii_case("amnezia")
    {
        vec![crate::state::ConnectionNode {
            id: template.id.clone(),
            connection_type: template.connection_type.clone(),
            protocol: template.protocol.clone(),
            host: template.host.clone(),
            port: template.port,
            username: template.username.clone(),
            password: template.password.clone(),
            bridges: template.bridges.clone(),
            settings: BTreeMap::new(),
        }]
    } else {
        Vec::new()
    };

    if nodes.len() != 1 {
        return Ok(false);
    }
    let node = &nodes[0];
    if !node.connection_type.trim().eq_ignore_ascii_case("vpn")
        || !node.protocol.trim().eq_ignore_ascii_case("amnezia")
    {
        return Ok(false);
    }
    let Some(key) = node.settings.get("amneziaKey") else {
        return Ok(false);
    };
    amnezia_config_requires_native_backend(key)
}

