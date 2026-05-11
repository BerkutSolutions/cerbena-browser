use super::*;

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

