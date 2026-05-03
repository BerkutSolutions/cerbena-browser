use cerbena_launcher::run_with_args;
use tempfile::tempdir;

#[test]
fn launcher_can_init_and_list_profiles() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().to_string_lossy().to_string();

    let out = run_with_args(&[
        "init-profile".to_string(),
        "--root".to_string(),
        root.clone(),
        "--name".to_string(),
        "CLI Profile".to_string(),
        "--engine".to_string(),
        "camoufox".to_string(),
    ])
    .expect("init profile");
    assert!(!out.trim().is_empty());

    let listed = run_with_args(&[
        "list-profiles".to_string(),
        "--root".to_string(),
        root.clone(),
    ])
    .expect("list");
    assert!(listed.contains("CLI Profile"));
}

#[test]
fn launcher_update_apply_manual_path_works() {
    let out = run_with_args(&[
        "update-apply".to_string(),
        "--version".to_string(),
        "1.0.11".to_string(),
        "--signature".to_string(),
        "sig-ok".to_string(),
    ])
    .expect("update apply");
    assert!(out.contains("updated_to_1.0.11"));
}
