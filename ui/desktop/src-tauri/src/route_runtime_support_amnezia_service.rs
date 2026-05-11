use super::*;

#[derive(Debug, Clone)]
pub(super) struct AmneziaServiceSnapshot {
    pub(super) exists: bool,
    pub(super) state_code: Option<u32>,
    pub(super) raw_output: String,
}

pub(super) fn uninstall_amnezia_tunnel_impl(
    binary: &PathBuf,
    tunnel_name: &str,
) -> Result<(), String> {
    let args = vec![
        "/uninstalltunnelservice".to_string(),
        tunnel_name.to_string(),
    ];
    let output = run_amneziawg_command_impl(binary, &args, "uninstall tunnel")?;
    if output.status.success() || !amnezia_tunnel_service_exists_impl(tunnel_name) {
        return Ok(());
    }
    if is_amnezia_access_denied_impl(&output) {
        let elevated = run_amneziawg_command_elevated_impl(binary, &args, "uninstall tunnel");
        match elevated {
            Ok(out) if out.status.success() || !amnezia_tunnel_service_exists_impl(tunnel_name) => {
                return Ok(());
            }
            Ok(out) => {
                if is_uac_elevation_cancelled_impl(&out) {
                    return Err(
                        "amneziawg tunnel uninstall requires administrator approval (UAC was cancelled)"
                            .to_string(),
                    );
                }
                let reason = describe_process_failure(&out, "amneziawg elevated uninstall");
                return Err(format!("amneziawg tunnel uninstall failed: {reason}"));
            }
            Err(error) => {
                return Err(format!(
                    "amneziawg tunnel uninstall requires administrator privileges: {error}"
                ));
            }
        }
    }
    let reason = describe_process_failure(&output, "amneziawg uninstall");
    Err(format!("amneziawg tunnel uninstall failed: {reason}"))
}

pub(super) fn start_amnezia_tunnel_service_impl(tunnel_name: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let service_name = format!("AmneziaWGTunnel${tunnel_name}");
        let output = hidden_command("sc.exe")
            .arg("start")
            .arg(&service_name)
            .output()
            .map_err(|e| format!("start amneziawg tunnel service failed: {e}"))?;
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .to_lowercase();
        if output.status.success()
            || text.contains("already running")
            || text.contains("service has already been started")
            || text.contains("service is already running")
        {
            return Ok(());
        }
        return Err(format!(
            "unable to start amneziawg tunnel service: {}",
            tail_lines(&text, 12)
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = tunnel_name;
        Err("amneziawg service start is only supported on Windows".to_string())
    }
}

pub(super) fn stop_amnezia_tunnel_service_impl(tunnel_name: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let service_name = format!("AmneziaWGTunnel${tunnel_name}");
        let output = hidden_command("sc.exe")
            .arg("stop")
            .arg(&service_name)
            .output()
            .map_err(|e| format!("stop amneziawg tunnel service failed: {e}"))?;
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .to_lowercase();
        if output.status.success()
            || text.contains("service has not been started")
            || text.contains("service is not started")
            || text.contains("service was stopped")
            || text.contains("service cannot accept control messages")
        {
            return Ok(());
        }
        return Err(format!(
            "unable to stop amneziawg tunnel service: {}",
            tail_lines(&text, 12)
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = tunnel_name;
        Err("amneziawg service stop is only supported on Windows".to_string())
    }
}

pub(super) fn delete_amnezia_tunnel_service_impl(tunnel_name: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let service_name = amnezia_service_name_impl(tunnel_name);
        let args = vec!["delete".to_string(), service_name.clone()];
        let output = hidden_command("sc")
            .arg("delete")
            .arg(&service_name)
            .output()
            .map_err(|e| format!("delete amneziawg tunnel service failed: {e}"))?;
        if output.status.success() {
            return Ok(());
        }
        if is_amnezia_access_denied_impl(&output) {
            let elevated = run_sc_command_elevated_impl(&args, "delete amneziawg service");
            match elevated {
                Ok(out) if out.status.success() => return Ok(()),
                Ok(out) => {
                    if is_uac_elevation_cancelled_impl(&out) {
                        return Err(
                            "amneziawg service deletion requires administrator approval (UAC was cancelled)"
                                .to_string(),
                        );
                    }
                    let reason = describe_process_failure(&out, "amneziawg elevated delete");
                    return Err(format!(
                        "unable to delete amneziawg tunnel service: {reason}"
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "amneziawg service deletion requires administrator privileges: {error}"
                    ));
                }
            }
        }
        let reason = describe_process_failure(&output, "amneziawg delete");
        Err(format!(
            "unable to delete amneziawg tunnel service: {reason}"
        ))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = tunnel_name;
        Err("amneziawg service deletion is only supported on Windows".to_string())
    }
}

#[cfg(target_os = "windows")]
pub(super) fn run_sc_command_elevated_impl(args: &[String], action: &str) -> Result<Output, String> {
    let arg_list = args
        .iter()
        .map(|value| format!("'{}'", escape_powershell_single_quoted_impl(value)))
        .collect::<Vec<_>>()
        .join(", ");
    let script = format!(
        "$p = Start-Process -FilePath 'sc.exe' -ArgumentList @({arg_list}) -Verb RunAs -WindowStyle Hidden -PassThru -Wait; exit $p.ExitCode"
    );
    let mut command = Command::new("powershell.exe");
    command
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script);
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .output()
        .map_err(|e| format!("spawn elevated sc.exe {action} failed: {e}"))
}

