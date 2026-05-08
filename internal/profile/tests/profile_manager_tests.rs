use browser_profile::{
    validate_modal_payload, AuditFilter, CreateProfileInput, Engine, PatchProfileInput,
    ProfileManager, ProfileModalPayload, ProfileState, SelectiveWipeRequest, WipeDataType,
};
use rusqlite::{params, Connection};
use tempfile::tempdir;

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
            servers: vec!["1.1.15.1".to_string()],
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

#[test]
fn selective_wipe_removes_requested_data_types() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "Wipe".to_string(),
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

    let root = tmp.path().join(created.id.to_string());
    std::fs::create_dir_all(root.join("data").join("cookies")).expect("cookies dir");
    std::fs::create_dir_all(root.join("data").join("history")).expect("history dir");
    std::fs::write(
        root.join("data").join("cookies").join("cookie.db"),
        b"cookie",
    )
    .expect("cookie write");
    std::fs::write(
        root.join("data").join("history").join("history.db"),
        b"history",
    )
    .expect("history write");

    let affected = manager
        .selective_wipe_profile_data(
            created.id,
            &SelectiveWipeRequest {
                data_types: vec![WipeDataType::Cookies],
                site_scopes: vec![],
                retain_paths: vec![],
            },
            "qa-user",
        )
        .expect("wipe");

    assert!(!affected.is_empty());
    assert!(!root.join("data").join("cookies").join("cookie.db").exists());
    assert!(root
        .join("data")
        .join("history")
        .join("history.db")
        .exists());
}

#[test]
fn selective_wipe_preserves_scoped_domains_in_chromium_sqlite_stores() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "Chromium Scoped".to_string(),
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

    let root = tmp
        .path()
        .join(created.id.to_string())
        .join("engine-profile")
        .join("Default");
    std::fs::create_dir_all(root.join("Network")).expect("network dir");
    let cookies_path = root.join("Network").join("Cookies");
    let history_path = root.join("History");
    seed_chromium_cookies(&cookies_path);
    seed_chromium_history(&history_path);

    let affected = manager
        .selective_wipe_profile_data(
            created.id,
            &SelectiveWipeRequest {
                data_types: vec![WipeDataType::Cookies, WipeDataType::History],
                site_scopes: vec!["keep.example.com".to_string()],
                retain_paths: vec![],
            },
            "qa-user",
        )
        .expect("wipe");

    assert!(affected.iter().any(|item| item.ends_with("Cookies")));
    assert!(affected.iter().any(|item| item.ends_with("History")));
    assert_eq!(
        read_i64(
            &cookies_path,
            "SELECT COUNT(*) FROM cookies WHERE host_key = '.keep.example.com'"
        ),
        1
    );
    assert_eq!(
        read_i64(
            &cookies_path,
            "SELECT COUNT(*) FROM cookies WHERE host_key = '.drop.example.com'"
        ),
        0
    );
    assert_eq!(
        read_i64(
            &history_path,
            "SELECT COUNT(*) FROM urls WHERE url LIKE 'https://keep.example.com/%'"
        ),
        1
    );
    assert_eq!(
        read_i64(
            &history_path,
            "SELECT COUNT(*) FROM urls WHERE url LIKE 'https://drop.example.com/%'"
        ),
        0
    );
    assert_eq!(read_i64(&history_path, "SELECT COUNT(*) FROM visits"), 1);
}

#[test]
fn selective_wipe_preserves_scoped_domains_in_firefox_sqlite_stores() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let created = manager
        .create_profile(CreateProfileInput {
            name: "Firefox Scoped".to_string(),
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

    let root = tmp
        .path()
        .join(created.id.to_string())
        .join("engine-profile");
    std::fs::create_dir_all(&root).expect("engine root");
    let cookies_path = root.join("cookies.sqlite");
    let places_path = root.join("places.sqlite");
    seed_firefox_cookies(&cookies_path);
    seed_firefox_places(&places_path);

    manager
        .selective_wipe_profile_data(
            created.id,
            &SelectiveWipeRequest {
                data_types: vec![WipeDataType::Cookies, WipeDataType::History],
                site_scopes: vec!["keep.example.com".to_string()],
                retain_paths: vec![],
            },
            "qa-user",
        )
        .expect("wipe");

    assert_eq!(
        read_i64(
            &cookies_path,
            "SELECT COUNT(*) FROM moz_cookies WHERE host = '.keep.example.com'"
        ),
        1
    );
    assert_eq!(
        read_i64(
            &cookies_path,
            "SELECT COUNT(*) FROM moz_cookies WHERE host = '.drop.example.com'"
        ),
        0
    );
    assert_eq!(
        read_i64(
            &places_path,
            "SELECT COUNT(*) FROM moz_places WHERE url LIKE 'https://keep.example.com/%'"
        ),
        1
    );
    assert_eq!(
        read_i64(
            &places_path,
            "SELECT COUNT(*) FROM moz_places WHERE url LIKE 'https://drop.example.com/%'"
        ),
        0
    );
    assert_eq!(
        read_i64(&places_path, "SELECT COUNT(*) FROM moz_historyvisits"),
        1
    );
    assert_eq!(
        read_i64(&places_path, "SELECT COUNT(*) FROM moz_bookmarks"),
        1
    );
}

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

fn seed_chromium_cookies(path: &std::path::Path) {
    let conn = Connection::open(path).expect("open cookies");
    conn.execute_batch(
        "CREATE TABLE cookies (
            host_key TEXT NOT NULL,
            name TEXT NOT NULL,
            value TEXT NOT NULL
        );",
    )
    .expect("schema");
    conn.execute(
        "INSERT INTO cookies(host_key, name, value) VALUES (?1, ?2, ?3)",
        params![".keep.example.com", "sid", "1"],
    )
    .expect("keep cookie");
    conn.execute(
        "INSERT INTO cookies(host_key, name, value) VALUES (?1, ?2, ?3)",
        params![".drop.example.com", "sid", "2"],
    )
    .expect("drop cookie");
}

