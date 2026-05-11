use super::*;

pub(crate) fn collect_profile_data_files(data_root: &Path) -> Result<Vec<SnapshotFileEntry>, String> {
    let mut files = Vec::new();
    if !data_root.exists() {
        return Ok(files);
    }
    collect_profile_data_files_recursive(data_root, data_root, &mut files)?;
    Ok(files)
}

fn collect_profile_data_files_recursive(
    base: &Path,
    current: &Path,
    out: &mut Vec<SnapshotFileEntry>,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|e| format!("read profile data dir: {e}"))? {
        let entry = entry.map_err(|e| format!("read profile data entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_profile_data_files_recursive(base, &path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path)
            .map_err(|e| format!("read profile snapshot file {}: {e}", path.display()))?;
        let relative_path = path
            .strip_prefix(base)
            .map_err(|e| format!("strip snapshot base {}: {e}", path.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        out.push(SnapshotFileEntry {
            relative_path,
            content_b64: B64.encode(bytes),
        });
    }
    Ok(())
}

pub(crate) fn normalize_snapshot_file_target(
    data_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, String> {
    let relative = PathBuf::from(relative_path.replace('\\', "/"));
    if relative.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(format!("invalid snapshot relative path: {relative_path}"));
    }
    Ok(data_root.join(relative))
}

#[cfg(test)]
mod tests {
    use super::{
        filter_restore_paths, normalize_snapshot_file_target, SnapshotMaterial,
        SnapshotProfileConfig,
    };
    use browser_sync_client::{RestoreRequest, RestoreScope};
    use std::path::Path;
    use uuid::Uuid;

    #[test]
    fn selective_restore_filters_payload_paths() {
        let request = RestoreRequest {
            profile_id: Uuid::new_v4(),
            snapshot_id: "snap-1".to_string(),
            scope: RestoreScope::Selective,
            include_prefixes: vec!["files/cookies/".to_string(), "identity/".to_string()],
            expected_schema_version: 1,
        };
        let filtered = filter_restore_paths(
            &[
                "profile/config.json".to_string(),
                "identity/preset.json".to_string(),
                "files/cookies/data.json".to_string(),
                "files/history/data.json".to_string(),
            ],
            &request,
        );
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|item| item == "identity/preset.json"));
        assert!(filtered
            .iter()
            .any(|item| item == "files/cookies/data.json"));
    }

    #[test]
    fn snapshot_restore_rejects_parent_escape_paths() {
        let data_root = Path::new("C:/tmp/data");
        assert!(normalize_snapshot_file_target(data_root, "../evil.txt").is_err());
        assert!(normalize_snapshot_file_target(data_root, "cookies/good.txt").is_ok());
    }

    #[test]
    fn snapshot_material_schema_is_stable() {
        let material = SnapshotMaterial {
            schema_version: 2,
            profile_id: Uuid::nil(),
            profile: SnapshotProfileConfig {
                name: "Main".to_string(),
                description: None,
                tags: vec!["daily".to_string()],
                default_start_page: Some("https://duckduckgo.com".to_string()),
                default_search_provider: Some("duckduckgo".to_string()),
                ephemeral_mode: false,
                panic_frame_enabled: true,
                panic_frame_color: None,
                panic_protected_sites: vec!["duckduckgo.com".to_string()],
                ephemeral_retain_paths: vec![],
            },
            files: vec![],
            identity_preset: None,
            dns_policy: None,
            vpn_proxy_policy: None,
            sync_controls: None,
            sync_conflicts: vec![],
        };
        assert_eq!(material.schema_version, 2);
        assert!(material.profile.panic_frame_enabled);
    }
}
