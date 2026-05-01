use std::{fs, path::Path};

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

#[test]
fn security_gates_preflight_is_wired_into_local_preflight() {
    let root = repo_root();
    let script_path = root.join("scripts").join("security-gates-preflight.ps1");
    assert!(script_path.exists(), "missing {}", script_path.display());

    let preflight = fs::read_to_string(root.join("scripts").join("local-ci-preflight.ps1"))
        .expect("read local preflight");
    assert!(
        preflight.contains("security-gates-preflight.ps1"),
        "local preflight must invoke security-gates-preflight.ps1"
    );
}

#[test]
fn security_gates_preflight_covers_tasks4_release_artifacts() {
    let root = repo_root();
    let script = fs::read_to_string(root.join("scripts").join("security-gates-preflight.ps1"))
        .expect("read security gates preflight");
    for needle in [
        "docs\\eng\\operators\\security-validation.md",
        "docs\\ru\\operators\\security-validation.md",
        "git-hygiene-preflight.ps1",
        "cargo",
        "npm.cmd"
    ] {
        assert!(
            script.contains(needle),
            "security gates preflight must mention {needle}"
        );
    }
}
