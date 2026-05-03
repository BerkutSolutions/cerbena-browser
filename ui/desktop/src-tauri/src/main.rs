#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod device_posture;
mod certificate_runtime;
mod envelope;
mod extensions_commands;
mod identity_commands;
mod install_registration;
mod instance_handoff;
mod keepassxc_bridge;
mod launch_sessions;
mod launcher_commands;
mod network_commands;
mod network_runtime;
mod network_sandbox;
mod network_sandbox_adapter;
mod network_sandbox_container;
mod network_sandbox_container_runtime;
mod network_sandbox_lifecycle;
mod panic_frame;
mod process_tracking;
mod profile_commands;
mod profile_runtime_logs;
mod profile_security;
mod route_runtime;
mod sensitive_store;
mod service_catalog_seed;
mod service_domains;
mod service_domains_data;
mod shell_commands;
mod state;
mod sync_commands;
mod sync_snapshots;
mod traffic_commands;
mod traffic_gateway;
mod update_commands;
mod window_commands;

use state::AppState;
use tauri::{Manager, WindowEvent};

fn updater_relaunch_auto_exit_after() -> Option<std::time::Duration> {
    let raw = std::env::var(update_commands::UPDATER_RELAUNCH_AUTO_EXIT_ENV).ok()?;
    let seconds = raw.trim().parse::<u64>().ok()?;
    if seconds == 0 {
        return None;
    }
    Some(std::time::Duration::from_secs(seconds))
}

fn startup_external_link_arg(
    updater_launch_mode: update_commands::UpdaterLaunchMode,
) -> Option<String> {
    if updater_launch_mode.is_active() {
        return None;
    }
    std::env::args().skip(1).find(|value| {
        !value.trim_start().starts_with("--") && launcher_commands::detect_link_type(value).is_ok()
    })
}

fn handle_selftest_version_probe(updater_launch_mode: update_commands::UpdaterLaunchMode) -> bool {
    if updater_launch_mode.is_active() {
        return false;
    }
    let Ok(path) = std::env::var("CERBENA_SELFTEST_REPORT_VERSION_FILE") else {
        return false;
    };
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    let target = std::path::PathBuf::from(trimmed);
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&target, env!("CARGO_PKG_VERSION")).is_ok()
}

