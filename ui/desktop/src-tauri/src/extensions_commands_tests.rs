use super::*;
use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn build_zip(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut output = Cursor::new(Vec::<u8>::new());
    let mut writer = ZipWriter::new(&mut output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, body) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
    output.into_inner()
}

#[test]
fn reads_single_extension_archive_metadata() {
    let archive = build_zip(&[(
        "manifest.json",
        r#"{"name":"Single","version":"1.2.4","minimum_chrome_version":"120"}"#,
    )]);
    let batch =
        read_extension_archive_metadata_batch_from_bytes(&archive, "single.zip", None).unwrap();
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].display_name.as_deref(), Some("Single"));
    assert_eq!(batch[0].version.as_deref(), Some("1.2.4"));
    assert_eq!(batch[0].engine_scope.as_deref(), Some("chromium"));
}

#[test]
fn reads_multi_extension_archive_metadata_from_nested_folders() {
    let archive = build_zip(&[
        (
            "alpha/manifest.json",
            r#"{"name":"Alpha","version":"1.0.0","minimum_chrome_version":"120"}"#,
        ),
        (
            "beta/manifest.json",
            r#"{"name":"Beta","version":"2.0.0","browser_specific_settings":{"gecko":{"id":"beta@example.com"}}}"#,
        ),
    ]);
    let batch =
        read_extension_archive_metadata_batch_from_bytes(&archive, "bundle.zip", None).unwrap();
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0].display_name.as_deref(), Some("Alpha"));
    assert_eq!(batch[1].display_name.as_deref(), Some("Beta"));
    assert_eq!(batch[0].package_file_name.as_deref(), Some("alpha.zip"));
    assert_eq!(batch[1].package_file_name.as_deref(), Some("beta.zip"));
}

#[test]
fn reads_extension_directory_metadata_for_firefox_folder() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(
        temp.path().join("manifest.json"),
        r#"{"name":"Folder Fox","version":"3.1.0","browser_specific_settings":{"gecko":{"id":"folder-fox@example.com"}}}"#,
    )
    .expect("write manifest");
    fs::create_dir_all(temp.path().join("icons")).expect("create icons");
    fs::write(temp.path().join("icons").join("icon.png"), b"png").expect("write icon");

    let batch =
        read_extension_directory_metadata_batch(temp.path(), None).expect("read folder");
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].display_name.as_deref(), Some("Folder Fox"));
    assert_eq!(batch[0].engine_scope.as_deref(), Some("firefox"));
    assert_eq!(batch[0].package_extension.as_deref(), Some("xpi"));
    assert!(batch[0]
        .package_bytes
        .as_ref()
        .is_some_and(|value| !value.is_empty()));
}

#[test]
fn reads_nested_extension_directories_when_root_has_no_manifest() {
    let temp = tempfile::tempdir().expect("tempdir");
    let alpha = temp.path().join("alpha");
    let beta = temp.path().join("beta");
    fs::create_dir_all(&alpha).expect("create alpha");
    fs::create_dir_all(&beta).expect("create beta");
    fs::write(
        alpha.join("manifest.json"),
        r#"{"name":"Alpha Dir","version":"1.0.0","minimum_chrome_version":"120"}"#,
    )
    .expect("write alpha manifest");
    fs::write(
        beta.join("manifest.json"),
        r#"{"name":"Beta Dir","version":"2.0.0","browser_specific_settings":{"gecko":{"id":"beta-dir@example.com"}}}"#,
    )
    .expect("write beta manifest");

    let batch = read_extension_directory_metadata_batch(temp.path(), None)
        .expect("read nested folders");
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0].display_name.as_deref(), Some("Alpha Dir"));
    assert_eq!(batch[0].package_extension.as_deref(), Some("zip"));
    assert_eq!(batch[1].display_name.as_deref(), Some("Beta Dir"));
    assert_eq!(batch[1].package_extension.as_deref(), Some("xpi"));
}

#[test]
fn transfer_file_import_keeps_non_url_source_identifiers() {
    let item = ExtensionLibraryTransferItem {
        display_name: "Catalog Item".to_string(),
        version: "1.0.0".to_string(),
        engine_scope: "chromium".to_string(),
        source_kind: "store".to_string(),
        source_value: "abc123def456".to_string(),
        logo_url: None,
        store_url: None,
        tags: Vec::new(),
        auto_update_enabled: false,
        preserve_on_panic_wipe: false,
        protect_data_from_panic_wipe: false,
        package_file_name: None,
        package_relative_path: None,
        variants: Vec::new(),
    };

    let requests = transfer::build_import_requests_from_transfer_item(
        &item,
        Path::new("."),
        TransferMode::File,
    )
    .expect("build requests");

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].source_kind, "store");
    assert_eq!(requests[0].source_value, "abc123def456");
}

#[test]
fn store_url_import_falls_back_to_manifest_fields_when_store_metadata_unavailable() {
    let request = ImportExtensionLibraryRequest {
        source_kind: "store_url".to_string(),
        source_value: "https://chromewebstore.google.com/detail/example/invalidinvalidinvalidinvalid".to_string(),
        store_url: Some(
            "https://chromewebstore.google.com/detail/example/invalidinvalidinvalidinvalid"
                .to_string(),
        ),
        display_name: Some("Example Extension".to_string()),
        version: Some("1.2.4".to_string()),
        logo_url: Some("https://example.invalid/logo.png".to_string()),
        engine_scope: Some("chromium".to_string()),
        tags: Some(Vec::new()),
        assigned_profile_ids: Vec::new(),
        auto_update_enabled: Some(false),
        preserve_on_panic_wipe: Some(false),
        protect_data_from_panic_wipe: Some(false),
        package_file_name: None,
        package_bytes_base64: None,
    };

    let batch =
        derive_extension_metadata_batch(&request, request.store_url.as_deref()).expect("batch");
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].display_name.as_deref(), Some("Example Extension"));
    assert_eq!(batch[0].version.as_deref(), Some("1.2.4"));
    assert_eq!(batch[0].engine_scope.as_deref(), Some("chromium"));
    assert_eq!(
        batch[0].logo_url.as_deref(),
        Some("https://example.invalid/logo.png")
    );
}
