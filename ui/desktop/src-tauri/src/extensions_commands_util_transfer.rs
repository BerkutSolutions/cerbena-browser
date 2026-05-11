use super::*;

pub(crate) fn normalize_transfer_mode(value: &str) -> Result<TransferMode, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "file" => Ok(TransferMode::File),
        "archive" => Ok(TransferMode::Archive),
        _ => Err("unsupported transfer mode".to_string()),
    }
}

pub(crate) fn pick_folder() -> Result<String, String> {
    dialogs::pick_folder()
}

pub(crate) fn pick_import_source(mode: TransferMode) -> Result<String, String> {
    let (filter, title) = match mode {
        TransferMode::File => (
            "Cerbena extension links (cerbena-extensions-links.json)|cerbena-extensions-links.json|JSON files (*.json)|*.json",
            "Select Cerbena extension links file",
        ),
        TransferMode::Archive => (
            "Cerbena archive manifest (manifest.json)|manifest.json|JSON files (*.json)|*.json",
            "Select Cerbena archive manifest file",
        ),
    };
    dialogs::pick_file(title, filter)
}

pub(crate) fn sanitize_file_name(value: &str) -> String {
    let mut cleaned = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    while cleaned.contains("__") {
        cleaned = cleaned.replace("__", "_");
    }
    let trimmed = cleaned.trim_matches('_').trim().to_string();
    if trimmed.is_empty() {
        "extension.zip".to_string()
    } else {
        trimmed
    }
}

pub(crate) fn unique_archive_file_name(
    packages_dir: &Path,
    extension_id: &str,
    original_name: &str,
) -> String {
    let extension = Path::new(original_name)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("zip");
    let stem = Path::new(original_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("extension");
    let base = format!(
        "{}-{}",
        sanitize_file_name(extension_id),
        sanitize_file_name(stem)
    );
    let mut candidate = format!("{base}.{extension}");
    let mut counter = 2usize;
    while packages_dir.join(&candidate).exists() {
        candidate = format!("{base}-{counter}.{extension}");
        counter += 1;
    }
    candidate
}

pub(crate) fn persist_extension_package(
    state: &AppState,
    extension_id: &str,
    package_bytes: Option<&[u8]>,
    package_extension: Option<&str>,
    package_file_name: Option<&str>,
) -> Result<(Option<String>, Option<String>), String> {
    let Some(bytes) = package_bytes.filter(|value| !value.is_empty()) else {
        return Ok((None, package_file_name.map(str::to_string)));
    };
    let package_root = state.extension_packages_root(&state.app_handle)?;
    fs::create_dir_all(&package_root).map_err(|e| format!("create extension package store: {e}"))?;
    let extension = package_extension
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("zip");
    let fallback_name = format!("{}.{}", sanitize_file_name(extension_id), extension);
    let display_name = package_file_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&fallback_name);
    let file_name = unique_archive_file_name(&package_root, extension_id, display_name);
    let path = package_root.join(&file_name);
    fs::write(&path, bytes).map_err(|e| format!("write extension package: {e}"))?;
    Ok((Some(path.to_string_lossy().to_string()), Some(file_name)))
}

pub(crate) fn delete_extension_package(package_path: Option<&str>) {
    let Some(path) = package_path else {
        return;
    };
    if path.trim().is_empty() {
        return;
    }
    let package = Path::new(path);
    if let Err(error) = fs::remove_file(package) {
        let missing = error.kind() == std::io::ErrorKind::NotFound;
        if !missing {
            eprintln!("failed to remove extension package {}: {error}", package.display());
        }
    }
}
