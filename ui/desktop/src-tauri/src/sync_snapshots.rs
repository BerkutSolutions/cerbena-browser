use std::{
    fs,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use browser_fingerprint::IdentityPreset;
use browser_network_policy::{DnsTabPayload, VpnProxyTabPayload};
use browser_profile::{PatchProfileInput, ProfileMetadata};
use browser_sync_client::{
    decrypt_sync_payload, encrypt_sync_payload, BackupSnapshot, ConflictViewItem, RestoreRequest,
    RestoreResult, RestoreScope, SyncControlsModel, SyncKeyMaterial,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::state::{
    persist_identity_store, persist_network_store, persist_sync_store_with_secret, AppState,
};

const SNAPSHOT_KEY_ID: &str = "cerbena-local-snapshot-v1";
const SNAPSHOT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotFileEntry {
    pub relative_path: String,
    pub content_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotProfileConfig {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub default_start_page: Option<String>,
    pub default_search_provider: Option<String>,
    pub ephemeral_mode: bool,
    pub panic_frame_enabled: bool,
    #[serde(default)]
    pub panic_frame_color: Option<String>,
    #[serde(default)]
    pub panic_protected_sites: Vec<String>,
    #[serde(default)]
    pub ephemeral_retain_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMaterial {
    pub schema_version: u32,
    pub profile_id: Uuid,
    pub profile: SnapshotProfileConfig,
    #[serde(default)]
    pub files: Vec<SnapshotFileEntry>,
    pub identity_preset: Option<IdentityPreset>,
    pub dns_policy: Option<DnsTabPayload>,
    pub vpn_proxy_policy: Option<VpnProxyTabPayload>,
    pub sync_controls: Option<SyncControlsModel>,
    #[serde(default)]
    pub sync_conflicts: Vec<ConflictViewItem>,
}

pub fn create_snapshot_for_profile(
    state: &AppState,
    profile_id: Uuid,
) -> Result<(String, String, Vec<String>), String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "manager lock poisoned".to_string())?;
    manager
        .ensure_unlocked(profile_id)
        .map_err(|e| e.to_string())?;
    let metadata = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    drop(manager);

    let material = build_snapshot_material(state, &metadata)?;
    let plaintext =
        serde_json::to_vec_pretty(&material).map_err(|e| format!("serialize snapshot: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&plaintext);
    let plaintext_hash = format!("{:x}", hasher.finalize());

    let key = snapshot_key(state, profile_id);
    let envelope =
        encrypt_sync_payload(&key, &plaintext).map_err(|e| format!("encrypt snapshot: {e}"))?;
    let envelope_bytes =
        serde_json::to_vec(&envelope).map_err(|e| format!("serialize envelope: {e}"))?;
    let encrypted_blob_b64 = B64.encode(envelope_bytes);
    let payload_paths = snapshot_payload_paths(&material);
    Ok((encrypted_blob_b64, plaintext_hash, payload_paths))
}

pub fn restore_snapshot_for_profile(
    state: &AppState,
    request: &RestoreRequest,
    snapshot: &BackupSnapshot,
) -> Result<(RestoreResult, Vec<String>), String> {
    let material = decode_snapshot_material(state, snapshot)?;
    if material.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported snapshot schema version: {}",
            material.schema_version
        ));
    }
    if material.profile_id != request.profile_id {
        return Err("snapshot profile mismatch".to_string());
    }

    let payload_paths = snapshot_payload_paths(&material);
    let selected_paths = filter_restore_paths(&payload_paths, request);
    apply_snapshot_material(
        state,
        request.profile_id,
        &material,
        &selected_paths,
        request.scope,
    )?;

    let restored_items = selected_paths.len();
    let skipped_items = payload_paths.len().saturating_sub(restored_items);
    Ok((
        RestoreResult {
            restored_snapshot_id: snapshot.snapshot_id.clone(),
            restored_profile_id: snapshot.profile_id,
            restored_items,
            skipped_items,
        },
        payload_paths,
    ))
}

pub fn verify_snapshot_integrity(
    state: &AppState,
    snapshot: &BackupSnapshot,
) -> Result<(bool, Vec<String>), String> {
    let material = decode_snapshot_material(state, snapshot)?;
    let payload_paths = snapshot_payload_paths(&material);
    let plaintext =
        serde_json::to_vec_pretty(&material).map_err(|e| format!("serialize snapshot: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&plaintext);
    let computed = format!("{:x}", hasher.finalize());
    Ok((
        snapshot
            .integrity_sha256_hex
            .eq_ignore_ascii_case(computed.trim()),
        payload_paths,
    ))
}

fn build_snapshot_material(
    state: &AppState,
    metadata: &ProfileMetadata,
) -> Result<SnapshotMaterial, String> {
    let profile_key = metadata.id.to_string();
    let files =
        collect_profile_data_files(&state.profile_root.join(profile_key.clone()).join("data"))?;

    let identity_preset = {
        let store = state
            .identity_store
            .lock()
            .map_err(|_| "identity store lock poisoned".to_string())?;
        store.items.get(&profile_key).cloned()
    };
    let (dns_policy, vpn_proxy_policy) = {
        let store = state
            .network_store
            .lock()
            .map_err(|_| "network store lock poisoned".to_string())?;
        (
            store.dns.get(&profile_key).cloned(),
            store.vpn_proxy.get(&profile_key).cloned(),
        )
    };
    let (sync_controls, sync_conflicts) = {
        let store = state
            .sync_store
            .lock()
            .map_err(|_| "sync store lock poisoned".to_string())?;
        (
            store.controls.get(&profile_key).cloned(),
            store
                .conflicts
                .get(&profile_key)
                .cloned()
                .unwrap_or_default(),
        )
    };

    Ok(SnapshotMaterial {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        profile_id: metadata.id,
        profile: SnapshotProfileConfig {
            name: metadata.name.clone(),
            description: metadata.description.clone(),
            tags: metadata.tags.clone(),
            default_start_page: metadata.default_start_page.clone(),
            default_search_provider: metadata.default_search_provider.clone(),
            ephemeral_mode: metadata.ephemeral_mode,
            panic_frame_enabled: metadata.panic_frame_enabled,
            panic_frame_color: metadata.panic_frame_color.clone(),
            panic_protected_sites: metadata.panic_protected_sites.clone(),
            ephemeral_retain_paths: metadata.ephemeral_retain_paths.clone(),
        },
        files,
        identity_preset,
        dns_policy,
        vpn_proxy_policy,
        sync_controls,
        sync_conflicts,
    })
}

fn decode_snapshot_material(
    state: &AppState,
    snapshot: &BackupSnapshot,
) -> Result<SnapshotMaterial, String> {
    let bytes = B64
        .decode(snapshot.encrypted_blob_b64.trim())
        .map_err(|e| format!("decode snapshot blob: {e}"))?;
    let envelope =
        serde_json::from_slice(&bytes).map_err(|e| format!("parse snapshot blob: {e}"))?;
    let key = snapshot_key(state, snapshot.profile_id);
    let mut plaintext =
        decrypt_sync_payload(&key, &envelope).map_err(|e| format!("decrypt snapshot: {e}"))?;
    let material = serde_json::from_slice::<SnapshotMaterial>(&plaintext)
        .map_err(|e| format!("parse decrypted snapshot: {e}"))?;
    plaintext.fill(0);
    Ok(material)
}

fn snapshot_key(state: &AppState, profile_id: Uuid) -> SyncKeyMaterial {
    SyncKeyMaterial {
        profile_id,
        key_id: SNAPSHOT_KEY_ID.to_string(),
        wrapping_secret: state.sensitive_store_secret.clone(),
    }
}

fn snapshot_payload_paths(material: &SnapshotMaterial) -> Vec<String> {
    let mut paths = Vec::new();
    paths.push("profile/config.json".to_string());
    if material.identity_preset.is_some() {
        paths.push("identity/preset.json".to_string());
    }
    if material.dns_policy.is_some() {
        paths.push("network/dns.json".to_string());
    }
    if material.vpn_proxy_policy.is_some() {
        paths.push("network/vpn_proxy.json".to_string());
    }
    if material.sync_controls.is_some() {
        paths.push("sync/controls.json".to_string());
    }
    if !material.sync_conflicts.is_empty() {
        paths.push("sync/conflicts.json".to_string());
    }
    paths.extend(
        material
            .files
            .iter()
            .map(|entry| format!("files/{}", entry.relative_path)),
    );
    paths
}

fn filter_restore_paths(all_paths: &[String], request: &RestoreRequest) -> Vec<String> {
    match request.scope {
        RestoreScope::Full => all_paths.to_vec(),
        RestoreScope::Selective => all_paths
            .iter()
            .filter(|path| {
                request
                    .include_prefixes
                    .iter()
                    .any(|prefix| path.starts_with(prefix))
            })
            .cloned()
            .collect(),
    }
}

fn apply_snapshot_material(
    state: &AppState,
    profile_id: Uuid,
    material: &SnapshotMaterial,
    selected_paths: &[String],
    scope: RestoreScope,
) -> Result<(), String> {
    let profile_key = profile_id.to_string();
    let restore_config = selected_paths
        .iter()
        .any(|path| path == "profile/config.json");
    let restore_identity = selected_paths
        .iter()
        .any(|path| path == "identity/preset.json");
    let restore_dns = selected_paths.iter().any(|path| path == "network/dns.json");
    let restore_vpn = selected_paths
        .iter()
        .any(|path| path == "network/vpn_proxy.json");
    let restore_sync_controls = selected_paths
        .iter()
        .any(|path| path == "sync/controls.json");
    let restore_sync_conflicts = selected_paths
        .iter()
        .any(|path| path == "sync/conflicts.json");
    let restore_files = selected_paths
        .iter()
        .filter_map(|path| path.strip_prefix("files/").map(str::to_string))
        .collect::<Vec<_>>();

    if restore_config {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "manager lock poisoned".to_string())?;
        manager
            .update_profile_with_actor(
                profile_id,
                PatchProfileInput {
                    name: Some(material.profile.name.clone()),
                    description: Some(material.profile.description.clone()),
                    tags: Some(material.profile.tags.clone()),
                    default_start_page: Some(material.profile.default_start_page.clone()),
                    default_search_provider: Some(material.profile.default_search_provider.clone()),
                    ephemeral_mode: Some(material.profile.ephemeral_mode),
                    panic_frame_enabled: Some(material.profile.panic_frame_enabled),
                    panic_frame_color: None,
                    panic_protected_sites: Some(material.profile.panic_protected_sites.clone()),
                    ephemeral_retain_paths: Some(material.profile.ephemeral_retain_paths.clone()),
                    ..PatchProfileInput::default()
                },
                None,
                "sync-restore",
            )
            .map_err(|e| e.to_string())?;
    }

    if restore_identity {
        let mut store = state
            .identity_store
            .lock()
            .map_err(|_| "identity store lock poisoned".to_string())?;
        match &material.identity_preset {
            Some(preset) => {
                store.items.insert(profile_key.clone(), preset.clone());
            }
            None => {
                store.items.remove(&profile_key);
            }
        }
        let path = state.identity_store_path(&state.app_handle)?;
        persist_identity_store(&path, &store)?;
    }

    if restore_dns || restore_vpn {
        let mut store = state
            .network_store
            .lock()
            .map_err(|_| "network store lock poisoned".to_string())?;
        if restore_dns {
            match &material.dns_policy {
                Some(policy) => {
                    store.dns.insert(profile_key.clone(), policy.clone());
                }
                None => {
                    store.dns.remove(&profile_key);
                }
            }
        }
        if restore_vpn {
            match &material.vpn_proxy_policy {
                Some(policy) => {
                    store.vpn_proxy.insert(profile_key.clone(), policy.clone());
                }
                None => {
                    store.vpn_proxy.remove(&profile_key);
                }
            }
        }
        let path = state.network_store_path(&state.app_handle)?;
        persist_network_store(&path, &store)?;
    }

    if restore_sync_controls || restore_sync_conflicts {
        let mut store = state
            .sync_store
            .lock()
            .map_err(|_| "sync store lock poisoned".to_string())?;
        if restore_sync_controls {
            match &material.sync_controls {
                Some(controls) => {
                    store.controls.insert(profile_key.clone(), controls.clone());
                }
                None => {
                    store.controls.remove(&profile_key);
                }
            }
        }
        if restore_sync_conflicts {
            if material.sync_conflicts.is_empty() {
                store.conflicts.remove(&profile_key);
            } else {
                store
                    .conflicts
                    .insert(profile_key.clone(), material.sync_conflicts.clone());
            }
        }
        let path = state.sync_store_path(&state.app_handle)?;
        persist_sync_store_with_secret(&path, &state.sensitive_store_secret, &store)?;
    }

    if !restore_files.is_empty() {
        let data_root = state.profile_root.join(profile_key).join("data");
        if matches!(scope, RestoreScope::Full) {
            if data_root.exists() {
                fs::remove_dir_all(&data_root)
                    .map_err(|e| format!("clear profile data for restore: {e}"))?;
            }
            fs::create_dir_all(&data_root)
                .map_err(|e| format!("recreate profile data for restore: {e}"))?;
        }
        for entry in &material.files {
            if !restore_files
                .iter()
                .any(|path| path == &entry.relative_path)
            {
                continue;
            }
            let target = normalize_snapshot_file_target(&data_root, &entry.relative_path)?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("create restored parent dir: {e}"))?;
            }
            let bytes = B64
                .decode(entry.content_b64.trim())
                .map_err(|e| format!("decode restored file {}: {e}", entry.relative_path))?;
            fs::write(&target, bytes)
                .map_err(|e| format!("write restored file {}: {e}", target.display()))?;
        }
    }

    Ok(())
}

fn collect_profile_data_files(data_root: &Path) -> Result<Vec<SnapshotFileEntry>, String> {
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

fn normalize_snapshot_file_target(
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
