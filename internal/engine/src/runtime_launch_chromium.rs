use super::*;

pub(super) fn launch_args_chromium_family_impl(
    profile_root: &Path,
    start_page: Option<&str>,
    private_mode: bool,
    gateway_proxy_port: Option<u16>,
    runtime_hardening: bool,
) -> Result<Vec<String>, EngineError> {
    let runtime_dir = profile_root.join("engine-profile");
    let locked_app = load_locked_app_config(profile_root)?;
    let identity_policy = load_identity_launch_policy(profile_root);
    const MAX_HOST_RESOLVER_RULES_LEN: usize = 8_192;
    let mut args = vec![
        format!("--user-data-dir={}", runtime_dir.to_string_lossy()),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-background-mode".to_string(),
        "--disable-quic".to_string(),
        "--disable-features=AsyncDns,DnsHttpssvc".to_string(),
    ];
    if private_mode {
        args.push("--incognito".to_string());
    }
    if runtime_hardening {
        args.push("--disable-sync".to_string());
        args.push("--disable-save-password-bubble".to_string());
    }
    if let Some(port) = gateway_proxy_port {
        args.push(format!("--proxy-server=http://127.0.0.1:{port}"));
        args.push("--proxy-bypass-list=".to_string());
    }
    if gateway_proxy_port.is_none() {
        if let Some(host_rules) =
            chromium_host_resolver_rules(profile_root, MAX_HOST_RESOLVER_RULES_LEN)
        {
            args.push(format!("--host-resolver-rules={host_rules}"));
        }
    }
    apply_chromium_identity_args_impl(profile_root, identity_policy.as_ref(), &mut args)?;
    let extension_dirs = prepare_chromium_extension_dirs(profile_root)?;
    if !extension_dirs.is_empty() {
        let joined = extension_dirs
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(",");
        args.push(format!("--load-extension={joined}"));
    }
    if let Some(config) = locked_app {
        let app_url = resolve_locked_app_target_url(&config, start_page.unwrap_or(""));
        args.push(format!("--app={app_url}"));
    } else if let Some(page) = start_page.map(str::trim).filter(|value| !value.is_empty()) {
        args.push(page.to_string());
    }
    Ok(args)
}

pub(super) fn apply_chromium_identity_args_impl(
    profile_root: &Path,
    identity: Option<&IdentityLaunchPolicy>,
    args: &mut Vec<String>,
) -> Result<(), EngineError> {
    let Some(identity) = identity else {
        return Ok(());
    };
    if !identity.core.user_agent.trim().is_empty() && !identity_uses_native_user_agent(identity) {
        args.push(format!("--user-agent={}", identity.core.user_agent.trim()));
    }
    if let Some(language) = normalize_primary_language(&identity.locale.navigator_language) {
        args.push(format!("--lang={language}"));
    }
    let window_width = first_positive(identity.window.outer_width, identity.screen.width);
    let window_height = first_positive(identity.window.outer_height, identity.screen.height);
    if window_width > 0 && window_height > 0 {
        args.push(format!("--window-size={window_width},{window_height}"));
    }
    if identity.window.screen_x != 0 || identity.window.screen_y != 0 {
        args.push(format!(
            "--window-position={},{}",
            identity.window.screen_x, identity.window.screen_y
        ));
    }
    let languages = normalize_accept_languages(
        &identity.locale.navigator_language,
        &identity.locale.languages,
    );
    if !languages.is_empty() {
        args.push(format!("--accept-lang={}", languages.join(",")));
        write_chromium_language_preferences(profile_root, &languages)?;
        write_chromium_local_state_locale(profile_root, &languages)?;
    }
    Ok(())
}

pub(super) fn chromium_launch_environment_impl(profile_root: &Path) -> Vec<(String, String)> {
    let Some(identity) = load_identity_launch_policy(profile_root) else {
        return Vec::new();
    };
    let languages = normalize_accept_languages(
        &identity.locale.navigator_language,
        &identity.locale.languages,
    );
    if languages.is_empty() {
        return Vec::new();
    }
    let primary = languages[0].clone();
    vec![
        ("LANG".to_string(), format!("{primary}.UTF-8")),
        ("LANGUAGE".to_string(), languages.join(":")),
        ("LC_ALL".to_string(), format!("{primary}.UTF-8")),
    ]
}

pub(super) fn reopen_args_chromium_family_impl(
    profile_root: &Path,
    runtime_dir: &Path,
    url: &str,
) -> Result<Vec<String>, EngineError> {
    let locked_app = load_locked_app_config(profile_root)?;
    let mut args = vec![format!("--user-data-dir={}", runtime_dir.to_string_lossy())];
    if let Some(config) = locked_app {
        args.push(format!(
            "--app={}",
            resolve_locked_app_target_url(&config, url)
        ));
    } else {
        args.push(url.trim().to_string());
    }
    Ok(args)
}
