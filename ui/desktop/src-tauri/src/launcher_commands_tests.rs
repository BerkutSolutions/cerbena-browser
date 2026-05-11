use super::*;
use crate::sensitive_store::derive_app_secret_material;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("cerbena-{label}-{unique}.json"))
}

fn temp_dir_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("cerbena-{label}-{unique}"))
}

#[test]
fn extract_blocklist_title_reads_comment_title_marker() {
    let content = r#"
!
! Title: AdGuard DNS filter
! Description: sample
!
||example.com^
"#;
    let title = security_store::extract_blocklist_title_impl(content).expect("title");
    assert_eq!(title, "AdGuard DNS filter");
}

#[test]
fn merge_default_dns_blocklists_keeps_existing_activity() {
    let existing = vec![ManagedBlocklistRecord {
        id: "custom-id".to_string(),
        name: "Custom name".to_string(),
        source_kind: "url".to_string(),
        source_value: "https://adguardteam.github.io/HostlistsRegistry/assets/filter_1.txt"
            .to_string(),
        active: true,
        domains: vec!["example.com".to_string()],
        updated_at_epoch: 123,
    }];
    let merged = security_store::merge_default_dns_blocklists_impl(existing);
    let item = merged
        .into_iter()
        .find(|value| {
            value.source_value
                == "https://adguardteam.github.io/HostlistsRegistry/assets/filter_1.txt"
        })
        .expect("default list is present");
    assert!(item.active);
    assert_eq!(item.domains, vec!["example.com".to_string()]);
}

#[test]
fn detect_link_type_rejects_cli_flags() {
    assert!(detect_link_type("--updater").is_err());
    assert!(detect_link_type("--updater-preview").is_err());
}

#[test]
fn global_security_store_encrypts_startup_page_and_certificate_paths() {
    let path = temp_path("global-security-store");
    let legacy = temp_path("global-security-legacy");
    let app_data_dir = temp_dir_path("global-security-store-app-data");
    let binary_path = app_data_dir.join("cerbena.exe");
    let secret = derive_app_secret_material(&app_data_dir, &binary_path, "dev.cerbena.app")
        .expect("derive secret");
    let payload = GlobalSecuritySettingsRecord {
        startup_page: Some("https://duckduckgo.com".to_string()),
        certificates: vec![ManagedCertificateRecord {
            id: "cert-a".to_string(),
            name: "Cert A".to_string(),
            path: "C:/secret/cert.pem".to_string(),
            issuer_name: None,
            subject_name: None,
            apply_globally: true,
            profile_ids: Vec::new(),
        }],
        blocked_domain_suffixes: vec!["example".to_string()],
        blocklists: Vec::new(),
    };

    persist_global_security_record_to_paths(&path, &legacy, &secret, &payload).expect("persist");
    let on_disk = fs::read_to_string(&path).expect("read");
    assert!(!on_disk.contains("duckduckgo"));
    assert!(!on_disk.contains("C:/secret/cert.pem"));

    let loaded = load_global_security_record_from_paths(&path, &legacy, &secret).expect("load");
    assert_eq!(loaded.startup_page, payload.startup_page);
    assert_eq!(loaded.certificates[0].path, "C:/secret/cert.pem".to_string());

    let _ = fs::remove_file(path);
    let _ = fs::remove_dir_all(app_data_dir);
}

#[test]
fn global_security_store_reads_legacy_plaintext_file() {
    let path = temp_path("global-security-store-new");
    let legacy = temp_path("global-security-legacy-old");
    let app_data_dir = temp_dir_path("global-security-legacy-app-data");
    let binary_path = app_data_dir.join("cerbena.exe");
    let secret = derive_app_secret_material(&app_data_dir, &binary_path, "dev.cerbena.app")
        .expect("derive secret");
    fs::write(
        &legacy,
        r#"{"startup_page":"https://legacy.test","certificates":["C:/legacy/cert.pem"],"blocked_domain_suffixes":["legacy"]}"#,
    )
    .expect("write legacy");

    let loaded =
        load_global_security_record_from_paths(&path, &legacy, &secret).expect("load legacy");
    assert_eq!(loaded.startup_page, Some("https://legacy.test".to_string()));
    assert_eq!(loaded.certificates[0].path, "C:/legacy/cert.pem".to_string());

    let _ = fs::remove_file(legacy);
    let _ = fs::remove_dir_all(app_data_dir);
}

#[test]
fn merge_panic_retain_paths_normalizes_domains_and_avoids_duplicates() {
    let profile = browser_profile::ProfileMetadata {
        id: uuid::Uuid::new_v4(),
        name: "Panic".to_string(),
        description: None,
        tags: Vec::new(),
        engine: browser_profile::Engine::Chromium,
        state: browser_profile::ProfileState::Ready,
        default_start_page: None,
        default_search_provider: None,
        ephemeral_mode: false,
        password_lock_enabled: false,
        panic_frame_enabled: true,
        panic_frame_color: None,
        panic_protected_sites: vec![
            " Example.COM ".to_string(),
            "example.com".to_string(),
            "".to_string(),
            "Sub.Domain.test".to_string(),
        ],
        crypto_version: 1,
        ephemeral_retain_paths: Vec::new(),
        created_at: "2026-05-01T00:00:00Z".to_string(),
        updated_at: "2026-05-01T00:00:00Z".to_string(),
    };

    let merged = operator::merge_panic_retain_paths_impl(
        &profile,
        &[
            "manual/path".to_string(),
            "data/cookies/example.com".to_string(),
            "data/history/sub.domain.test".to_string(),
        ],
    );

    assert_eq!(
        merged,
        vec![
            "manual/path".to_string(),
            "data/cookies/example.com".to_string(),
            "data/history/sub.domain.test".to_string(),
            "data/history/example.com".to_string(),
            "data/cookies/sub.domain.test".to_string(),
        ]
    );
}
