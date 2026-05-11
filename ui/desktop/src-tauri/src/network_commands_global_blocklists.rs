use super::*;

#[derive(Debug, Clone)]
pub(crate) struct GlobalBlocklistRecord {
    pub(crate) source_kind: String,
    pub(crate) source_value: String,
    pub(crate) domains: Vec<String>,
    pub(crate) updated_at_epoch: u64,
}

pub(crate) fn hydrate_dns_blocklists_from_global_security_impl(
    state: &AppState,
    payload: &mut DnsTabPayload,
) -> Result<(), String> {
    if payload.selected_blocklists.is_empty() {
        return Ok(());
    }
    let records = load_global_security_blocklists_impl(state)?;
    if records.is_empty() {
        return Ok(());
    }
    let updater = DnsBlocklistUpdater::new();
    for list in &mut payload.selected_blocklists {
        if !list.domains.is_empty() {
            continue;
        }
        let Some(record) = records.get(&list.list_id) else {
            continue;
        };
        if !record.domains.is_empty() {
            list.domains = normalize_blocklist_domains_impl(record.domains.clone());
            if list.updated_at_epoch == 0 {
                list.updated_at_epoch = record.updated_at_epoch;
            }
            continue;
        }
        let source = global_blocklist_source_impl(record)?;
        let snapshot = updater
            .update_from_source(&list.list_id, &source)
            .map_err(|e| e.to_string())?;
        list.domains = snapshot.domains;
        list.updated_at_epoch = snapshot.updated_at_epoch;
    }
    Ok(())
}

pub(crate) fn load_global_security_blocklists_impl(
    state: &AppState,
) -> Result<BTreeMap<String, GlobalBlocklistRecord>, String> {
    let items = load_global_security_record(state)?.blocklists;
    let mut out = BTreeMap::new();
    for item in items {
        let id = item.id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        out.insert(
            id.clone(),
            GlobalBlocklistRecord {
                source_kind: item.source_kind,
                source_value: item.source_value,
                domains: item.domains,
                updated_at_epoch: item.updated_at_epoch,
            },
        );
    }
    Ok(out)
}

pub(crate) fn global_blocklist_source_impl(
    record: &GlobalBlocklistRecord,
) -> Result<BlocklistSource, String> {
    match record.source_kind.as_str() {
        "url" => {
            if record.source_value.trim().is_empty() {
                return Err("global blocklist URL is empty".to_string());
            }
            Ok(BlocklistSource::RemoteUrl {
                url: record.source_value.clone(),
                require_https: true,
                expected_sha256: None,
            })
        }
        "file" => {
            if record.source_value.trim().is_empty() {
                return Err("global blocklist file path is empty".to_string());
            }
            Ok(BlocklistSource::LocalFile {
                path: std::path::PathBuf::from(&record.source_value),
            })
        }
        _ => Ok(BlocklistSource::InlineDomains {
            domains: record.domains.clone(),
        }),
    }
}

pub(crate) fn normalize_blocklist_domains_impl(domains: Vec<String>) -> Vec<String> {
    let mut unique = std::collections::BTreeSet::new();
    for domain in domains {
        let normalized = domain.trim().to_lowercase();
        if !normalized.is_empty() {
            unique.insert(normalized);
        }
    }
    unique.into_iter().collect()
}
