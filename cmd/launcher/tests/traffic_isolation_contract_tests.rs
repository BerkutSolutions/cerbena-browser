use std::{fs, path::Path};

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

#[test]
fn desktop_backend_declares_traffic_isolation_regression_tests() {
    let root = repo_root();
    let sandbox_tests = fs::read_to_string(
        root.join("ui")
            .join("desktop")
            .join("src-tauri")
            .join("src")
            .join("network_sandbox.rs"),
    )
    .expect("read network_sandbox.rs");

    for needle in [
        "traffic_isolation_prefers_userspace_for_non_native_routes",
        "traffic_isolation_blocks_native_routes_without_explicit_fallback",
        "traffic_isolation_allows_native_container_mode_when_requested",
    ] {
        assert!(
            sandbox_tests.contains(needle),
            "network sandbox regressions must cover {needle}"
        );
    }
}

#[test]
fn release_and_github_gates_run_traffic_isolation_regressions() {
    let root = repo_root();
    let local_preflight = fs::read_to_string(root.join("scripts").join("local-ci-preflight.ps1"))
        .expect("read local-ci-preflight.ps1");
    let workflow = fs::read_to_string(
        root.join(".github")
            .join("workflows")
            .join("security-regression-gate.yml"),
    )
    .expect("read security-regression-gate.yml");

    assert!(
        local_preflight.contains("Traffic isolation regression tests"),
        "local preflight must expose a dedicated traffic isolation step"
    );
    assert!(
        local_preflight.contains("traffic_isolation"),
        "local preflight must invoke the traffic isolation regression filter"
    );
    assert!(
        workflow.contains("traffic-isolation-regression"),
        "GitHub security regression workflow must run traffic isolation regressions"
    );
    assert!(
        workflow.contains("cargo test traffic_isolation"),
        "GitHub security regression workflow must execute the traffic isolation filter"
    );
}
