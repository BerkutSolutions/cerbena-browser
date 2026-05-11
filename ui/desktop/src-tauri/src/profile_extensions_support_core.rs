use super::*;

use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use browser_profile::Engine;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::state::{ExtensionLibraryItem, ExtensionPackageVariant};
#[path = "profile_extensions_support_core_fs.rs"]
mod profile_extensions_support_core_fs;

pub(crate) use profile_extensions_support_core_fs::{
    collect_extension_dir_names,
    suppress_dark_reader_install_tab,
};

pub(super) fn collect_profile_package_stems(root: &Path, ids: &mut BTreeSet<String>) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)
        .map_err(|e| format!("read profile package dir {}: {e}", root.display()))?
    {
        let entry = entry.map_err(|e| format!("read profile package entry: {e}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        ids.insert(stem.to_string());
    }
    Ok(())
}

pub(super) fn resolve_variant_for_engine(
    item: &ExtensionLibraryItem,
    engine: Engine,
) -> Option<ExtensionPackageVariant> {
    let mut variants = item.package_variants.clone();
    if variants.is_empty() {
        variants.push(ExtensionPackageVariant {
            engine_scope: item.engine_scope.clone(),
            version: item.version.clone(),
            source_kind: item.source_kind.clone(),
            source_value: item.source_value.clone(),
            logo_url: item.logo_url.clone(),
            store_url: item.store_url.clone(),
            package_path: item.package_path.clone(),
            package_file_name: item.package_file_name.clone(),
        });
    }
    variants
        .into_iter()
        .find(|variant| engine_scope_matches_profile(&variant.engine_scope, engine.clone()))
}

pub(super) fn engine_scope_matches_profile(engine_scope: &str, engine: Engine) -> bool {
    match normalize_engine_scope(engine_scope).as_str() {
        "firefox" => matches!(engine, Engine::Librewolf | Engine::FirefoxEsr),
        "chromium" => engine.is_chromium_family(),
        "chromium/firefox" => true,
        _ => true,
    }
}

pub(super) fn normalize_engine_scope(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "librewolf" => "firefox".to_string(),
        "ungoogled-chromium" => "chromium".to_string(),
        "" => "chromium/firefox".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn package_extension(package_path: &str, package_file_name: Option<&str>) -> String {
    Path::new(package_file_name.unwrap_or(package_path))
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "zip".to_string())
}

pub(super) fn read_extension_manifest(path: &Path) -> Result<serde_json::Value, String> {
    let bytes =
        fs::read(path).map_err(|e| format!("read extension package {}: {e}", path.display()))?;
    let archive_bytes = if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("crx"))
        .unwrap_or(false)
    {
        extract_crx_zip_bytes(&bytes)?
    } else {
        bytes
    };
    let cursor = Cursor::new(archive_bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("open extension archive: {e}"))?;
    let manifest = read_manifest_from_archive(&mut archive)?;
    serde_json::from_str(&manifest).map_err(|e| format!("parse extension manifest: {e}"))
}

pub(super) fn read_manifest_from_archive<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<String, String> {
    if let Ok(mut manifest) = archive.by_name("manifest.json") {
        let mut text = String::new();
        manifest
            .read_to_string(&mut text)
            .map_err(|e| format!("read manifest.json: {e}"))?;
        return Ok(text);
    }
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("read archive entry: {e}"))?;
        let name = entry.name().replace('\\', "/");
        if name.ends_with("/manifest.json") {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|e| format!("read nested manifest.json: {e}"))?;
            return Ok(text);
        }
    }
    Err("extension package manifest.json not found".to_string())
}

pub(super) fn extract_crx_zip_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let signature = b"PK\x03\x04";
    let Some(offset) = bytes
        .windows(signature.len())
        .position(|window| window == signature)
    else {
        return Err("embedded zip payload not found in CRX package".to_string());
    };
    Ok(bytes[offset..].to_vec())
}

pub(super) fn chromium_extension_id_from_manifest(manifest: &serde_json::Value) -> Option<String> {
    let key = manifest.get("key")?.as_str()?.trim();
    if key.is_empty() {
        return None;
    }
    let der = BASE64_STANDARD.decode(key.as_bytes()).ok()?;
    let hash = Sha256::digest(&der);
    let alphabet = b"abcdefghijklmnop";
    let mut id = String::with_capacity(32);
    for byte in hash.iter().take(16) {
        id.push(alphabet[((byte >> 4) & 0x0f) as usize] as char);
        id.push(alphabet[(byte & 0x0f) as usize] as char);
    }
    Some(id)
}

