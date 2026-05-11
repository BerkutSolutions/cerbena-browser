use super::*;

pub(crate) fn launch_openvpn_runtime_impl(
    node: &NormalizedNode,
    runtime_dir: &PathBuf,
    profile_id: Uuid,
    binary: &str,
) -> Result<OpenVpnLaunch, String> {
    let config_path = runtime_dir.join("openvpn-route.ovpn");
    let log_path = runtime_dir.join("openvpn-route.log");
    let auth_path = build_openvpn_auth_file(node, runtime_dir, profile_id)?;
    let config_text = build_openvpn_config_text(node, auth_path.as_ref(), &log_path)?;
    fs::write(&config_path, config_text).map_err(|e| format!("write openvpn config: {e}"))?;
    let _ = fs::remove_file(&log_path);

    let mut command = hidden_command(binary);
    command
        .arg("--config")
        .arg(&config_path)
        .arg("--verb")
        .arg("3")
        .arg("--log")
        .arg(&log_path)
        .arg("--suppress-timestamps");
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn openvpn runtime failed: {e}"))?;
    let pid = child.id();
    if pid == 0 {
        return Err("openvpn runtime spawn failed: empty pid".to_string());
    }
    thread::sleep(Duration::from_millis(400));
    if let Some(status) = child
        .try_wait()
        .map_err(|e| format!("openvpn runtime process status check failed: {e}"))?
    {
        let code = status
            .code()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "terminated".to_string());
        let log = fs::read_to_string(&log_path).unwrap_or_default();
        let summary = tail_lines(&log, 16);
        return Err(format!(
            "openvpn runtime exited immediately (code {code}){}",
            if summary.is_empty() {
                String::new()
            } else {
                format!(": {summary}")
            }
        ));
    }
    wait_openvpn_connected(pid, &log_path, 20_000)?;

    let mut cleanup_paths = vec![log_path];
    if let Some(path) = auth_path {
        cleanup_paths.push(path);
    }
    Ok(OpenVpnLaunch {
        pid,
        config_path,
        cleanup_paths,
    })
}

pub(crate) fn build_openvpn_auth_file_impl(
    node: &NormalizedNode,
    runtime_dir: &PathBuf,
    profile_id: Uuid,
) -> Result<Option<PathBuf>, String> {
    let username = node.username.as_deref().unwrap_or_default().trim();
    let password = node.password.as_deref().unwrap_or_default().trim();
    if username.is_empty() && password.is_empty() {
        return Ok(None);
    }
    if username.is_empty() {
        return Err("openvpn username is required when password is set".to_string());
    }
    let path = runtime_dir.join(format!("openvpn-auth-{}.txt", profile_id.as_simple()));
    fs::write(&path, format!("{username}\n{password}\n"))
        .map_err(|e| format!("write openvpn auth file: {e}"))?;
    Ok(Some(path))
}

