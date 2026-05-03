use std::{fs, sync::Mutex};

use browser_engine::{
    contract::{EngineAdapter, LaunchRequest},
    wayfern::WayfernAdapter,
    CamoufoxAdapter,
};
use tempfile::tempdir;
use uuid::Uuid;

static APPDATA_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn camoufox_builds_launch_plan() {
    let tmp = tempdir().expect("tempdir");
    let adapter = CamoufoxAdapter {
        install_root: tmp.path().join("install"),
        cache_dir: tmp.path().join("cache"),
    };
    let req = LaunchRequest {
        profile_id: Uuid::new_v4(),
        profile_root: tmp.path().join("profile"),
        binary_path: tmp.path().join("bin").join("camoufox.exe"),
        args: vec!["--profile".to_string(), "x".to_string()],
    };
    let plan = adapter.build_launch_plan(req).expect("plan");
    assert_eq!(plan.args.len(), 2);
}

#[test]
fn wayfern_requires_tos_ack() {
    let _guard = APPDATA_ENV_LOCK.lock().expect("lock appdata env");
    let tmp = tempdir().expect("tempdir");
    let appdata_root = tmp.path().join("appdata");
    fs::create_dir_all(&appdata_root).expect("mk appdata");
    let previous_appdata = std::env::var_os("APPDATA");
    std::env::set_var("APPDATA", &appdata_root);
    let profile_root = tmp.path().join("profile");
    fs::create_dir_all(&profile_root).expect("mk profile");
    let profile_id = Uuid::new_v4();
    let adapter = WayfernAdapter {
        install_root: tmp.path().join("install"),
        cache_dir: tmp.path().join("cache"),
        tos_version: "2026-04".to_string(),
    };

    let req = LaunchRequest {
        profile_id,
        profile_root: profile_root.clone(),
        binary_path: tmp.path().join("bin").join("wayfern.exe"),
        args: vec![],
    };
    assert!(adapter.build_launch_plan(req.clone()).is_err());

    let result = std::panic::catch_unwind(|| {
        adapter
            .acknowledge_tos(&profile_root, profile_id)
            .expect("ack");
        assert!(adapter.build_launch_plan(req).is_ok());
    });

    match previous_appdata {
        Some(value) => std::env::set_var("APPDATA", value),
        None => std::env::remove_var("APPDATA"),
    }
    result.expect("wayfern ack flow")
}
