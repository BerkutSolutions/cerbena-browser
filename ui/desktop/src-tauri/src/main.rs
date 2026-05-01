#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod device_posture;
mod envelope;
mod extensions_commands;
mod identity_commands;
mod launch_sessions;
mod launcher_commands;
mod network_commands;
mod network_runtime;
mod panic_frame;
mod process_tracking;
mod profile_commands;
mod profile_security;
mod route_runtime;
mod sensitive_store;
mod service_catalog_seed;
mod service_domains;
mod service_domains_data;
mod state;
mod sync_commands;
mod sync_snapshots;
mod traffic_commands;
mod traffic_gateway;
mod update_commands;
mod window_commands;

use state::AppState;
use tauri::{Manager, WindowEvent};

fn main() {
    let updater_launch_mode = update_commands::active_updater_launch_mode();
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
            if update_commands::active_updater_launch_mode().is_active() {
                return;
            }
            if let WindowEvent::CloseRequested { .. } = event {
                update_commands::launch_pending_update_on_exit(&window.app_handle());
                process_tracking::stop_all_profile_processes(&window.app_handle());
                route_runtime::stop_all_route_runtime(&window.app_handle());
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
            #[cfg(debug_assertions)]
            {
                let _ = main_window.eval("window.__BROWSER_DEV__ = true;");
                let _ = main_window.eval("console.log('[dev] injected debug flag');");
                println!("[dev] Cerbena debug mode enabled");
            }
            #[cfg(debug_assertions)]
            main_window.open_devtools();

            let state = AppState::bootstrap(app.handle())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            launch_sessions::prune_inactive_sessions(&state)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            if let Some(link_arg) = std::env::args()
                .skip(1)
                .find(|value| launcher_commands::detect_link_type(value).is_ok())
            {
                if let Ok(mut pending) = state.pending_external_link.lock() {
                    *pending = Some(link_arg);
                }
            }
            app.manage(state);
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
