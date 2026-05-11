use super::*;

#[test]
fn create_profile_builds_isolated_layout_and_metadata() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");

    let created = manager
        .create_profile(CreateProfileInput {
            name: "Main".to_string(),
            description: Some("primary profile".to_string()),
            tags: vec!["daily".to_string()],
            engine: Engine::Chromium,
            default_start_page: Some("https://example.com".to_string()),
            default_search_provider: Some("duckduckgo".to_string()),
            ephemeral_mode: false,
            password_lock_enabled: true,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .expect("create profile");

    let profile_dir = tmp.path().join(created.id.to_string());
    assert!(profile_dir.exists());
    assert!(profile_dir.join("data").exists());
    assert!(profile_dir.join("cache").exists());
    assert!(profile_dir.join("extensions").exists());
    assert!(profile_dir.join("metadata.json").exists());
}

#[test]
fn update_profile_changes_state_and_updated_at() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "Editable".to_string(),
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
        .expect("create profile");

    let before = created.updated_at.clone();
    let updated = manager
        .update_profile(
            created.id,
            PatchProfileInput {
                engine: Some(Engine::Chromium),
                state: Some(ProfileState::Ready),
                tags: Some(vec!["work".to_string(), "isolated".to_string()]),
                ..PatchProfileInput::default()
            },
        )
        .expect("update profile");

    assert_eq!(updated.engine, Engine::Chromium);
    assert_eq!(updated.state, ProfileState::Ready);
    assert_eq!(updated.tags.len(), 2);
    assert_ne!(updated.updated_at, before);
}

#[test]
fn delete_profile_removes_profile_tree() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "To Delete".to_string(),
            description: None,
            tags: vec![],
            engine: Engine::Chromium,
            default_start_page: None,
            default_search_provider: None,
            ephemeral_mode: true,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .expect("create profile");

    manager.delete_profile(created.id).expect("delete profile");
    assert!(!tmp.path().join(created.id.to_string()).exists());
}

#[test]
fn encrypted_secret_requires_correct_key() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "Encrypted".to_string(),
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
        .encrypt_profile_secret(created.id, "master-key", b"super-secret")
        .expect("encrypt");
    let plain = manager
        .decrypt_profile_secret(created.id, "master-key")
        .expect("decrypt");
    assert_eq!(plain, b"super-secret");
    assert!(manager
        .decrypt_profile_secret(created.id, "bad-key")
        .is_err());
}

#[test]
fn password_lock_requires_unlock() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "Locked".to_string(),
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
        .expect("create profile");

    manager
        .set_profile_password(created.id, "p@ssword-123", None)
        .expect("set lock");
    assert!(manager.ensure_unlocked(created.id).is_err());
    let wrong = manager
        .unlock_profile(created.id, "bad")
        .expect("unlock attempt");
    assert!(!wrong);
    let ok = manager
        .unlock_profile(created.id, "p@ssword-123")
        .expect("unlock valid");
    assert!(ok);
    assert!(manager.ensure_unlocked(created.id).is_ok());
}

#[test]
fn close_profile_cleans_ephemeral_data_but_keeps_whitelist() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "Ephemeral".to_string(),
            description: None,
            tags: vec![],
            engine: Engine::Chromium,
            default_start_page: None,
            default_search_provider: None,
            ephemeral_mode: true,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec!["keep.txt".to_string()],
        })
        .expect("create profile");

    let profile_root = tmp.path().join(created.id.to_string());
    std::fs::write(profile_root.join("data").join("keep.txt"), b"keep").expect("write keep");
    std::fs::write(profile_root.join("data").join("drop.txt"), b"drop").expect("write drop");
    std::fs::write(profile_root.join("cache").join("cache.bin"), b"cache").expect("write cache");

    manager.close_profile(created.id).expect("close");

    assert!(profile_root.join("data").join("keep.txt").exists());
    assert!(!profile_root.join("data").join("drop.txt").exists());
    assert!(!profile_root.join("cache").join("cache.bin").exists());
}

#[test]
fn profile_modal_payload_validation_works() {
    let payload = ProfileModalPayload {
        general: browser_profile::profile_modal::GeneralTab {
            name: "Tab Profile".to_string(),
            description: None,
            tags: vec!["prod".to_string()],
            default_start_page: None,
            default_search_provider: None,
        },
        identity: browser_profile::profile_modal::IdentityTab {
            mode: "real".to_string(),
            platform_target: None,
            template_key: None,
        },
        vpn_proxy: browser_profile::profile_modal::VpnProxyTab {
            route_mode: "proxy".to_string(),
            proxy_url: Some("socks5://127.0.0.1:9050".to_string()),
            vpn_profile_ref: None,
        },
        dns: browser_profile::profile_modal::DnsTab {
            resolver_mode: "custom".to_string(),
            servers: vec!["1.2.1.1".to_string()],
        },
        extensions: browser_profile::profile_modal::ExtensionsTab {
            enabled_extension_ids: vec![],
        },
        security: browser_profile::profile_modal::SecurityTab {
            password_lock_enabled: true,
            ephemeral_mode: false,
            ephemeral_retain_paths: vec![],
        },
    };
    validate_modal_payload(&payload).expect("payload is valid");
}

#[test]
fn update_profile_conflict_detected_by_expected_version() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "Versioned".to_string(),
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

    let _ = manager
        .update_profile(created.id, PatchProfileInput::default())
        .expect("first update");

    let stale = manager.update_profile_with_actor(
        created.id,
        PatchProfileInput {
            name: Some("Will fail".to_string()),
            ..PatchProfileInput::default()
        },
        Some(&created.updated_at),
        "tester",
    );
    assert!(stale.is_err());
}

#[test]
fn audit_trail_supports_filtered_queries() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "Audit".to_string(),
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
        .expect("create profile");

    manager
        .update_profile_with_actor(
            created.id,
            PatchProfileInput {
                description: Some(Some("updated".to_string())),
                ..PatchProfileInput::default()
            },
            None,
            "qa-user",
        )
        .expect("update");

    let events = manager
        .get_audit_events(AuditFilter {
            actor: Some("qa-user".to_string()),
            action_prefix: Some("profile.update".to_string()),
            profile_id: Some(created.id),
            outcome: Some("success".to_string()),
        })
        .expect("read events");
    assert_eq!(events.len(), 1);
}

