use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn current_version(root: &Path) -> String {
    let tauri_config = fs::read_to_string(
        root.join("ui")
            .join("desktop")
            .join("src-tauri")
            .join("tauri.conf.json"),
    )
    .expect("read tauri config");
    let json: Value = serde_json::from_str(&tauri_config).expect("parse tauri config");
    json.get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn version_manifest_paths(root: &Path) -> BTreeSet<String> {
    let manifest = fs::read_to_string(root.join("scripts").join("version-sync-targets.json"))
        .expect("read version sync manifest");
    let json: Value = serde_json::from_str(&manifest).expect("parse version sync manifest");
    json.get("targets")
        .and_then(Value::as_array)
        .expect("version sync targets array")
        .iter()
        .map(|entry| {
            entry
                .get("path")
                .and_then(Value::as_str)
                .expect("version sync target path")
                .replace('\\', "/")
        })
        .collect()
}

fn collect_version_literal_paths(root: &Path, version: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    walk_version_paths(root, root, version, &mut found);
    found
}

fn walk_version_paths(root: &Path, current: &Path, version: &str, found: &mut BTreeSet<String>) {
    let skip_dirs = [
        ".docusaurus",
        ".git",
        ".work",
        "target",
        "node_modules",
        "build",
    ];
    let allowed_exts = ["toml", "lock", "json", "js", "jsx", "rs", "md", "ps1"];
    let entries = fs::read_dir(current).expect("read repo directory");
    for entry in entries {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if skip_dirs.contains(&name.as_ref()) {
                continue;
            }
            walk_version_paths(root, &path, version, found);
            continue;
        }
        let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if !allowed_exts.contains(&ext) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if content.contains(version) {
            let relative = path
                .strip_prefix(root)
                .expect("relative version path")
                .to_string_lossy()
                .replace('\\', "/");
            found.insert(relative);
        }
    }
}

#[test]
fn release_scripts_exist_and_reference_current_quality_gates() {
    let root = repo_root();
    let artifacts_script =
        fs::read_to_string(root.join("scripts").join("generate-release-artifacts.ps1"))
            .expect("read generate-release-artifacts.ps1");
    let installer_script = fs::read_to_string(root.join("scripts").join("build-installer.ps1"))
        .expect("read build-installer.ps1");
    let release_script =
        fs::read_to_string(root.join("scripts").join("release.ps1")).expect("read release.ps1");
    let version_script = fs::read_to_string(root.join("scripts").join("update-version.ps1"))
        .expect("read update-version.ps1");
    let version_manifest =
        fs::read_to_string(root.join("scripts").join("version-sync-targets.json"))
            .expect("read version-sync-targets.json");
    let signing_helper =
        fs::read_to_string(root.join("scripts").join("release-signing.ps1"))
            .expect("read release-signing.ps1");
    let signing_bootstrap =
        fs::read_to_string(root.join("scripts").join("new-release-signing-material.ps1"))
            .expect("read new-release-signing-material.ps1");
    let release_public_key = fs::read_to_string(
        root.join("config")
            .join("release")
            .join("release-signing-public-key.xml"),
    )
    .expect("read release signing public key");

    for needle in [
        "checksums.sig",
        "cargo",
        "npm.cmd",
        "cerbena-windows-x64.zip",
        "cerbena-browser-setup-",
        "cerbena-updater.exe",
        "release-manifest.json",
        "checksums.txt",
        "checksums.sig",
        ".msi",
        "Assert-GitHubReleaseAssetsPublished",
        ".assets[].name",
        "update-version.ps1",
        "version-sync-targets.json",
    ] {
        assert!(
            artifacts_script.contains(needle)
                || installer_script.contains(needle)
                || release_script.contains(needle)
                || signing_helper.contains(needle)
                || signing_bootstrap.contains(needle)
                || release_public_key.contains(needle)
                || version_script.contains(needle)
                || version_manifest.contains(needle),
            "release pipeline scripts must mention {needle}"
        );
    }

    for needle in [
        "ISCC.exe",
        "wix.exe",
        "localappdata}\\Cerbena Browser",
        "cerbena-browser-setup",
        "cerbena-browser-",
        "\"msi\"",
        "\"direct_msi\"",
        "Primary $true",
        "manual_installer",
    ] {
        assert!(
            installer_script.contains(needle),
            "installer build script must mention {needle}"
        );
    }

    assert!(
        artifacts_script.contains("release-signing.ps1"),
        "generate-release-artifacts.ps1 must source release-signing.ps1"
    );
    assert!(
        installer_script.contains("release-signing.ps1"),
        "build-installer.ps1 must source release-signing.ps1"
    );
    assert!(
        signing_helper.contains("CERBENA_AUTHENTICODE_PFX_PATH")
            && signing_helper.contains("CERBENA_AUTHENTICODE_PFX_PASSWORD"),
        "release signing helper must require operator-provided Authenticode secrets"
    );
    assert!(
        !artifacts_script.contains("<D>") && !artifacts_script.contains("<P>"),
        "generate-release-artifacts.ps1 must not embed a private RSA key"
    );
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
    assert!(ci_quality
        .contains("cargo test -p cerbena-launcher --test release_pipeline_contract_tests"));

    let local_preflight = fs::read_to_string(root.join("scripts").join("local-ci-preflight.ps1"))
        .expect("read local ci preflight");
    let release_script =
        fs::read_to_string(root.join("scripts").join("release.ps1")).expect("read release script");
    assert!(local_preflight.contains("Trusted updater regression tests"));
    assert!(local_preflight.contains("Published updater end-to-end test"));
    assert!(local_preflight.contains("Release pipeline contract"));
    assert!(local_preflight.contains("published-updater-e2e.ps1"));
    assert!(local_preflight.contains("cargo"));
    assert!(local_preflight.contains("trusted_updater"));
    assert!(local_preflight.contains("release_scripts_exist_and_reference_current_quality_gates"));
    assert!(local_preflight.contains("Desktop UI dev smoke"));
    assert!(local_preflight.contains("npm.cmd run dev"));
    assert!(local_preflight.contains("Version sync contract"));
    assert!(local_preflight.contains("version_sync_contract"));
    assert!(release_script.contains("1. Change version"));
    assert!(release_script.contains("2. Full cycle"));
    assert!(release_script.contains("3. Publish only"));
    assert!(release_script.contains("4. Checks only"));
    assert!(release_script.contains("update-version.ps1"));
    assert!(release_script.contains("required legacy EXE installer is missing for compatibility"));
    assert!(release_script.contains("required MSI installer is missing"));

    let security_supply = fs::read_to_string(workflows.join("security-supply-chain.yml"))
        .expect("read security-supply-chain workflow");
    for needle in [
        "dependency-review",
        "cargo-audit",
        "npm audit",
        "gitleaks",
        "trivy",
    ] {
        assert!(
            security_supply.contains(needle),
            "security-supply-chain workflow must mention {needle}"
        );
    }
}

#[test]
fn version_sync_contract_covers_current_version_surfaces() {
    let root = repo_root();
    let version = current_version(&root);
    let manifest_paths = version_manifest_paths(&root);
    let literal_paths = collect_version_literal_paths(&root, &version);

    let uncovered = literal_paths
        .difference(&manifest_paths)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        uncovered.is_empty(),
        "current version literal appears in files not registered in scripts/version-sync-targets.json: {:?}",
        uncovered
    );

    let missing = manifest_paths
        .difference(&literal_paths)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "version sync manifest includes files that do not currently contain version {version}: {:?}",
        missing
    );
}
