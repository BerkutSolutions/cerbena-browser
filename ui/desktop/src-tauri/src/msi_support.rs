use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::install_registration;

const CLEANUP_FILES: &[&str] = &[
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
];

const CLEANUP_DIRS: &[&str] = &[
    "profiles",
    "engine-runtime",
    "network-runtime",
    "extension-packages",
    "updates",
    "native-messaging",
];

pub fn handle_maintenance_cli() -> Result<bool, String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args
        .iter()
        .any(|value| value.eq_ignore_ascii_case("--reconcile-install-registration"))
    {
        install_registration::register_browser_capabilities_for_current_install()?;
        return Ok(true);
    }
    if args
        .iter()
        .any(|value| value.eq_ignore_ascii_case("--msi-cleanup"))
    {
        run_msi_cleanup()?;
        return Ok(true);
    }
    Ok(false)
}

fn run_msi_cleanup() -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("resolve current exe: {e}"))?;
    let install_root = current_exe
        .parent()
        .ok_or_else(|| "resolve install root for MSI cleanup".to_string())?
        .to_path_buf();

    let _ = install_registration::remove_browser_capabilities();
    cleanup_local_state(&install_root);
    cleanup_legacy_amnezia_services(&install_root);
    cleanup_managed_container_artifacts();
    Ok(())
}

fn cleanup_local_state(install_root: &Path) {
    for file_name in CLEANUP_FILES {
        let path = install_root.join(file_name);
        let _ = fs::remove_file(path);
    }
    for dir_name in CLEANUP_DIRS {
        let path = install_root.join(dir_name);
        let _ = fs::remove_dir_all(path);
    }
}

fn cleanup_legacy_amnezia_services(install_root: &Path) {
    let mut service_names = std::collections::BTreeSet::new();
    let profiles_root = install_root.join("profiles");
    if profiles_root.is_dir() {
        collect_matching_files(&profiles_root, &mut |path| {
            if let Some(file_name) = path.file_name().and_then(|value| value.to_str()) {
                if file_name.starts_with("awg-") && file_name.ends_with(".conf") {
                    let tunnel_name = file_name.trim_end_matches(".conf");
                    service_names.insert(format!("AmneziaWGTunnel$awg-{tunnel_name}"));
                }
            }
        });
    }

    if let Ok(output) = run_hidden_process("sc.exe", &["query", "state=", "all"]) {
        for line in output.lines() {
            let trimmed = line.trim();
            if let Some(service_name) = trimmed.strip_prefix("SERVICE_NAME:") {
                let normalized = service_name.trim();
                if normalized.starts_with("AmneziaWGTunnel$awg-") {
                    service_names.insert(normalized.to_string());
                }
            }
        }
    }

    for service_name in service_names {
        let _ = run_hidden_process("sc.exe", &["stop", &service_name]);
        let _ = run_hidden_process("sc.exe", &["delete", &service_name]);
    }
}

fn cleanup_managed_container_artifacts() {
    if let Ok(output) = run_hidden_process(
        "docker.exe",
        &[
            "ps",
            "-a",
            "--filter",
            "label=cerbena.kind=network-sandbox-runtime",
            "--format",
            "{{.Names}}",
        ],
    ) {
        for name in output
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let _ = run_hidden_process("docker.exe", &["rm", "-f", name]);
        }
    }

    if let Ok(output) =
        run_hidden_process("docker.exe", &["network", "ls", "--format", "{{.Name}}"])
    {
        for name in output
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if name.starts_with("cerbena-profile-") {
                let _ = run_hidden_process("docker.exe", &["network", "rm", name]);
            }
        }
    }

    let _ = run_hidden_process(
        "docker.exe",
        &["image", "rm", "-f", "cerbena/network-sandbox:2026-05-02-r5"],
    );
}

fn run_hidden_process(command: &str, arguments: &[&str]) -> Result<String, String> {
    let mut process = Command::new(command);
    process.args(arguments);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        process.creation_flags(0x08000000);
    }
    let output = process
        .output()
        .map_err(|e| format!("run {command}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{command} {:?} failed with code {:?}: {}",
            arguments,
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn collect_matching_files(root: &Path, visitor: &mut dyn FnMut(&Path)) {
    let mut stack = vec![PathBuf::from(root)];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                visitor(&path);
            }
        }
    }
}
