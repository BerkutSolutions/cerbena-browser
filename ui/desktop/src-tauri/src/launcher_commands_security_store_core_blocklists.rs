use super::*;

pub(crate) fn normalize_blocklists_impl(
    state: &AppState,
    items: Vec<ManagedBlocklistInput>,
    existing: &[ManagedBlocklistRecord],
) -> Result<Vec<ManagedBlocklistRecord>, String> {
    let updater = DnsBlocklistUpdater::new();
    let mut out = Vec::new();
    let mut seen_ids = std::collections::BTreeSet::new();
    let mut seen_sources = std::collections::BTreeSet::new();
    let defaults_applied = merge_default_dns_blocklist_inputs_impl(items);
    let existing_by_id = existing
        .iter()
        .map(|item| (item.id.clone(), item.clone()))
        .collect::<BTreeMap<_, _>>();
    let existing_by_source = existing
        .iter()
        .map(|item| {
            (
                blocklist_source_key_impl(&item.source_kind, &item.source_value),
                item.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let active_total = defaults_applied
        .iter()
        .filter(|item| item.active)
        .count()
        .max(1);
    let mut processed_active = 0usize;
    let started_at = std::time::Instant::now();
    for item in defaults_applied {
        let source_kind = normalize_source_kind_impl(&item.source_kind);
        let source_value = item.source_value.trim().to_string();
        if matches!(source_kind.as_str(), "url" | "file") && source_value.is_empty() {
            continue;
        }
        let id_seed = if item.id.trim().is_empty() {
            if source_value.is_empty() {
                item.name.trim().to_string()
            } else {
                source_value.clone()
            }
        } else {
            item.id.clone()
        };
        let id = slugify_impl(&id_seed);
        if id.is_empty() || !seen_ids.insert(id.clone()) {
            continue;
        }
        let source_key = blocklist_source_key_impl(&source_kind, &source_value);
        if !seen_sources.insert(source_key.clone()) {
            continue;
        }
        let previous = existing_by_id
            .get(&id)
            .or_else(|| existing_by_source.get(&source_key));

        let fallback_name = if item.name.trim().is_empty() {
            previous
                .map(|record| record.name.clone())
                .unwrap_or_else(|| fallback_blocklist_name_impl(&source_value))
        } else {
            item.name.trim().to_string()
        };

        if item.active {
            let source = blocklist_source_from_fields_impl(&source_kind, &source_value, &item.domains)?;
            let _ = state.app_handle.emit(
                "dns-blocklist-progress",
                json!({
                    "stage": "downloading",
                    "name": fallback_name,
                    "progress": if active_total == 0 { 0.0 } else { (processed_active as f64 / active_total as f64) * 100.0 },
                    "processed": processed_active,
                    "total": active_total,
                    "elapsedSeconds": started_at.elapsed().as_secs_f64()
                }),
            );
            let snapshot = updater
                .update_from_source(&id, &source)
                .map_err(|e| e.to_string())?;
            processed_active += 1;
            let resolved_name = resolve_blocklist_title_impl(&source_kind, &source_value)
                .unwrap_or_else(|| fallback_name.clone());
            out.push(ManagedBlocklistRecord {
                id,
                name: resolved_name,
                source_kind: source_kind.clone(),
                source_value: source_value.clone(),
                active: true,
                domains: snapshot.domains,
                updated_at_epoch: snapshot.updated_at_epoch,
            });
            let _ = state.app_handle.emit(
                "dns-blocklist-progress",
                json!({
                    "stage": "downloading",
                    "name": out.last().map(|value| value.name.clone()).unwrap_or_default(),
                    "progress": (processed_active as f64 / active_total as f64) * 100.0,
                    "processed": processed_active,
                    "total": active_total,
                    "elapsedSeconds": started_at.elapsed().as_secs_f64()
                }),
            );
            continue;
        }

        let domains = previous
            .map(|record| record.domains.clone())
            .filter(|values| !values.is_empty())
            .unwrap_or_else(|| normalize_inline_domains_impl(item.domains));
        let updated_at_epoch = previous.map(|record| record.updated_at_epoch).unwrap_or(0);
        out.push(ManagedBlocklistRecord {
            id,
            name: fallback_name,
            source_kind,
            source_value,
            active: false,
            domains,
            updated_at_epoch,
        });
    }
    let _ = state.app_handle.emit(
        "dns-blocklist-progress",
        json!({
            "stage": "completed",
            "name": out.last().map(|value| value.name.clone()).unwrap_or_default(),
            "progress": 100.0,
            "processed": processed_active,
            "total": active_total,
            "elapsedSeconds": started_at.elapsed().as_secs_f64()
        }),
    );
    Ok(out)
}

pub(crate) fn merge_default_dns_blocklist_inputs_impl(
    mut items: Vec<ManagedBlocklistInput>,
) -> Vec<ManagedBlocklistInput> {
    let mut seen_sources = items
        .iter()
        .map(|item| blocklist_source_key_impl(&item.source_kind, &item.source_value))
        .collect::<std::collections::BTreeSet<_>>();
    for (name, url) in DEFAULT_DNS_BLOCKLISTS {
        let source_key = blocklist_source_key_impl("url", url);
        if seen_sources.insert(source_key) {
            items.push(ManagedBlocklistInput {
                id: slugify_impl(url),
                name: (*name).to_string(),
                source_kind: "url".to_string(),
                source_value: (*url).to_string(),
                active: false,
                domains: Vec::new(),
            });
        }
    }
    items
}

pub(crate) fn merge_default_dns_blocklists_impl(
    existing: Vec<ManagedBlocklistRecord>,
) -> Vec<ManagedBlocklistRecord> {
    let mut by_source = existing
        .into_iter()
        .map(|item| {
            (
                blocklist_source_key_impl(&item.source_kind, &item.source_value),
                item,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut merged = Vec::new();
    for (name, url) in DEFAULT_DNS_BLOCKLISTS {
        let key = blocklist_source_key_impl("url", url);
        if let Some(mut current) = by_source.remove(&key) {
            current.id = if current.id.trim().is_empty() {
                slugify_impl(url)
            } else {
                slugify_impl(&current.id)
            };
            current.source_kind = "url".to_string();
            current.source_value = (*url).to_string();
            if current.name.trim().is_empty() {
                current.name = (*name).to_string();
            }
            merged.push(current);
        } else {
            merged.push(ManagedBlocklistRecord {
                id: slugify_impl(url),
                name: (*name).to_string(),
                source_kind: "url".to_string(),
                source_value: (*url).to_string(),
                active: false,
                domains: Vec::new(),
                updated_at_epoch: 0,
            });
        }
    }
    merged.extend(by_source.into_values());
    merged
}

pub(crate) fn normalize_source_kind_impl(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "url" => "url".to_string(),
        "file" => "file".to_string(),
        _ => "inline".to_string(),
    }
}

pub(crate) fn blocklist_source_key_impl(source_kind: &str, source_value: &str) -> String {
    format!(
        "{}:{}",
        normalize_source_kind_impl(source_kind),
        source_value.trim().to_ascii_lowercase()
    )
}

pub(crate) fn blocklist_source_from_fields_impl(
    source_kind: &str,
    source_value: &str,
    domains: &[String],
) -> Result<BlocklistSource, String> {
    match source_kind {
        "url" => {
            if source_value.trim().is_empty() {
                return Err("blocklist URL is required".to_string());
            }
            Ok(BlocklistSource::RemoteUrl {
                url: source_value.to_string(),
                require_https: true,
                expected_sha256: None,
            })
        }
        "file" => {
            if source_value.trim().is_empty() {
                return Err("blocklist file path is required".to_string());
            }
            Ok(BlocklistSource::LocalFile {
                path: std::path::PathBuf::from(source_value),
            })
        }
        _ => Ok(BlocklistSource::InlineDomains {
            domains: normalize_inline_domains_impl(domains.to_vec()),
        }),
    }
}

pub(crate) fn normalize_inline_domains_impl(domains: Vec<String>) -> Vec<String> {
    domains
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn fallback_blocklist_name_impl(source_value: &str) -> String {
    let trimmed = source_value.trim();
    if trimmed.is_empty() {
        return "DNS blocklist".to_string();
    }
    if let Ok(url) = reqwest::Url::parse(trimmed) {
        if let Some(segment) = url
            .path_segments()
            .and_then(|segments| segments.filter(|item| !item.is_empty()).last())
        {
            return segment.to_string();
        }
    }
    std::path::Path::new(trimmed)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| trimmed.to_string())
}

pub(crate) fn resolve_blocklist_title_impl(source_kind: &str, source_value: &str) -> Option<String> {
    let text = match source_kind {
        "url" => {
            let url = reqwest::Url::parse(source_value.trim()).ok()?;
            let client = reqwest::blocking::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(8))
                .timeout(std::time::Duration::from_secs(20))
                .user_agent("Cerbena/0.1")
                .build()
                .ok()?;
            let response = client.get(url).send().ok()?;
            if !response.status().is_success() {
                return None;
            }
            response.text().ok()?
        }
        "file" => std::fs::read_to_string(source_value.trim()).ok()?,
        _ => return None,
    };
    extract_blocklist_title_impl(&text)
}

pub(crate) fn extract_blocklist_title_impl(content: &str) -> Option<String> {
    for raw_line in content.lines().take(120) {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let mut stripped = line;
        if stripped.starts_with('!') || stripped.starts_with('#') {
            stripped = stripped[1..].trim_start();
        }
        let lower = stripped.to_ascii_lowercase();
        if !lower.starts_with("title:") {
            continue;
        }
        let value = stripped
            .split_once(':')
            .map(|(_, right)| right.trim())
            .unwrap_or_default();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}