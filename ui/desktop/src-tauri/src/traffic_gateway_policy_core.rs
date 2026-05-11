use super::*;

pub(crate) fn evaluate_request(
    app_handle: &AppHandle,
    profile_id: Uuid,
    host: &str,
) -> GatewayDecision {
    let normalized = normalize_domain(host);
    let state = app_handle.state::<AppState>();
    let route_policy = crate::traffic_gateway::current_route_policy(app_handle, profile_id);
    let route = current_route_mode(app_handle, profile_id, route_policy.as_ref());

    if let Some(reason) = route_kill_switch_reason(app_handle, profile_id, &route_policy) {
        return GatewayDecision {
            blocked: true,
            reason,
            route,
            blocked_globally: false,
            blocked_for_profile: false,
        };
    }

    if let Ok(gateway) = state.traffic_gateway.lock() {
        if gateway
            .rules
            .global_blocked_domains
            .iter()
            .any(|rule| host_matches(&normalized, rule))
        {
            return GatewayDecision {
                blocked: true,
                reason: "User global rule".to_string(),
                route,
                blocked_globally: true,
                blocked_for_profile: false,
            };
        }
        if gateway
            .rules
            .profile_blocked_domains
            .get(&profile_id.to_string())
            .map(|rules| rules.iter().any(|rule| host_matches(&normalized, rule)))
            .unwrap_or(false)
        {
            return GatewayDecision {
                blocked: true,
                reason: "User profile rule".to_string(),
                route,
                blocked_globally: false,
                blocked_for_profile: true,
            };
        }
    }

    if let Some(reason) = dns_block_reason(&state, profile_id, &normalized) {
        return GatewayDecision {
            blocked: true,
            reason,
            route,
            blocked_globally: false,
            blocked_for_profile: false,
        };
    }

    if let Some(reason) = global_security_block_reason(&state, &normalized) {
        return GatewayDecision {
            blocked: true,
            reason,
            route,
            blocked_globally: false,
            blocked_for_profile: false,
        };
    }

    GatewayDecision {
        blocked: false,
        reason: "Allowed".to_string(),
        route,
        blocked_globally: false,
        blocked_for_profile: false,
    }
}

fn route_kill_switch_reason(
    app_handle: &AppHandle,
    profile_id: Uuid,
    route_policy: &Option<VpnProxyTabPayload>,
) -> Option<String> {
    let state = app_handle.state::<AppState>();
    let profile_route_mode = profile_route_mode(&state, profile_id);
    if profile_route_mode == "direct" {
        return None;
    }
    let (global_vpn_enabled, block_without_vpn) = state
        .network_store
        .lock()
        .ok()
        .map(|store| {
            (
                store.global_route_settings.global_vpn_enabled,
                store.global_route_settings.block_without_vpn,
            )
        })
        .unwrap_or((false, false));
    if global_vpn_enabled || block_without_vpn {
        return if runtime_session_active(app_handle, profile_id) {
            None
        } else if global_vpn_enabled {
            Some("Kill-switch: global VPN route is unavailable".to_string())
        } else {
            Some("Kill-switch: VPN tunnel is required by global policy".to_string())
        };
    }

    let policy = route_policy.as_ref()?;
    if !policy.kill_switch_enabled {
        return None;
    }
    let cache_key = format!(
        "{}:{}",
        profile_id,
        serde_json::to_string(policy).unwrap_or_else(|_| policy.route_mode.clone())
    );
    let now = now_epoch_ms();

    {
        let state = app_handle.state::<AppState>();
        let lock_result = state.traffic_gateway.lock();
        if let Ok(gateway) = lock_result {
            if let Some(cached) = gateway.route_health_cache.get(&cache_key) {
                if now.saturating_sub(cached.checked_at_ms) < ROUTE_HEALTH_TTL_MS {
                    return cached.blocked_reason.clone();
                }
            }
        };
    };

    let computed = compute_route_kill_switch_reason(app_handle, profile_id, policy);
    {
        let state = app_handle.state::<AppState>();
        let lock_result = state.traffic_gateway.lock();
        if let Ok(mut gateway) = lock_result {
            gateway.route_health_cache.insert(
                cache_key,
                RouteHealthCacheEntry {
                    checked_at_ms: now,
                    blocked_reason: computed.clone(),
                },
            );
            if gateway.route_health_cache.len() > 128 {
                let cutoff = now.saturating_sub(ROUTE_HEALTH_TTL_MS * 2);
                gateway
                    .route_health_cache
                    .retain(|_, entry| entry.checked_at_ms >= cutoff);
            }
        };
    };
    computed
}

fn compute_route_kill_switch_reason(
    app_handle: &AppHandle,
    profile_id: Uuid,
    policy: &VpnProxyTabPayload,
) -> Option<String> {
    if let Some(strategy) = resolved_route_strategy(app_handle, profile_id) {
        match strategy {
            ResolvedNetworkSandboxMode::Blocked => {
                let state = app_handle.state::<AppState>();
                let reason = resolve_sandbox_strategy_reason(state.inner(), profile_id)
                    .unwrap_or_else(|| {
                        "selected isolated route policy blocks this backend".to_string()
                    });
                return Some(format!("Kill-switch: {reason}"));
            }
            ResolvedNetworkSandboxMode::Container => {
                if !resolved_sandbox_adapter_available(app_handle, profile_id) {
                    let state = app_handle.state::<AppState>();
                    let reason = resolve_sandbox_adapter_reason(state.inner(), profile_id)
                        .unwrap_or_else(|| {
                            "container sandbox mode is selected, but the adapter is not available"
                                .to_string()
                        });
                    return Some(format!("Kill-switch: {reason}"));
                }
            }
            _ => {}
        }
    }
    let runtime_required = route_runtime_required_for_profile(app_handle, profile_id);
    let runtime_active = runtime_session_active(app_handle, profile_id);
    if runtime_required && !runtime_active {
        let strategy_label = resolved_route_strategy(app_handle, profile_id)
            .map(|value| value.as_str().to_string())
            .unwrap_or_else(|| "runtime".to_string());
        return Some(format!(
            "Kill-switch: selected {strategy_label} route runtime is unavailable"
        ));
    }
    if runtime_active {
        return None;
    }
    match policy.route_mode.trim().to_lowercase().as_str() {
        "direct" => None,
        "proxy" => proxy_unavailable_reason(policy.proxy.as_ref()),
        "vpn" => vpn_unavailable_reason(policy.vpn.as_ref()),
        "hybrid" => proxy_unavailable_reason(policy.proxy.as_ref())
            .or_else(|| vpn_unavailable_reason(policy.vpn.as_ref())),
        "tor" => tor_unavailable_reason(app_handle, profile_id),
        _ => Some("Kill-switch: invalid route mode in policy".to_string()),
    }
}


#[path = "traffic_gateway_policy_core_support.rs"]
mod support;
pub(crate) use support::*;


