use std::{fs, path::Path};

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

#[test]
fn installer_cleanup_covers_local_state_and_profile_runtime_roots() {
    let root = repo_root();
    let installer = fs::read_to_string(root.join("scripts").join("build-installer.ps1"))
        .expect("read build-installer.ps1");

    for needle in [
        ".app-secret.dpapi",
        "identity_store.json",
        "network_store.json",
        "network_sandbox_store.json",
        "extension_library.json",
        "sync_store.json",
        "link_routing_store.json",
        "launch_session_store.json",
        "device_posture_store.json",
        "app_update_store.json",
        "global_security_store.json",
        "traffic_gateway_log.json",
        "traffic_gateway_rules.json",
        "profiles",
        "engine-runtime",
        "network-runtime",
        "extension-packages",
        "updates",
        "native-messaging",
    ] {
        assert!(
            installer.contains(needle),
            "installer cleanup must mention {needle}"
        );
    }
}

#[test]
fn installer_cleanup_covers_managed_docker_artifacts_and_legacy_services() {
    let root = repo_root();
    let installer = fs::read_to_string(root.join("scripts").join("build-installer.ps1"))
        .expect("read build-installer.ps1");

    for needle in [
        "CleanupManagedContainerArtifacts",
        "cerbena.kind=network-sandbox-runtime",
        "docker.exe",
        "network rm",
        "image rm -f cerbena/network-sandbox:2026-05-02-r5",
        "LegacyAmneziaServicePrefix",
        "AmneziaWGTunnel`$awg-",
    ] {
        assert!(
            installer.contains(needle),
            "installer cleanup must mention {needle}"
        );
    }
}
