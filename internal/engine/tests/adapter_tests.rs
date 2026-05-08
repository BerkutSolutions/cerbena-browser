use browser_engine::{
    chromium::ChromiumAdapter,
    contract::{EngineAdapter, LaunchRequest},
    ungoogled_chromium::UngoogledChromiumAdapter,
    LibrewolfAdapter,
};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn librewolf_builds_launch_plan() {
    let tmp = tempdir().expect("tempdir");
    let adapter = LibrewolfAdapter {
        install_root: tmp.path().join("install"),
        cache_dir: tmp.path().join("cache"),
    };
    let req = LaunchRequest {
        profile_id: Uuid::new_v4(),
        profile_root: tmp.path().join("profile"),
        binary_path: tmp.path().join("bin").join("librewolf.exe"),
        args: vec!["--profile".to_string(), "x".to_string()],
        env: vec![("LANG".to_string(), "en-US.UTF-8".to_string())],
    };
    let plan = adapter.build_launch_plan(req).expect("plan");
    assert_eq!(plan.args.len(), 2);
    assert_eq!(plan.env.len(), 1);
}

#[test]
fn chromium_builds_launch_plan_without_tos_gate() {
    let tmp = tempdir().expect("tempdir");
    let adapter = ChromiumAdapter {
        install_root: tmp.path().join("install"),
        cache_dir: tmp.path().join("cache"),
    };
    let req = LaunchRequest {
        profile_id: Uuid::new_v4(),
        profile_root: tmp.path().join("profile"),
        binary_path: tmp.path().join("bin").join("chromium.exe"),
        args: vec!["--profile-directory=Default".to_string()],
        env: vec![("LANG".to_string(), "en-US.UTF-8".to_string())],
    };
    let plan = adapter.build_launch_plan(req).expect("plan");
    assert_eq!(plan.engine, browser_engine::EngineKind::Chromium);
    assert_eq!(plan.args.len(), 1);
    assert_eq!(plan.env.len(), 1);
}

#[test]
fn ungoogled_chromium_builds_launch_plan_without_vendor_specific_gate() {
    let tmp = tempdir().expect("tempdir");
    let adapter = UngoogledChromiumAdapter {
        install_root: tmp.path().join("install"),
        cache_dir: tmp.path().join("cache"),
    };
    let req = LaunchRequest {
        profile_id: Uuid::new_v4(),
        profile_root: tmp.path().join("profile"),
        binary_path: tmp.path().join("bin").join("ungoogled-chromium.exe"),
        args: vec!["--profile-directory=Default".to_string()],
        env: vec![("LANG".to_string(), "en-US.UTF-8".to_string())],
    };
    let plan = adapter.build_launch_plan(req).expect("plan");
    assert_eq!(plan.engine, browser_engine::EngineKind::UngoogledChromium);
    assert_eq!(plan.args.len(), 1);
    assert_eq!(plan.env.len(), 1);
}
