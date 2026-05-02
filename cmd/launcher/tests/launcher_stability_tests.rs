use cerbena_launcher::run_with_args;
use tempfile::tempdir;

#[test]
fn launcher_wayfern_ack_and_launch_plan_are_repeatable() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().to_string_lossy().to_string();

    let profile_id = run_with_args(&[
        "init-profile".to_string(),
        "--root".to_string(),
        root.clone(),
        "--name".to_string(),
        "Stable Wayfern".to_string(),
        "--engine".to_string(),
        "wayfern".to_string(),
    ])
    .expect("init profile");
    let profile_id = profile_id.trim().to_string();

    let binary_path = temp
        .path()
        .join("bin")
        .join("wayfern.exe")
        .to_string_lossy()
        .to_string();

    let before_ack = run_with_args(&[
        "build-launch-plan".to_string(),
        "--root".to_string(),
        root.clone(),
        "--profile-id".to_string(),
        profile_id.clone(),
        "--binary".to_string(),
        binary_path.clone(),
    ]);
    assert!(before_ack.is_err(), "launch plan must fail before ToS ack");

    run_with_args(&[
        "ack-wayfern-tos".to_string(),
        "--root".to_string(),
        root.clone(),
        "--profile-id".to_string(),
        profile_id.clone(),
    ])
    .expect("ack wayfern tos");

    for _ in 0..3 {
        let plan = run_with_args(&[
            "build-launch-plan".to_string(),
            "--root".to_string(),
            root.clone(),
            "--profile-id".to_string(),
            profile_id.clone(),
            "--binary".to_string(),
            binary_path.clone(),
        ])
        .expect("build launch plan");
        assert!(plan.contains("Wayfern"));
    }
}

#[test]
fn launcher_multi_profile_listing_remains_stable() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().to_string_lossy().to_string();
    let names = [
        ("Alpha", "camoufox"),
        ("Beta", "wayfern"),
        ("Gamma", "camoufox"),
    ];

    for (name, engine) in names {
        run_with_args(&[
            "init-profile".to_string(),
            "--root".to_string(),
            root.clone(),
            "--name".to_string(),
            name.to_string(),
            "--engine".to_string(),
            engine.to_string(),
        ])
        .expect("init profile");
    }

    let first = run_with_args(&[
        "list-profiles".to_string(),
        "--root".to_string(),
        root.clone(),
    ])
    .expect("first list");
    let second = run_with_args(&[
        "list-profiles".to_string(),
        "--root".to_string(),
        root.clone(),
    ])
    .expect("second list");

    for (name, _) in names {
        assert!(
            first.contains(name),
            "missing profile in first list: {name}"
        );
        assert!(
            second.contains(name),
            "missing profile in second list: {name}"
        );
    }

    let first_count = first.lines().filter(|line| !line.trim().is_empty()).count();
    let second_count = second
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert_eq!(first_count, 3);
    assert_eq!(second_count, 3);
}

#[test]
fn launcher_manual_update_flow_is_repeatable() {
    for version in ["1.0.6", "1.0.7", "1.0.8"] {
        let out = run_with_args(&[
            "update-apply".to_string(),
            "--version".to_string(),
            version.to_string(),
            "--signature".to_string(),
            "sig-ok".to_string(),
        ])
        .expect("update apply");
        assert!(out.contains(version));
    }
}
