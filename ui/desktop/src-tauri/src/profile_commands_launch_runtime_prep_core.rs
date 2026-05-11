use super::*;

pub(crate) fn prepare_librewolf_profile_runtime_impl(
    profile_dir: &Path,
    default_start_page: Option<&str>,
    default_search_provider: Option<&str>,
    gateway_proxy_port: Option<u16>,
    runtime_hardening: bool,
    identity_preset: Option<&IdentityPreset>,
) -> Result<(), std::io::Error> {
    fs::create_dir_all(profile_dir)?;
    let cleared_lock_files = support::clear_stale_librewolf_lock_files_impl(profile_dir)?;
    let cleared_search_state =
        support::clear_stale_librewolf_search_state_impl(profile_dir, default_search_provider)?;
    let startup_page = support::normalize_optional_start_page_url_impl(default_start_page)
        .map(|value| support::escape_firefox_pref_string_impl(&value));
    let has_restore_session =
        support::librewolf_profile_has_session_restore_preference_impl(profile_dir);
    eprintln!(
        "[profile-launch] librewolf runtime prefs dir={} startup_page={:?} has_restore_session={} default_search_provider={:?} gateway_proxy_port={:?} runtime_hardening={} cleared_lock_files={:?}",
        profile_dir.display(),
        startup_page,
        has_restore_session,
        default_search_provider,
        gateway_proxy_port,
        runtime_hardening,
        cleared_lock_files
    );

    let startup_cache = profile_dir.join("startupCache");
    if startup_cache.exists() {
        let _ = fs::remove_dir_all(startup_cache);
    }
    if !cleared_search_state.is_empty() {
        eprintln!(
            "[profile-launch] librewolf cleared search state dir={} files={:?}",
            profile_dir.display(),
            cleared_search_state
        );
    }

    let mut user_js_lines = vec![
        "user_pref(\"browser.tabs.hideSingleTab\", false);".to_string(),
        "user_pref(\"browser.tabs.inTitlebar\", 1);".to_string(),
        "user_pref(\"browser.tabs.drawInTitlebar\", true);".to_string(),
        "user_pref(\"browser.tabs.closeWindowWithLastTab\", false);".to_string(),
        "user_pref(\"browser.startup.homepage_override.mstone\", \"ignore\");".to_string(),
        "user_pref(\"startup.homepage_welcome_url\", \"\");".to_string(),
        "user_pref(\"startup.homepage_welcome_url.additional\", \"\");".to_string(),
        "user_pref(\"startup.homepage_override_url\", \"\");".to_string(),
        "user_pref(\"browser.search.suggest.enabled\", false);".to_string(),
        "user_pref(\"browser.search.geoSpecificDefaults\", false);".to_string(),
        "user_pref(\"browser.search.geoSpecificDefaults.url\", \"\");".to_string(),
        "user_pref(\"browser.search.region\", \"US\");".to_string(),
        "user_pref(\"browser.urlbar.suggest.searches\", false);".to_string(),
        "user_pref(\"browser.shell.checkDefaultBrowser\", false);".to_string(),
        "user_pref(\"accessibility.browsewithcaret\", false);".to_string(),
        "user_pref(\"layout.accessiblecaret.enabled\", false);".to_string(),
        "user_pref(\"layout.accessiblecaret.hide_carets_for_mouse_input\", true);".to_string(),
        "user_pref(\"layout.accessiblecaret.allow_script_change_updates\", false);".to_string(),
        "user_pref(\"layout.accessiblecaret.use_long_tap_injector\", false);".to_string(),
        "user_pref(\"devtools.responsive.touchSimulation.enabled\", false);".to_string(),
        "user_pref(\"dom.w3c_touch_events.enabled\", 0);".to_string(),
        "user_pref(\"dom.w3c_touch_events.legacy_apis.enabled\", false);".to_string(),
        "user_pref(\"dom.w3c_pointer_events.dispatch_by_pointer_messages\", false);".to_string(),
        "user_pref(\"browser.ui.touch_activation.enabled\", false);".to_string(),
        "user_pref(\"apz.windows.use_direct_manipulation\", false);".to_string(),
        "user_pref(\"ui.osk.enabled\", false);".to_string(),
        "user_pref(\"userChrome.decoration.cursor\", false);".to_string(),
        "user_pref(\"humanize\", false);".to_string(),
        "user_pref(\"showcursor\", false);".to_string(),
        "user_pref(\"browser.search.newSearchConfigEnabled\", false);".to_string(),
        "user_pref(\"browser.newtabpage.enabled\", true);".to_string(),
    ];
    if has_restore_session {
        user_js_lines.push("user_pref(\"browser.startup.page\", 3);".to_string());
    } else {
        user_js_lines.push("user_pref(\"browser.startup.page\", 1);".to_string());
    }
    let homepage_requested = startup_page.is_some();
    let mut homepage_injected = false;
    if !has_restore_session {
        if let Some(homepage) = startup_page.as_ref() {
            user_js_lines.push(format!(
                "user_pref(\"browser.startup.homepage\", \"{}\");",
                homepage
            ));
            homepage_injected = true;
        }
    }
    if let Some(engine_name) = support::map_search_provider_to_firefox_engine_impl(default_search_provider) {
        user_js_lines.push("user_pref(\"browser.search.separatePrivateDefault\", false);".to_string());
        user_js_lines.push(
            "user_pref(\"browser.search.separatePrivateDefault.ui.enabled\", false);".to_string(),
        );
        user_js_lines.push("user_pref(\"browser.search.update\", false);".to_string());
        user_js_lines.push(format!(
            "user_pref(\"browser.search.defaultenginename\", \"{}\");",
            engine_name
        ));
        user_js_lines.push(format!(
            "user_pref(\"browser.search.defaultenginename.private\", \"{}\");",
            engine_name
        ));
        user_js_lines.push(format!(
            "user_pref(\"browser.search.defaultEngine\", \"{}\");",
            engine_name
        ));
        user_js_lines.push(format!(
            "user_pref(\"browser.search.defaultEngineName\", \"{}\");",
            engine_name
        ));
        user_js_lines.push(format!(
            "user_pref(\"browser.search.selectedEngine\", \"{}\");",
            engine_name
        ));
        user_js_lines.push(format!(
            "user_pref(\"browser.search.order.1\", \"{}\");",
            engine_name
        ));
    }
    if runtime_hardening {
        user_js_lines.push("user_pref(\"signon.rememberSignons\", false);".to_string());
        user_js_lines.push("user_pref(\"signon.autofillForms\", false);".to_string());
        user_js_lines.push("user_pref(\"browser.formfill.enable\", false);".to_string());
        user_js_lines
            .push("user_pref(\"extensions.formautofill.addresses.enabled\", false);".to_string());
        user_js_lines
            .push("user_pref(\"extensions.formautofill.creditCards.enabled\", false);".to_string());
        user_js_lines.push("user_pref(\"browser.sessionstore.privacy_level\", 2);".to_string());
    }
    if let Some(port) = gateway_proxy_port {
        user_js_lines.push("user_pref(\"network.proxy.type\", 1);".to_string());
        user_js_lines.push("user_pref(\"network.proxy.share_proxy_settings\", true);".to_string());
        user_js_lines.push("user_pref(\"network.proxy.http\", \"127.0.0.1\");".to_string());
        user_js_lines.push(format!("user_pref(\"network.proxy.http_port\", {port});"));
        user_js_lines.push("user_pref(\"network.proxy.ssl\", \"127.0.0.1\");".to_string());
        user_js_lines.push(format!("user_pref(\"network.proxy.ssl_port\", {port});"));
        user_js_lines.push("user_pref(\"network.proxy.no_proxies_on\", \"\");".to_string());
    }
    support::apply_librewolf_identity_prefs_impl(&mut user_js_lines, identity_preset);
    let user_js = user_js_lines.join("\n");
    fs::write(profile_dir.join("user.js"), format!("{user_js}\n"))?;
    support::sanitize_librewolf_runtime_prefs_impl(profile_dir)?;
    support::normalize_librewolf_sessionstore_impl(profile_dir)?;
    if has_restore_session {
        support::prune_librewolf_restore_backups_impl(profile_dir)?;
    }

    let chrome_dir = profile_dir.join("chrome");
    fs::create_dir_all(&chrome_dir)?;
    let user_chrome = r#"
/* Force full Firefox-like chrome instead of compact/new-tab shell styles */
#TabsToolbar {
  visibility: visible !important;
  display: -moz-box !important;
  min-height: 34px !important;
}
#tabbrowser-tabs,
#tabbrowser-arrowscrollbox,
#titlebar {
  visibility: visible !important;
  display: -moz-box !important;
}
#nav-bar {
  visibility: visible !important;
}
"#;
    fs::write(chrome_dir.join("userChrome.css"), user_chrome.trim_start())?;

    eprintln!(
        "[profile-launch] librewolf profile prepared dir={} homepage_requested={} homepage_injected={} has_restore_session={} startup_page_via_launch_arg={}",
        profile_dir.display(),
        homepage_requested,
        homepage_injected,
        has_restore_session,
        false
    );
    Ok(())
}


#[path = "profile_commands_launch_runtime_prep_core_support.rs"]
mod support;


