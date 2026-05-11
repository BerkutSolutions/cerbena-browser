use super::*;

pub(crate) fn load_global_security_record_impl(
    state: &AppState,
) -> Result<GlobalSecuritySettingsRecord, String> {
    let path = state.global_security_store_path(&state.app_handle)?;
    let legacy_path = state.global_security_legacy_path();
    load_global_security_record_from_paths_impl(&path, &legacy_path, &state.sensitive_store_secret)
}

pub(crate) fn load_global_security_record_from_paths_impl(
    path: &Path,
    legacy_path: &Path,
    secret_material: &str,
) -> Result<GlobalSecuritySettingsRecord, String> {
    let mut record = GlobalSecuritySettingsRecord {
        startup_page: None,
        certificates: Vec::new(),
        blocked_domain_suffixes: Vec::new(),
        blocklists: Vec::new(),
    };
    if path.exists() {
        record = crate::sensitive_store::load_sensitive_json_or_default(
            path,
            "global-security-store",
            secret_material,
        )?;
    } else if legacy_path.exists() {
        let raw = std::fs::read_to_string(legacy_path).map_err(|e| e.to_string())?;
        let parsed: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        record = parse_global_security_record_from_value_impl(&parsed);
    }
    record.blocklists = merge_default_dns_blocklists_impl(record.blocklists);
    Ok(record)
}

pub(crate) fn persist_global_security_record_impl(
    state: &AppState,
    payload: &GlobalSecuritySettingsRecord,
) -> Result<(), String> {
    let path = state.global_security_store_path(&state.app_handle)?;
    let legacy_path = state.global_security_legacy_path();
    persist_global_security_record_to_paths_impl(
        &path,
        &legacy_path,
        &state.sensitive_store_secret,
        payload,
    )
}

pub(crate) fn persist_global_security_record_to_paths_impl(
    path: &Path,
    legacy_path: &Path,
    secret_material: &str,
    payload: &GlobalSecuritySettingsRecord,
) -> Result<(), String> {
    crate::sensitive_store::persist_sensitive_json(
        path,
        "global-security-store",
        secret_material,
        payload,
    )?;
    if legacy_path.exists() {
        let _ = std::fs::remove_file(legacy_path);
    }
    Ok(())
}

pub(crate) fn parse_global_security_record_from_value_impl(
    parsed: &Value,
) -> GlobalSecuritySettingsRecord {
    GlobalSecuritySettingsRecord {
        startup_page: parsed
            .get("startup_page")
            .or_else(|| parsed.get("startupPage"))
            .and_then(Value::as_str)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        certificates: if let Some(items) = parsed.get("certificates").and_then(Value::as_array) {
            if items.iter().all(Value::is_string) {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|path| ManagedCertificateRecord {
                        id: slugify_impl(path),
                        name: certificate_name_from_path_impl(path),
                        path: path.trim().to_string(),
                        issuer_name: None,
                        subject_name: None,
                        apply_globally: true,
                        profile_ids: Vec::new(),
                    })
                    .collect()
            } else {
                serde_json::from_value::<Vec<ManagedCertificateRecord>>(json!(items))
                    .unwrap_or_default()
            }
        } else {
            Vec::new()
        },
        blocked_domain_suffixes: normalize_suffixes_impl(
            parsed
                .get("blocked_domain_suffixes")
                .or_else(|| parsed.get("blockedDomainSuffixes"))
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        ),
        blocklists: serde_json::from_value::<Vec<ManagedBlocklistRecord>>(
            parsed
                .get("blocklists")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .unwrap_or_default(),
    }
}

pub(crate) fn normalize_suffixes_impl(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|v| v.trim().trim_start_matches('.').to_lowercase())
        .filter(|v| !v.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn slugify_impl(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

pub(crate) fn certificate_name_from_path_impl(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|v| v.to_str())
        .map(|v| v.to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| path.to_string())
}

pub(crate) fn now_unix_ms_impl() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}