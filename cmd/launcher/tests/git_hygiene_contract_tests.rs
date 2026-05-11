use std::{fs, path::Path};

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
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
        "ui/desktop/$null",
    ] {
        assert!(
            gitignore.contains(needle),
            ".gitignore must include {needle}"
        );
    }
}
