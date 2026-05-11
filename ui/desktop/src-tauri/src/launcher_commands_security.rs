use super::*;

pub(crate) fn get_global_security_settings(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let data = load_global_security_record(&state)?;
    Ok(ok(
        correlation_id,
        serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?,
    ))
}

pub(crate) fn save_global_security_settings(
    state: State<AppState>,
    request: GlobalSecuritySettingsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let existing = load_global_security_record(&state)?;
    let payload = GlobalSecuritySettingsRecord {
        startup_page: request
            .startup_page
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        certificates: security_store::normalize_certificates_impl(
            &state,
            request.certificates,
            &existing.certificates,
        )?,
        blocked_domain_suffixes: security_store::normalize_suffixes_impl(
            request.blocked_domain_suffixes,
        ),
        blocklists: security_store::normalize_blocklists_impl(
            &state,
            request.blocklists,
            &existing.blocklists,
        )?,
    };
    persist_global_security_record(&state, &payload)?;
    security_store::cleanup_unused_managed_certificates_impl(
        &state,
        &existing.certificates,
        &payload.certificates,
    );

    if let Some(start_page) = payload.startup_page.clone() {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "manager lock poisoned".to_string())?;
        let profiles = manager.list_profiles().map_err(|e| e.to_string())?;
        drop(manager);
        for profile in profiles {
            if profile.default_start_page.is_some() {
                continue;
            }
            let manager = state
                .manager
                .lock()
                .map_err(|_| "manager lock poisoned".to_string())?;
            let _ = manager.update_profile(
                profile.id,
                browser_profile::PatchProfileInput {
                    default_start_page: Some(Some(start_page.to_string())),
                    ..browser_profile::PatchProfileInput::default()
                },
            );
        }
    }
    Ok(ok(correlation_id, true))
}