pub(crate) fn build_openvpn_config_text_impl(
    node: &NormalizedNode,
    auth_path: Option<&PathBuf>,
    log_path: &PathBuf,
) -> Result<String, String> {
    let host = node
        .host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "openvpn host is required".to_string())?;
    let port = node
        .port
        .filter(|value| *value > 0)
        .ok_or_else(|| "openvpn port is required".to_string())?;
    let transport = node
        .settings
        .get("transport")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| value.eq_ignore_ascii_case("udp") || value.eq_ignore_ascii_case("tcp"))
        .unwrap_or("udp")
        .to_ascii_lowercase();

    if let Some(raw) = node
        .settings
        .get("ovpnRaw")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let has_auth_user_pass = openvpn_config_has_directive(raw, "auth-user-pass");
        if auth_path.is_none()
            && has_auth_user_pass
            && !raw.to_ascii_lowercase().contains("<auth-user-pass>")
        {
            return Err(
                "openvpn profile requests auth-user-pass; set username/password fields".to_string(),
            );
        }
        let mut out = raw.replace('\r', "");
        if let Some(path) = auth_path {
            let auth_line = format!(
                "auth-user-pass \"{}\"",
                path.to_string_lossy().replace('\\', "\\\\")
            );
            if has_auth_user_pass {
                out = rewrite_openvpn_auth_user_pass(&out, &auth_line);
            } else {
                append_openvpn_directive_if_missing(&mut out, &auth_line, "auth-user-pass");
            }
            append_openvpn_directive_if_missing(&mut out, "auth-retry nointeract", "auth-retry");
        }
        append_openvpn_directive_if_missing(&mut out, "script-security 2", "script-security");
        append_openvpn_directive_if_missing(&mut out, "up /usr/local/bin/openvpn-dns-sync", "up");
        append_openvpn_directive_if_missing(
            &mut out,
            "down /usr/local/bin/openvpn-dns-sync",
            "down",
        );
        append_openvpn_directive_if_missing(
            &mut out,
            &format!(
                "log \"{}\"",
                log_path.to_string_lossy().replace('\\', "\\\\")
            ),
            "log",
        );
        append_openvpn_directive_if_missing(&mut out, "verb 3", "verb");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        return Ok(out);
    }

    let mut lines = vec![
        "client".to_string(),
        "dev tun".to_string(),
        format!("proto {transport}"),
        format!("remote {host} {port}"),
        "resolv-retry infinite".to_string(),
        "nobind".to_string(),
        "persist-key".to_string(),
        "persist-tun".to_string(),
        "auth-retry nointeract".to_string(),
        "script-security 2".to_string(),
        "up /usr/local/bin/openvpn-dns-sync".to_string(),
        "down /usr/local/bin/openvpn-dns-sync".to_string(),
        "remote-cert-tls server".to_string(),
        "verb 3".to_string(),
    ];

    if let Some(path) = auth_path {
        lines.push(format!(
            "auth-user-pass \"{}\"",
            path.to_string_lossy().replace('\\', "\\\\")
        ));
    }

    for (setting_key, inline_tag) in [
        ("caCert", "ca"),
        ("clientCert", "cert"),
        ("clientKey", "key"),
    ] {
        if let Some(value) = node
            .settings
            .get(setting_key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            lines.push(format!("<{inline_tag}>"));
            lines.extend(value.replace('\r', "").lines().map(|line| line.to_string()));
            lines.push(format!("</{inline_tag}>"));
        }
    }

    lines.push(format!(
        "log \"{}\"",
        log_path.to_string_lossy().replace('\\', "\\\\")
    ));
    Ok(lines.join("\n") + "\n")
}

fn append_openvpn_directive_if_missing(config: &mut String, directive: &str, key: &str) {
    if openvpn_config_has_directive(config, key) {
        return;
    }
    if !config.ends_with('\n') {
        config.push('\n');
    }
    config.push_str(directive);
    config.push('\n');
}

fn openvpn_config_has_directive(config: &str, key: &str) -> bool {
    config.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            return false;
        }
        let lower = trimmed.to_ascii_lowercase();
        lower == key || lower.starts_with(&format!("{key} "))
    })
}

fn rewrite_openvpn_auth_user_pass(config: &str, directive: &str) -> String {
    let mut replaced = false;
    let mut lines = Vec::new();
    for line in config.lines() {
        let trimmed = line.trim();
        let is_directive =
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                false
            } else {
                let lower = trimmed.to_ascii_lowercase();
                lower == "auth-user-pass" || lower.starts_with("auth-user-pass ")
            };
        if is_directive {
            if !replaced {
                lines.push(directive.to_string());
                replaced = true;
            }
            continue;
        }
        lines.push(line.to_string());
    }
    if !replaced {
        lines.push(directive.to_string());
    }
    lines.join("\n")
}

fn wait_openvpn_connected(pid: u32, log_path: &PathBuf, timeout_ms: u64) -> Result<(), String> {
    let started = std::time::Instant::now();
    loop {
        if !is_process_running(pid) {
            let summary = fs::read_to_string(log_path)
                .ok()
                .map(|text| tail_lines(&text, 20))
                .unwrap_or_default();
            return Err(format!(
                "openvpn runtime terminated before connect{}",
                if summary.is_empty() {
                    String::new()
                } else {
                    format!(": {summary}")
                }
            ));
        }
        let log = fs::read_to_string(log_path).unwrap_or_default();
        if log.contains("Initialization Sequence Completed") {
            return Ok(());
        }
        if log.contains("AUTH_FAILED")
            || log.contains("Options error")
            || log.contains("TLS Error")
            || log.contains("Exiting due to fatal error")
        {
            let summary = tail_lines(&log, 20);
            return Err(format!("openvpn runtime failed to initialize: {summary}"));
        }
        if started.elapsed() >= Duration::from_millis(timeout_ms.max(1)) {
            let summary = tail_lines(&log, 20);
            return Err(format!(
                "openvpn runtime did not reach CONNECTED within {} ms{}",
                timeout_ms,
                if summary.is_empty() {
                    String::new()
                } else {
                    format!(": {summary}")
                }
            ));
        }
        thread::sleep(Duration::from_millis(250));
    }
}
