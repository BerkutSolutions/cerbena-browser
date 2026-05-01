use std::{fs, path::Path};

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

#[test]
fn local_ci_preflight_invokes_git_hygiene_checks() {
    let root = repo_root();
    let script_path = root.join("scripts").join("git-hygiene-preflight.ps1");
    assert!(script_path.exists(), "missing {}", script_path.display());

    let preflight = fs::read_to_string(root.join("scripts").join("local-ci-preflight.ps1"))
        .expect("read local-ci-preflight.ps1");
    assert!(
        preflight.contains("git-hygiene-preflight.ps1"),
        "local preflight must invoke git-hygiene-preflight.ps1"
    );
}

#[test]
fn gitignore_marks_worktree_and_artifacts_as_local_only() {
    let root = repo_root();
    let gitignore = fs::read_to_string(root.join(".gitignore")).expect("read .gitignore");

    for needle in [
        ".work/",
        ".cache/",
        "build/",
        "target/",
        "node_modules/",
        "docs/build/",
        "ui/desktop/src-tauri/target/",
    ] {
        assert!(
            gitignore.contains(needle),
            ".gitignore must include {needle}"
        );
    }
}
