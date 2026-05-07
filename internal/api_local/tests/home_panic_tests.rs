use browser_api_local::{HomePageService, PanicMode, PanicWipeService};
use browser_profile::{CreateProfileInput, Engine, ProfileManager};
use tempfile::tempdir;

#[test]
fn home_dashboard_includes_metrics_and_actions() {
    let profile_id = uuid::Uuid::new_v4();
    let service = HomePageService;
    let model = service.build_dashboard(profile_id, 10, 3, 2, false);
    assert_eq!(model.metrics.len(), 3);
    assert_eq!(model.quick_actions.len(), 3);
}

#[test]
fn panic_wipe_requires_confirmation_and_wipes_data() {
    let temp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(temp.path()).expect("manager");
    let profile = manager
        .create_profile(CreateProfileInput {
            name: "panic".to_string(),
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
        .expect("profile");
    let root = temp.path().join(profile.id.to_string());
    std::fs::create_dir_all(root.join("data").join("cookies")).expect("cookies");
    std::fs::write(root.join("data").join("cookies").join("a"), b"1").expect("cookie write");
    let panic = PanicWipeService;
    assert!(panic
        .execute(
            &manager,
            profile.id,
            PanicMode::Full,
            vec![],
            vec![],
            "WRONG",
            "qa-user",
        )
        .is_err());
    let summary = panic
        .execute(
            &manager,
            profile.id,
            PanicMode::Full,
            vec![],
            vec![],
            "ERASE_NOW",
            "qa-user",
        )
        .expect("panic ok");
    assert!(summary.wiped_paths > 0);
}
