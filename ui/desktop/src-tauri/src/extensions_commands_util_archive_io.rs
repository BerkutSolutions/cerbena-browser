use super::*;

pub(crate) fn read_extension_archive_metadata_batch(
    path: &Path,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    let bytes = fs::read(path).map_err(|e| format!("read extension package: {e}"))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("extension.zip");
    read_extension_archive_metadata_batch_from_bytes(&bytes, file_name, store_url)
}

pub(crate) fn read_extension_directory_metadata_batch(
    path: &Path,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    if !path.exists() {
        return Err(format!("extension directory not found: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!(
            "extension directory path is not a folder: {}",
            path.display()
        ));
    }

    let roots = discover_extension_directory_roots(path)?;
    if roots.is_empty() {
        return Err(format!(
            "extension directory manifest.json not found under {}",
            path.display()
        ));
    }
    let mut batch = Vec::new();
    for root in roots {
        let zip_bytes = package_extension_directory(&root)?;
        let base_name = root
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("extension");
        let provisional_name = format!("{base_name}.zip");
        let mut metadata =
            read_extension_archive_metadata_from_bytes(&zip_bytes, &provisional_name, store_url)?;
        let package_extension = match metadata.engine_scope.as_deref() {
            Some("firefox") => "xpi",
            _ => "zip",
        };
        metadata.package_bytes = Some(zip_bytes);
        metadata.package_extension = Some(package_extension.to_string());
        metadata.package_file_name = Some(super::archive_manifest::package_display_name(
            &format!("{base_name}.{package_extension}"),
            package_extension,
        ));
        batch.push(metadata);
    }
    Ok(batch)
}

pub(crate) fn read_extension_archive_metadata_from_bytes(
    bytes: &[u8],
    file_name: &str,
    store_url: Option<&str>,
) -> Result<DerivedExtensionMetadata, String> {
    let batch = read_extension_archive_metadata_batch_from_bytes(bytes, file_name, store_url)?;
    batch
        .into_iter()
        .next()
        .ok_or_else(|| "extension package manifest.json not found".to_string())
}

pub(crate) fn read_extension_archive_metadata_batch_from_bytes(
    bytes: &[u8],
    file_name: &str,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    let package_extension = super::archive_manifest::infer_package_extension(file_name, store_url);
    let archive_bytes = if package_extension.eq_ignore_ascii_case("crx") {
        extract_embedded_zip_bytes(bytes)?
    } else {
        bytes.to_vec()
    };
    let cursor = Cursor::new(archive_bytes);
    let mut zip = ZipArchive::new(cursor).map_err(|e| format!("open extension archive: {e}"))?;
    let Some(manifest) = read_zip_text(&mut zip, "manifest.json") else {
        if package_extension.eq_ignore_ascii_case("zip") {
            let roots = discover_nested_extension_roots(&mut zip)?;
            if roots.is_empty() {
                return Err("extension package manifest.json not found".to_string());
            }
            let mut batch = Vec::new();
            for root in roots {
                let nested_bytes = repackage_nested_extension(bytes, &root)?;
                let nested_file_name = super::archive_manifest::package_display_name(&format!("{root}.zip"), "zip");
                let mut metadata = read_extension_archive_metadata_from_bytes(
                    &nested_bytes,
                    &nested_file_name,
                    store_url,
                )?;
                metadata.package_bytes = Some(nested_bytes);
                metadata.package_extension = Some("zip".to_string());
                metadata.package_file_name = Some(nested_file_name);
                batch.push(metadata);
            }
            return Ok(batch);
        }
        return Err("extension package manifest.json not found".to_string());
    };
    let manifest_json = serde_json::from_str::<serde_json::Value>(&manifest)
        .map_err(|e| format!("parse manifest: {e}"))?;

    let display_name = super::archive_manifest::manifest_localized_string(
        &mut zip,
        &manifest_json,
        manifest_json.get("name").and_then(|value| value.as_str()),
    );
    let version = manifest_json
        .get("version")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let engine_scope = super::archive_manifest::manifest_engine_scope(&manifest_json, file_name, store_url);
    let logo_url = super::archive_manifest::manifest_logo_data_url(&mut zip, &manifest_json);

    Ok(vec![DerivedExtensionMetadata {
        stable_id: super::archive_manifest::manifest_stable_id(&manifest_json)
            .or_else(|| super::archive_manifest::store_url_fallback_id(store_url)),
        display_name,
        version,
        engine_scope,
        logo_url,
        package_bytes: Some(bytes.to_vec()),
        package_extension: Some(package_extension.clone()),
        package_file_name: Some(super::archive_manifest::package_display_name(
            file_name,
            &package_extension,
        )),
    }])
}

