use super::*;

#[test]
fn cache_cleanup_profile_and_global_work() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let p1 = manager
        .create_profile(CreateProfileInput {
            name: "C1".to_string(),
            description: None,
            tags: vec![],
            engine: Engine::Chromium,
            default_start_page: None,
            default_search_provider: None,
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .expect("p1");
    let p2 = manager
        .create_profile(CreateProfileInput {
            name: "C2".to_string(),
            description: None,
            tags: vec![],
            engine: Engine::Librewolf,
            default_start_page: None,
            default_search_provider: None,
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .expect("p2");

    std::fs::write(
        tmp.path()
            .join(p1.id.to_string())
            .join("cache")
            .join("a.tmp"),
        b"a",
    )
    .expect("write");
    std::fs::write(
        tmp.path()
            .join(p2.id.to_string())
            .join("cache")
            .join("b.tmp"),
        b"b",
    )
    .expect("write");

    let one = manager
        .cleanup_profile_cache(p1.id, "qa")
        .expect("cleanup one");
    assert!(one.removed_entries >= 1);
    let all = manager.cleanup_all_caches("qa").expect("cleanup all");
    assert!(all.removed_entries >= 1);
}

#[test]
fn running_profile_rejects_security_flag_changes() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "Runtime Locked".to_string(),
            description: None,
            tags: vec![],
            engine: Engine::Chromium,
            default_start_page: None,
            default_search_provider: None,
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .expect("create profile");

    manager
        .update_profile(
            created.id,
            PatchProfileInput {
                state: Some(ProfileState::Running),
                ..PatchProfileInput::default()
            },
        )
        .expect("set running");

    let err = manager.update_profile(
        created.id,
        PatchProfileInput {
            panic_frame_enabled: Some(true),
            ..PatchProfileInput::default()
        },
    );
    assert!(err.is_err());
}

