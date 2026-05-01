use std::{fs, path::Path};

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

#[test]
fn release_scripts_exist_and_reference_current_quality_gates() {
    let root = repo_root();
    let release_script =
        fs::read_to_string(root.join("scripts").join("release.ps1")).expect("read release.ps1");
    let artifacts_script = fs::read_to_string(root.join("scripts").join("generate-release-artifacts.ps1"))
        .expect("read generate-release-artifacts.ps1");
    let installer_script =
        fs::read_to_string(root.join("scripts").join("build-installer.ps1")).expect("read build-installer.ps1");

    for needle in [
        "local-ci-preflight.ps1",
        "generate-release-artifacts.ps1",
        "cargo",
        "npm.cmd",
        "cerbena-windows-x64.zip",
        "release-manifest.json",
        "checksums.txt",
        "https://github.com/BerkutSolutions/cerbena-browser.git",
        "git init",
        "git push",
        "v$Version",
    ] {
        assert!(
            release_script.contains(needle)
                || artifacts_script.contains(needle)
                || installer_script.contains(needle),
            "release pipeline scripts must mention {needle}"
        );
    }

    for needle in ["ISCC.exe", "localappdata}\\Cerbena Browser", "cerbena-browser-setup"] {
        assert!(
            installer_script.contains(needle),
            "installer build script must mention {needle}"
        );
    }
}

#[test]
fn github_workflows_cover_docs_quality_and_security_gates() {
    let root = repo_root();
    let workflows = root.join(".github").join("workflows");
    let files = [
        "docs-pages.yml",
        "ci-quality.yml",
        "security-supply-chain.yml",
        "security-regression-gate.yml",
        "smoke-e2e.yml",
    ];

    for file in files {
        let path = workflows.join(file);
        assert!(path.exists(), "missing workflow {}", path.display());
    }

    let ci_quality =
        fs::read_to_string(workflows.join("ci-quality.yml")).expect("read ci-quality workflow");
    assert!(ci_quality.contains("npm run docs:build"));
    assert!(ci_quality.contains("cargo test --workspace"));
    assert!(ci_quality.contains("scripts\\release.ps1") || ci_quality.contains("./scripts/release.ps1"));

    let security_supply = fs::read_to_string(workflows.join("security-supply-chain.yml"))
        .expect("read security-supply-chain workflow");
    for needle in ["dependency-review", "cargo-audit", "npm audit", "gitleaks", "trivy"] {
        assert!(
            security_supply.contains(needle),
            "security-supply-chain workflow must mention {needle}"
        );
    }
}