pub(crate) fn extract_embedded_zip_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let signature = b"PK\x03\x04";
    let Some(offset) = bytes
        .windows(signature.len())
        .position(|window| window == signature)
    else {
        return Err("embedded zip payload not found in CRX package".to_string());
    };
    Ok(bytes[offset..].to_vec())
}

pub(crate) fn read_zip_text<R: Read + std::io::Seek>(zip: &mut ZipArchive<R>, name: &str) -> Option<String> {
    let mut file = zip.by_name(name).ok()?;
    let mut out = String::new();
    file.read_to_string(&mut out).ok()?;
    Some(out)
}

pub(crate) fn discover_nested_extension_roots<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
) -> Result<Vec<String>, String> {
    let mut roots = BTreeSet::new();
    for index in 0..zip.len() {
        let entry_name = zip
            .by_index(index)
            .map_err(|e| format!("scan extension archive: {e}"))?
            .name()
            .replace('\\', "/");
        if !entry_name.ends_with("/manifest.json") {
            continue;
        }
        let root = entry_name.trim_end_matches("/manifest.json");
        if root.is_empty() {
            continue;
        }
        let segments = root
            .split('/')
            .filter(|segment| !segment.trim().is_empty())
            .collect::<Vec<_>>();
        if segments.len() == 1 {
            roots.insert(segments[0].to_string());
        }
    }
    Ok(roots.into_iter().collect())
}

pub(crate) fn discover_extension_directory_roots(path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.join("manifest.json").is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut roots = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current)
            .map_err(|error| format!("read extension directory {}: {error}", current.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("read extension directory entry: {error}"))?;
            let child = entry.path();
            if !child.is_dir() {
                continue;
            }
            if child.join("manifest.json").is_file() {
                roots.push(child);
            } else {
                stack.push(child);
            }
        }
    }

    roots.sort();
    roots.dedup();
    Ok(roots)
}

pub(crate) fn package_extension_directory(path: &Path) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::<u8>::new());
    let mut writer = ZipWriter::new(&mut output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_directory_to_zip(path, path, &mut writer, options)?;
    writer
        .finish()
        .map_err(|error| format!("finalize extension directory archive: {error}"))?;
    Ok(output.into_inner())
}

pub(crate) fn write_directory_to_zip(
    root: &Path,
    current: &Path,
    writer: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
    options: SimpleFileOptions,
) -> Result<(), String> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| format!("read extension directory {}: {error}", current.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read extension directory entries: {error}"))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("strip extension directory prefix: {error}"))?
            .to_string_lossy()
            .replace('\\', "/");
        if relative.is_empty() {
            continue;
        }
        if path.is_dir() {
            writer
                .add_directory(format!("{relative}/"), options)
                .map_err(|error| format!("write extension directory entry: {error}"))?;
            write_directory_to_zip(root, &path, writer, options)?;
            continue;
        }
        writer
            .start_file(relative, options)
            .map_err(|error| format!("write extension file header: {error}"))?;
        let bytes = fs::read(&path)
            .map_err(|error| format!("read extension file {}: {error}", path.display()))?;
        writer
            .write_all(&bytes)
            .map_err(|error| format!("write extension file {}: {error}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn repackage_nested_extension(bytes: &[u8], root: &str) -> Result<Vec<u8>, String> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut source = ZipArchive::new(cursor).map_err(|e| format!("open extension archive: {e}"))?;
    let mut output = Cursor::new(Vec::<u8>::new());
    let mut writer = ZipWriter::new(&mut output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let prefix = format!("{root}/");

    for index in 0..source.len() {
        let mut file = source
            .by_index(index)
            .map_err(|e| format!("read nested extension archive entry: {e}"))?;
        let entry_name = file.name().replace('\\', "/");
        if !entry_name.starts_with(&prefix) {
            continue;
        }
        let relative = entry_name[prefix.len()..].to_string();
        if relative.is_empty() {
            continue;
        }
        if file.is_dir() {
            writer
                .add_directory(relative, options)
                .map_err(|e| format!("write nested extension directory: {e}"))?;
            continue;
        }
        writer
            .start_file(relative, options)
            .map_err(|e| format!("write nested extension file header: {e}"))?;
        let mut file_bytes = Vec::new();
        file.read_to_end(&mut file_bytes)
            .map_err(|e| format!("read nested extension file: {e}"))?;
        writer
            .write_all(&file_bytes)
            .map_err(|e| format!("write nested extension file: {e}"))?;
    }
    writer
        .finish()
        .map_err(|e| format!("finalize nested extension archive: {e}"))?;
    Ok(output.into_inner())
}
