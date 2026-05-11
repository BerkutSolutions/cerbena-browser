pub(crate) fn resolve_effective_route_selection_impl(
    store: &crate::state::NetworkStore,
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