fn main() {
    let updater_launch_mode = update_commands::active_updater_launch_mode();
    if handle_selftest_version_probe(updater_launch_mode) {
        return;
    }
    #[cfg(target_os = "windows")]
    let app_data_root = std::env::var_os("LOCALAPPDATA").map(|local_app_data| {
        std::path::PathBuf::from(local_app_data).join(if cfg!(debug_assertions) {
            "dev.browser.launcher"
        } else {
            "Cerbena Browser"
        })
    });
    let startup_link_arg = startup_external_link_arg(updater_launch_mode);

    #[cfg(target_os = "windows")]
    if !updater_launch_mode.is_active() {
        if let Some(app_data_root) = app_data_root.as_deref() {
            if !instance_handoff::acquire_single_instance_guard(app_data_root).unwrap_or(true) {
                if let Some(link_arg) = startup_link_arg.as_deref() {
                    let _ = instance_handoff::forward_link_to_primary_data_root(
                        app_data_root,
                        link_arg,
                    );
                } else {
                    let _ = instance_handoff::signal_primary_activation_data_root(app_data_root);
                }
                return;
            }

            if let Some(link_arg) = startup_link_arg.as_deref() {
                if instance_handoff::forward_link_to_primary_data_root(app_data_root, link_arg)
                    .unwrap_or(false)
                {
                    return;
                }
            }
        }
    }

    tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label().starts_with("panic-frame-menu-") {
                if let WindowEvent::Focused(false) = event {
                    let _ = window.hide();
                }
                return;
            }
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                if update_commands::active_updater_launch_mode().is_active() {
                    update_commands::launch_pending_update_on_exit(&window.app_handle());
                    return;
                }
                match shell_commands::resolve_close_request(&window.app_handle()) {
                    Ok(shell_commands::CloseRequestAction::AllowExit) => {
                        window_commands::perform_shutdown_cleanup(&window.app_handle());
                    }
                    Ok(shell_commands::CloseRequestAction::HideToTray) => {
                        api.prevent_close();
                        let _ = shell_commands::hide_main_window(&window.app_handle());
                    }
                    Ok(shell_commands::CloseRequestAction::PromptToEnableTray) => {
                        api.prevent_close();
                        shell_commands::emit_close_to_tray_prompt(&window.app_handle());
                    }
                    Err(_) => {
                        window_commands::perform_shutdown_cleanup(&window.app_handle());
                    }
                }
            }
        })
        .on_page_load(|window, payload| {
            #[cfg(debug_assertions)]
            println!(
                "[dev][page-load] window={} url={}",
                window.label(),
                payload.url()
            );
            #[cfg(not(debug_assertions))]
            {
                let _ = (&window, &payload);
            }
        })
        .setup(move |app| {
            let main_window = app
                .get_webview_window("main")
                .expect("main window should exist");
            if updater_launch_mode.is_active() {
                update_commands::configure_window_for_launch_mode(
                    &main_window,
                    updater_launch_mode,
                )
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            } else {
                main_window.set_title("Cerbena")?;
            }
            if let Some(delay) = updater_relaunch_auto_exit_after() {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(delay);
                    let _ = shell_commands::request_exit(&app_handle);
                });
            }
            #[cfg(debug_assertions)]
            {
                let _ = main_window.eval("window.__BROWSER_DEV__ = true;");
                let _ = main_window.eval("console.log('[dev] injected debug flag');");
                println!("[dev] Cerbena debug mode enabled");
            }
            #[cfg(debug_assertions)]
            main_window.open_devtools();

            if let Some(link_arg) = startup_link_arg.as_deref() {
                if instance_handoff::forward_link_to_primary(app.handle(), link_arg)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                {
                    let _ = main_window.hide();
                    std::process::exit(0);
                }
            }

            let state = AppState::bootstrap(app.handle())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            launch_sessions::prune_inactive_sessions(&state)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            if let Some(link_arg) = startup_link_arg {
                if let Ok(mut pending) = state.pending_external_link.lock() {
                    *pending = Some(link_arg);
                }
            }
            app.manage(state);
            install_registration::reconcile_install_registration(app.handle());
            shell_commands::setup_system_tray(app)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            if !updater_launch_mode.is_active() {
                instance_handoff::setup_primary_instance_bridge(app)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            }
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    window_commands::emit_app_lifecycle_progress(
                        &app_handle,
                        "startup",
                        "janitor",
                        "app.lifecycle.startup.janitor",
                        false,
                    );
                    network_sandbox_lifecycle::cleanup_network_sandbox_janitor(&app_handle);
                    window_commands::emit_app_lifecycle_progress(
                        &app_handle,
                        "startup",
                        "janitor",
                        "app.lifecycle.startup.ready",
                        true,
                    );
                });
            }
            if updater_launch_mode.is_active() {
                let state = app.state::<AppState>();
                let _ = update_commands::ensure_updater_flow_started(&state);
            } else {
                update_commands::start_update_scheduler(app.handle().clone());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            profile_commands::list_profiles,
            profile_commands::create_profile,
            profile_commands::update_profile,
            profile_commands::delete_profile,
            profile_commands::duplicate_profile,
            profile_commands::launch_profile,
            profile_commands::stop_profile,
            profile_commands::acknowledge_wayfern_tos,
            profile_commands::get_wayfern_terms_status,
            profile_commands::read_profile_logs,
            profile_commands::ensure_engine_binaries,
            profile_commands::copy_profile_cookies,
            profile_commands::set_profile_password,
            profile_commands::unlock_profile,
            profile_commands::validate_profile_modal,
            profile_commands::pick_certificate_files,
            profile_commands::cancel_engine_download,
            profile_commands::export_profile,
            profile_commands::import_profile,
            identity_commands::generate_identity_auto_preset,
            identity_commands::validate_identity_preset_command,
            identity_commands::preview_identity_preset,
            identity_commands::validate_identity_save,
            identity_commands::save_identity_profile,
            identity_commands::get_identity_profile,
            identity_commands::apply_identity_auto_geolocation,
            network_commands::save_vpn_proxy_policy,
            network_commands::test_vpn_proxy_policy,
            network_commands::get_network_state,
            network_commands::save_connection_template,
            network_commands::delete_connection_template,
            network_commands::ping_connection_template,
            network_commands::test_connection_template_request,
            network_commands::save_global_route_settings,
            network_commands::save_dns_policy,
            network_sandbox::save_network_sandbox_profile_settings,
            network_sandbox::save_network_sandbox_global_settings,
            network_sandbox::preview_network_sandbox_settings,
            network_commands::get_service_catalog,
            network_commands::set_service_block_all,
            network_commands::set_service_allowed,
            network_commands::evaluate_network_policy_demo,
            traffic_commands::list_traffic_events,
            traffic_commands::set_traffic_rule,
            extensions_commands::list_extensions,
            extensions_commands::list_extension_library,
            extensions_commands::import_extension_library_item,
            extensions_commands::update_extension_library_item,
            extensions_commands::update_extension_library_preferences,
            extensions_commands::refresh_extension_library_updates,
            extensions_commands::export_extension_library,
            extensions_commands::import_extension_library,
            extensions_commands::set_extension_profiles,
            extensions_commands::remove_extension_library_item,
            extensions_commands::install_extension,
            extensions_commands::enable_extension,
            extensions_commands::disable_extension,
            extensions_commands::process_first_launch_extensions,
            extensions_commands::evaluate_extension_policy,
            sync_commands::save_sync_controls,
            sync_commands::get_sync_overview,
            sync_commands::add_sync_conflict,
            sync_commands::clear_sync_conflicts,
            sync_commands::create_backup_snapshot,
            sync_commands::restore_snapshot,
            sync_commands::sync_health_ping,
            launcher_commands::build_home_dashboard,
            launcher_commands::panic_wipe_profile,
            launcher_commands::set_default_profile_for_links,
            launcher_commands::clear_default_profile_for_links,
            launcher_commands::get_link_routing_overview,
            launcher_commands::save_link_type_profile_binding,
            launcher_commands::remove_link_type_profile_binding,
            launcher_commands::dispatch_external_link,
            launcher_commands::consume_pending_external_link,
            launcher_commands::execute_launch_hook,
            launcher_commands::resolve_pip_policy,
            launcher_commands::import_search_providers,
            launcher_commands::set_default_search_provider,
            launcher_commands::run_guardrail_check,
            launcher_commands::append_runtime_log,
            launcher_commands::read_runtime_logs,
            launcher_commands::get_global_security_settings,
            launcher_commands::save_global_security_settings,
            launcher_commands::get_device_posture_report,
            launcher_commands::refresh_device_posture_report,
            shell_commands::get_shell_preferences_state,
            shell_commands::save_shell_preferences,
            shell_commands::window_hide_to_tray,
            shell_commands::window_restore_from_tray,
            shell_commands::confirm_app_exit,
            shell_commands::open_default_apps_settings,
            update_commands::get_launcher_update_state,
            update_commands::set_launcher_auto_update,
            update_commands::check_launcher_updates,
            update_commands::get_updater_overview,
            update_commands::start_updater_flow,
            update_commands::launch_updater_preview,
            panic_frame::panic_frame_show_menu,
            panic_frame::panic_frame_hide_menu,
            window_commands::window_minimize,
            window_commands::window_toggle_maximize,
            window_commands::window_close
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