pub(super) fn run_amneziawg_command_impl(
    binary: &PathBuf,
    args: &[String],
    action: &str,
) -> Result<Output, String> {
    let mut command = Command::new(binary);
    for arg in args {
        command.arg(arg);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .output()
        .map_err(|e| format!("spawn amneziawg {action} failed: {e}"))
}

#[cfg(target_os = "windows")]
pub(super) fn run_amneziawg_command_elevated_impl(
    binary: &PathBuf,
    args: &[String],
    action: &str,
) -> Result<Output, String> {
    let file = escape_powershell_single_quoted_impl(&binary.to_string_lossy());
    let arg_list = args
        .iter()
        .map(|value| format!("'{}'", escape_powershell_single_quoted_impl(value)))
        .collect::<Vec<_>>()
        .join(", ");
    let script = format!(
        "$p = Start-Process -FilePath '{file}' -ArgumentList @({arg_list}) -Verb RunAs -WindowStyle Hidden -PassThru -Wait; exit $p.ExitCode"
    );
    let mut command = Command::new("powershell.exe");
    command
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script);
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .output()
        .map_err(|e| format!("spawn elevated amneziawg {action} failed: {e}"))
}

#[cfg(not(target_os = "windows"))]
pub(super) fn run_amneziawg_command_elevated_impl(
    _binary: &PathBuf,
    _args: &[String],
    _action: &str,
) -> Result<Output, String> {
    Err("amneziawg elevation is only supported on Windows".to_string())
}

#[cfg(target_os = "windows")]
pub(super) fn escape_powershell_single_quoted_impl(value: &str) -> String {
    value.replace('\'', "''")
}

pub(super) fn is_amnezia_access_denied_impl(output: &Output) -> bool {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = text.to_lowercase();
    lower.contains("access is denied") || lower.contains("error 5") || lower.contains("os error 5")
}

pub(super) fn is_uac_elevation_cancelled_impl(output: &Output) -> bool {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = text.to_lowercase();
    lower.contains("operation was canceled by the user")
        || lower.contains("the operation was canceled")
}

#[cfg(target_os = "windows")]
pub(super) fn amnezia_service_name_impl(tunnel_name: &str) -> String {
    format!("AmneziaWGTunnel${tunnel_name}")
}

pub(super) fn query_amnezia_tunnel_service_impl(tunnel_name: &str) -> AmneziaServiceSnapshot {
    #[cfg(target_os = "windows")]
    {
        let service_name = amnezia_service_name_impl(tunnel_name);
        let output = hidden_command("sc.exe")
            .arg("query")
            .arg(&service_name)
            .output();
        let Ok(output) = output else {
            return AmneziaServiceSnapshot {
                exists: false,
                state_code: None,
                raw_output: String::new(),
            };
        };
        let raw_output = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .trim()
        .to_string();
        if !output.status.success() {
            return AmneziaServiceSnapshot {
                exists: false,
                state_code: None,
                raw_output,
            };
        }
        let state_code = parse_sc_state_code(&raw_output);
        return AmneziaServiceSnapshot {
            exists: true,
            state_code,
            raw_output,
        };
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = tunnel_name;
        AmneziaServiceSnapshot {
            exists: false,
            state_code: None,
            raw_output: String::new(),
        }
    }
}

pub(super) fn amnezia_tunnel_service_exists_impl(tunnel_name: &str) -> bool {
    query_amnezia_tunnel_service_impl(tunnel_name).exists
}

pub(super) fn describe_amnezia_tunnel_status_impl(tunnel_name: &str) -> String {
    let snapshot = query_amnezia_tunnel_service_impl(tunnel_name);
    if !snapshot.exists {
        return "service is not installed".to_string();
    }
    let state = snapshot
        .state_code
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let details = tail_lines(&snapshot.raw_output, 12);
    if details.is_empty() {
        format!("service state code: {state}")
    } else {
        format!("service state code: {state}; details: {details}")
    }
}

pub(super) fn wait_amnezia_tunnel_state_impl(
    tunnel_name: &str,
    should_be_active: bool,
    timeout_ms: u64,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    loop {
        let snapshot = query_amnezia_tunnel_service_impl(tunnel_name);
        let active = snapshot.state_code == Some(4);
        if active == should_be_active {
            return Ok(());
        }
        if should_be_active && snapshot.exists && snapshot.state_code == Some(1) {
            let details = tail_lines(&snapshot.raw_output, 12);
            if details.is_empty() {
                return Err(
                    "amneziawg tunnel service entered STOPPED state before RUNNING".to_string(),
                );
            }
            return Err(format!(
                "amneziawg tunnel service entered STOPPED state before RUNNING: {details}"
            ));
        }
        if started.elapsed() >= Duration::from_millis(timeout_ms.max(1)) {
            return Err(if should_be_active {
                format!("amneziawg tunnel service did not reach RUNNING within {timeout_ms} ms")
            } else {
                format!(
                    "amneziawg tunnel service did not stop/uninstall within {} ms",
                    timeout_ms
                )
            });
        }
        thread::sleep(Duration::from_millis(250));
    }
}

pub(super) fn is_amnezia_tunnel_active_impl(tunnel_name: &str) -> bool {
    query_amnezia_tunnel_service_impl(tunnel_name).state_code == Some(4)
}