pub(super) fn firefox_extension_id_from_manifest(manifest: &serde_json::Value) -> Option<String> {
    manifest
        .get("browser_specific_settings")
        .and_then(|value| value.get("gecko"))
        .and_then(|value| value.get("id"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| {
            manifest
                .get("applications")
                .and_then(|value| value.get("gecko"))
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
}

pub(super) fn sync_unpacked_chromium_extension(package_path: &Path, destination: &Path) -> Result<(), String> {
    let package_hash = format!(
        "{:x}",
        Sha256::digest(
            &fs::read(package_path).map_err(|e| format!("read extension package: {e}"))?
        )
    );
    let marker_path = destination.join(".cerbena-package-sha256");
    if destination.is_dir()
        && destination.join("manifest.json").is_file()
        && fs::read_to_string(&marker_path).unwrap_or_default().trim() == package_hash
    {
        return Ok(());
    }
    if destination.exists() {
        let _ = fs::remove_dir_all(destination);
    }
    fs::create_dir_all(destination).map_err(|e| format!("create chromium extension dir: {e}"))?;
    unpack_extension_archive(package_path, destination)?;
    fs::write(marker_path, package_hash.as_bytes())
        .map_err(|e| format!("write chromium extension marker: {e}"))?;
    Ok(())
}

pub(super) fn unpack_extension_archive(package_path: &Path, destination: &Path) -> Result<(), String> {
    let bytes = fs::read(package_path).map_err(|e| format!("read extension package: {e}"))?;
    let archive_bytes = if package_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("crx"))
        .unwrap_or(false)
    {
        extract_crx_zip_bytes(&bytes)?
    } else {
        bytes
    };
    let cursor = Cursor::new(archive_bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("open extension archive: {e}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("read extension archive entry: {e}"))?;
        let Some(relative_path) = entry.enclosed_name().map(|value| value.to_path_buf()) else {
            continue;
        };
        let output_path = destination.join(relative_path);
        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(|e| format!("create extension dir: {e}"))?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create extension parent dir: {e}"))?;
        }
        let mut buffer = Vec::new();
        entry
            .read_to_end(&mut buffer)
            .map_err(|e| format!("read extension file: {e}"))?;
        fs::write(output_path, buffer).map_err(|e| format!("write extension file: {e}"))?;
    }
    Ok(())
}

pub(super) fn sync_firefox_engine_profile_extensions(
    set: &ProfileExtensionSet,
    profile_root: &Path,
) -> Result<(), String> {
    browser::sync_firefox_engine_profile_extensions_impl(set, profile_root)
}

pub(super) fn cleanup_chromium_external_extension_manifests(
    profile_root: &Path,
    keep_file_names: &BTreeSet<String>,
) -> Result<(), String> {
    browser::cleanup_chromium_external_extension_manifests_impl(profile_root, keep_file_names)
}

pub(super) fn register_chromium_external_manifest_for_item(
    profile_root: &Path,
    item: &ProfileInstalledExtension,
) -> Result<(), String> {
    browser::register_chromium_external_manifest_for_item_impl(profile_root, item)
}

pub(super) fn cleanup_legacy_chromium_extension_root(profile_root: &Path) -> Result<(), String> {
    browser::cleanup_legacy_chromium_extension_root_impl(profile_root)
}

pub(super) fn sanitize_chromium_runtime_extension_state(profile_root: &Path) -> Result<(), String> {
    browser::sanitize_chromium_runtime_extension_state_impl(profile_root)
}

#[allow(dead_code)]
pub(super) fn cleanup_chromium_extension_runtime_dirs(profile_root: &Path, removed_ids: &[String]) {
    if removed_ids.is_empty() {
        return;
    }
    let roots = [
        profile_root
            .join("engine-profile")
            .join("Default")
            .join("Local Extension Settings"),
        profile_root
            .join("engine-profile")
            .join("Default")
            .join("Sync Extension Settings"),
        profile_root
            .join("engine-profile")
            .join("Default")
            .join("Managed Extension Settings"),
        profile_root
            .join("engine-profile")
            .join("Default")
            .join("Extensions"),
    ];
    for root in roots {
        for browser_id in removed_ids {
            let target = root.join(browser_id);
            if target.exists() {
                let _ = fs::remove_dir_all(&target);
            }
        }
    }
}

pub(super) fn sync_chromium_store_from_browser(
    state: &AppState,
    profile: &ProfileMetadata,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    browser::sync_chromium_store_from_browser_impl(state, profile, set)
}

pub(super) fn sync_firefox_store_from_browser(
    state: &AppState,
    profile: &ProfileMetadata,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    browser::sync_firefox_store_from_browser_impl(state, profile, set)
}

#[allow(dead_code)]
pub(super) fn normalize_path_key(path: &str) -> String {
    path.replace('/', "\\").trim().to_ascii_lowercase()
}

pub(super) fn cleanup_profile_extension_artifacts(item: &ProfileInstalledExtension) {
    if let Some(path) = item.profile_unpacked_path.as_deref() {
        let _ = fs::remove_dir_all(path);
    }
    if let Some(path) = item.profile_package_path.as_deref() {
        let _ = fs::remove_file(path);
    }
}

#[allow(dead_code)]
pub(super) fn profile_extension_store_file(profile_root_base: &Path, profile_id: &str) -> PathBuf {
    store::profile_extension_store_file_impl(profile_root_base, profile_id)
}

pub(super) fn persist_all(
    state: &AppState,
    store: &ProfileExtensionStore,
    library: &ExtensionLibraryStore,
) -> Result<(), String> {
    store::persist_all_impl(state, store, library)
}