fn seed_chromium_history(path: &std::path::Path) {
    let conn = Connection::open(path).expect("open history");
    conn.execute_batch(
        "CREATE TABLE urls (
            id INTEGER PRIMARY KEY,
            url TEXT NOT NULL
        );
        CREATE TABLE visits (
            id INTEGER PRIMARY KEY,
            url INTEGER NOT NULL
        );",
    )
    .expect("schema");
    conn.execute(
        "INSERT INTO urls(id, url) VALUES (1, ?1)",
        params!["https://keep.example.com/path"],
    )
    .expect("keep url");
    conn.execute(
        "INSERT INTO urls(id, url) VALUES (2, ?1)",
        params!["https://drop.example.com/path"],
    )
    .expect("drop url");
    conn.execute("INSERT INTO visits(id, url) VALUES (1, 1)", [])
        .expect("keep visit");
    conn.execute("INSERT INTO visits(id, url) VALUES (2, 2)", [])
        .expect("drop visit");
}

fn seed_firefox_cookies(path: &std::path::Path) {
    let conn = Connection::open(path).expect("open cookies");
    conn.execute_batch(
        "CREATE TABLE moz_cookies (
            id INTEGER PRIMARY KEY,
            host TEXT NOT NULL
        );",
    )
    .expect("schema");
    conn.execute(
        "INSERT INTO moz_cookies(id, host) VALUES (1, ?1)",
        params![".keep.example.com"],
    )
    .expect("keep cookie");
    conn.execute(
        "INSERT INTO moz_cookies(id, host) VALUES (2, ?1)",
        params![".drop.example.com"],
    )
    .expect("drop cookie");
}

fn seed_firefox_places(path: &std::path::Path) {
    let conn = Connection::open(path).expect("open places");
    conn.execute_batch(
        "CREATE TABLE moz_places (
            id INTEGER PRIMARY KEY,
            url TEXT
        );
        CREATE TABLE moz_historyvisits (
            id INTEGER PRIMARY KEY,
            place_id INTEGER NOT NULL
        );
        CREATE TABLE moz_bookmarks (
            id INTEGER PRIMARY KEY,
            fk INTEGER NOT NULL
        );
        CREATE TABLE moz_inputhistory (
            place_id INTEGER PRIMARY KEY
        );",
    )
    .expect("schema");
    conn.execute(
        "INSERT INTO moz_places(id, url) VALUES (1, ?1)",
        params!["https://keep.example.com/path"],
    )
    .expect("keep place");
    conn.execute(
        "INSERT INTO moz_places(id, url) VALUES (2, ?1)",
        params!["https://drop.example.com/path"],
    )
    .expect("drop place");
    conn.execute(
        "INSERT INTO moz_places(id, url) VALUES (3, ?1)",
        params!["https://bookmark-only.example.com/path"],
    )
    .expect("bookmark place");
    conn.execute(
        "INSERT INTO moz_historyvisits(id, place_id) VALUES (1, 1)",
        [],
    )
    .expect("keep visit");
    conn.execute(
        "INSERT INTO moz_historyvisits(id, place_id) VALUES (2, 2)",
        [],
    )
    .expect("drop visit");
    conn.execute("INSERT INTO moz_bookmarks(id, fk) VALUES (1, 3)", [])
        .expect("bookmark");
    conn.execute("INSERT INTO moz_inputhistory(place_id) VALUES (2)", [])
        .expect("inputhistory");
}

fn read_i64(path: &std::path::Path, sql: &str) -> i64 {
    let conn = Connection::open(path).expect("open sqlite");
    conn.query_row(sql, [], |row| row.get(0)).expect("query")
}
