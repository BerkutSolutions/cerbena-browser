use super::*;

pub(crate) fn normalize_certificates_impl(
    state: &AppState,
    items: Vec<ManagedCertificateInput>,
    existing: &[ManagedCertificateRecord],
) -> Result<Vec<ManagedCertificateRecord>, String> {
    let root = state.managed_certificates_root(&state.app_handle)?;
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("create managed certificates dir: {error}"))?;
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let existing_by_id = existing
        .iter()
        .map(|item| (item.id.clone(), item.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    for item in items {
        let path = item.path.trim().to_string();
        if path.is_empty() || !seen.insert(path.clone()) {
            continue;
        }
        let id = if item.id.trim().is_empty() {
            slugify_impl(&path)
        } else {
            slugify_impl(&item.id)
        };
        let profile_ids = item
            .profile_ids
            .into_iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let existing_record = existing_by_id.get(&id);
        let managed_path = materialize_managed_certificate_impl(
            &root,
            &path,
            &id,
            existing_record.map(|record| record.path.as_str()),
        )?;
        let (subject_name, issuer_name) =
            load_certificate_metadata(&managed_path).unwrap_or((None, None));
        out.push(ManagedCertificateRecord {
            id,
            name: if item.name.trim().is_empty() {
                certificate_display_name_impl(subject_name.clone(), &managed_path.to_string_lossy())
            } else {
                item.name.trim().to_string()
            },
            path: managed_path.to_string_lossy().to_string(),
            issuer_name: display_certificate_issuer(issuer_name, subject_name.clone()),
            subject_name,
            apply_globally: item.apply_globally,
            profile_ids,
        });
    }
    Ok(out)
}

pub(crate) fn materialize_managed_certificate_impl(
    root: &Path,
    source_path: &str,
    id: &str,
    existing_managed_path: Option<&str>,
) -> Result<std::path::PathBuf, String> {
    let trimmed = source_path.trim();
    if trimmed.is_empty() {
        return Err("certificate path is required".to_string());
    }
    let source = Path::new(trimmed);
    if !source.exists() {
        return Err(format!("certificate file not found: {}", source.display()));
    }

    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().trim_start_matches('.'))
        .filter(|value| !value.is_empty())
        .unwrap_or("crt");
    let target = root.join(format!("{id}.{extension}"));
    if source == target {
        return Ok(target);
    }

    if let Some(existing_path) = existing_managed_path {
        let existing = Path::new(existing_path);
        if existing == source && existing.exists() {
            return Ok(existing.to_path_buf());
        }
    }
    std::fs::copy(source, &target)
        .map_err(|error| format!("copy certificate {}: {error}", source.display()))?;
    Ok(target)
}

pub(crate) fn cleanup_unused_managed_certificates_impl(
    state: &AppState,
    previous: &[ManagedCertificateRecord],
    next: &[ManagedCertificateRecord],
) {
    let Ok(root) = state.managed_certificates_root(&state.app_handle) else {
        return;
    };
    let keep = next
        .iter()
        .map(|item| item.path.trim().to_string())
        .collect::<std::collections::BTreeSet<_>>();
    for path in previous
        .iter()
        .map(|item| item.path.trim())
        .filter(|value| !value.is_empty())
    {
        let candidate = Path::new(path);
        if !candidate.starts_with(&root) {
            continue;
        }
        if keep.contains(path) {
            continue;
        }
        let _ = std::fs::remove_file(candidate);
    }
}

pub(crate) fn certificate_display_name_impl(
    subject_name: Option<String>,
    fallback_path: &str,
) -> String {
    subject_name
        .and_then(|subject| {
            certificate_common_name_impl(&subject).or_else(|| {
                let compact = subject.trim().to_string();
                if compact.is_empty() {
                    None
                } else {
                    Some(compact)
                }
            })
        })
        .unwrap_or_else(|| certificate_name_from_path_impl(fallback_path))
}

pub(crate) fn certificate_common_name_impl(subject_name: &str) -> Option<String> {
    subject_name.split(',').map(str::trim).find_map(|part| {
        let (key, value) = part.split_once('=')?;
        if key.trim().eq_ignore_ascii_case("CN") {
            let clean = value.trim();
            if clean.is_empty() {
                None
            } else {
                Some(clean.to_string())
            }
        } else {
            None
        }
    })
}