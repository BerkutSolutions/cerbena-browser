use browser_import_export::{export_profile_archive, import_profile_archive};
use browser_profile::{CreateProfileInput, Engine, ProfileManager};
use tempfile::tempdir;

#[test]
fn profile_archive_roundtrip_is_encrypted_and_validated() {
    let temp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(temp.path()).expect("manager");
    let profile = manager
        .create_profile(CreateProfileInput {
            name: "Exportable".to_string(),
            description: None,
            tags: vec!["sync".to_string()],
            engine: Engine::Wayfern,
            default_start_page: None,
            default_search_provider: None,
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .expect("profile");

    let archive = export_profile_archive(
        &profile,
        vec![(
            "data/bookmarks.json".to_string(),
            b"{\"items\":[]}".to_vec(),
        )],
        "export-passphrase",
    )
    .expect("archive");

    let imported =
        import_profile_archive(&archive, profile.id, "export-passphrase").expect("import archive");
    assert_eq!(imported.profile_id, profile.id);
    assert_eq!(imported.files.len(), 1);
}
