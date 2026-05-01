use std::fs;

use browser_engine::{
    contract::{EngineAdapter, LaunchRequest},
    wayfern::WayfernAdapter,
    CamoufoxAdapter,
};
use tempfile::tempdir;
use uuid::Uuid;

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
    let tmp = tempdir().expect("tempdir");
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

    adapter
        .acknowledge_tos(&profile_root, profile_id)
        .expect("ack");
    assert!(adapter.build_launch_plan(req).is_ok());
}
