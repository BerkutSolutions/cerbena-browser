use std::{
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

use browser_profile::{PatchProfileInput, ProfileState};
use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::certificate_runtime::clear_librewolf_profile_certificates;
use crate::launch_sessions::revoke_launch_session;
use crate::network_sandbox_lifecycle::stop_profile_network_stack;
use crate::panic_frame::close_panic_frame;
use crate::profile_runtime_logs::append_profile_log;
use crate::state::AppState;

pub fn track_profile_process(
    app_handle: AppHandle,
    profile_id: Uuid,
    pid: u32,
    profile_data_dir: PathBuf,
    ignored_pids: std::collections::BTreeSet<u32>,
) {
    const LOCK_STALE_GRACE_TICKS: u8 = 2;
    const HEADLESS_STALE_GRACE_TICKS: u8 = 5;
    const LOCKED_HEADLESS_STALE_GRACE_TICKS: u8 = 18;
    const ANY_HEADLESS_STALE_GRACE_TICKS: u8 = 45;
    const FIREFOX_NO_MAIN_WINDOW_GRACE_TICKS: u8 = 6;
    std::thread::spawn(move || {
        let mut tracked_pid = pid;
        let mut known_session_pids = vec![pid];
        let mut missing_profile_ticks = 0u8;
        let mut lock_only_ticks = 0u8;
        let mut headless_ticks = 0u8;
        let mut locked_headless_ticks = 0u8;
        let mut any_headless_ticks = 0u8;
        let mut firefox_no_main_window_ticks = 0u8;
        loop {
            let mapped_pid = current_tracked_profile_pid(&app_handle, profile_id);
            let Some(mapped_pid) = mapped_pid else {
                finalize_untracked_profile_state(&app_handle, profile_id, &profile_data_dir);
                eprintln!(
                    "[process-tracking] profile={} tracker exit: profile no longer tracked for pid={}",
                    profile_id, tracked_pid
                );
                break;
            };
            if mapped_pid != tracked_pid {
                tracked_pid = mapped_pid;
                if !known_session_pids.contains(&mapped_pid) {
                    known_session_pids.push(mapped_pid);
                }
            }
            if let Some(bootstrap_pid) = firefox_family_left_only_bootstrap_pid(
                &profile_data_dir,
                &known_session_pids,
            ) {
                eprintln!(
                    "[process-tracking] profile={} considered stopped because only bootstrap firefox-family pid remains pid={} known_session_pids={:?}",
                    profile_id,
                    bootstrap_pid,
                    known_session_pids
                );
                clear_profile_process(&app_handle, profile_id, tracked_pid, true);
                break;
            }
            if let Some(actual_pid) =
                find_profile_process_pid_preferred(
                    &profile_data_dir,
                    tracked_pid,
                    &known_session_pids,
                    &ignored_pids,
                )
            {
                let main_window_pid =
                    find_visible_profile_main_window_pid_for_dir(&profile_data_dir);
                if profile_has_firefox_family_processes(&profile_data_dir) {
                    if main_window_pid.is_none() {
                        firefox_no_main_window_ticks =
                            firefox_no_main_window_ticks.saturating_add(1);
                        if firefox_no_main_window_ticks >= FIREFOX_NO_MAIN_WINDOW_GRACE_TICKS {
                            eprintln!(
                                "[process-tracking] profile={} tracked_pid={} considered stopped after {} no-main-window ticks for firefox-family candidates={:?}",
                                profile_id,
                                tracked_pid,
                                firefox_no_main_window_ticks,
                                describe_profile_process_candidates(&profile_data_dir)
                            );
                            clear_profile_process(&app_handle, profile_id, tracked_pid, true);
                            break;
                        }
                    } else {
                        firefox_no_main_window_ticks = 0;
                    }
                } else {
                    firefox_no_main_window_ticks = 0;
                }
                missing_profile_ticks = 0;
                lock_only_ticks = 0;
                if !librewolf_profile_lock_present(&profile_data_dir)
                    && profile_process_candidates_have_no_visible_window(&profile_data_dir)
                {
                    headless_ticks = headless_ticks.saturating_add(1);
                    if headless_ticks >= HEADLESS_STALE_GRACE_TICKS {
                        eprintln!(
                            "[process-tracking] profile={} tracked_pid={} considered stopped after {} headless ticks lock_present=false candidates={:?}",
                            profile_id,
                            tracked_pid,
                            headless_ticks,
                            describe_profile_process_candidates(&profile_data_dir)
                        );
                        clear_profile_process(&app_handle, profile_id, tracked_pid, true);
                        break;
                    }
                } else {
                    headless_ticks = 0;
                }
                if profile_process_candidates_have_no_visible_window(&profile_data_dir) {
                    any_headless_ticks = any_headless_ticks.saturating_add(1);
                    if any_headless_ticks >= ANY_HEADLESS_STALE_GRACE_TICKS {
                        eprintln!(
                            "[process-tracking] profile={} tracked_pid={} considered stopped after {} any-headless ticks candidates={:?}",
                            profile_id,
                            tracked_pid,
                            any_headless_ticks,
                            describe_profile_process_candidates(&profile_data_dir)
                        );
                        clear_profile_process(&app_handle, profile_id, tracked_pid, true);
                        break;
                    }
                } else {
                    any_headless_ticks = 0;
                }
                if librewolf_profile_lock_present(&profile_data_dir)
                    && profile_process_candidates_have_no_visible_window(&profile_data_dir)
                {
                    locked_headless_ticks = locked_headless_ticks.saturating_add(1);
                    if locked_headless_ticks >= LOCKED_HEADLESS_STALE_GRACE_TICKS {
                        eprintln!(
                            "[process-tracking] profile={} tracked_pid={} considered stopped after {} locked-headless ticks candidates={:?}",
                            profile_id,
                            tracked_pid,
                            locked_headless_ticks,
                            describe_profile_process_candidates(&profile_data_dir)
                        );
                        clear_profile_process(&app_handle, profile_id, tracked_pid, true);
                        break;
                    }
                } else {
                    locked_headless_ticks = 0;
                }
                if actual_pid != tracked_pid {
                    if is_process_running(tracked_pid)
                        && !known_session_pids.contains(&actual_pid)
                        && !profile_process_is_child_of_pid(
                            &profile_data_dir,
                            actual_pid,
                            tracked_pid,
                        )
                    {
                        thread::sleep(Duration::from_millis(800));
                        continue;
                    }
                    replace_tracked_pid(&app_handle, profile_id, tracked_pid, actual_pid);
                    tracked_pid = actual_pid;
                    if !known_session_pids.contains(&actual_pid) {
                        known_session_pids.push(actual_pid);
                    }
                }
            } else {
                firefox_no_main_window_ticks = 0;
                let lock_present = librewolf_profile_lock_present(&profile_data_dir);
                let profile_candidates = find_profile_process_pids_for_dir(&profile_data_dir);
                let any_profile_process_running = !profile_candidates.is_empty();
                if lock_present && any_profile_process_running {
                    missing_profile_ticks = 0;
                    lock_only_ticks = 0;
                    headless_ticks = 0;
                    if profile_process_candidates_have_no_visible_window(&profile_data_dir) {
                        locked_headless_ticks = locked_headless_ticks.saturating_add(1);
                        any_headless_ticks = any_headless_ticks.saturating_add(1);
                        if locked_headless_ticks >= LOCKED_HEADLESS_STALE_GRACE_TICKS {
                            eprintln!(
                                "[process-tracking] profile={} tracked_pid={} considered stopped after {} locked-headless ticks (lock+running branch) candidates={:?}",
                                profile_id,
                                tracked_pid,
                                locked_headless_ticks,
                                describe_profile_process_candidates(&profile_data_dir)
                            );
                            clear_profile_process(&app_handle, profile_id, tracked_pid, true);
                            break;
                        }
                        if any_headless_ticks >= ANY_HEADLESS_STALE_GRACE_TICKS {
                            eprintln!(
                                "[process-tracking] profile={} tracked_pid={} considered stopped after {} any-headless ticks (lock+running branch) candidates={:?}",
                                profile_id,
                                tracked_pid,
                                any_headless_ticks,
                                describe_profile_process_candidates(&profile_data_dir)
                            );
                            clear_profile_process(&app_handle, profile_id, tracked_pid, true);
                            break;
                        }
                    } else {
                        locked_headless_ticks = 0;
                        any_headless_ticks = 0;
                    }
                    thread::sleep(Duration::from_millis(800));
                    continue;
                }
                if lock_present && !any_profile_process_running {
                    lock_only_ticks = lock_only_ticks.saturating_add(1);
                    // Keep missing_profile_ticks progressing after grace is reached.
                    // Firefox-family may leave a stale lock file after UI exit.
                    if lock_only_ticks < LOCK_STALE_GRACE_TICKS {
                        missing_profile_ticks = 0;
                    }
                    headless_ticks = 0;
                    locked_headless_ticks = 0;
                    any_headless_ticks = 0;
                    if lock_only_ticks < LOCK_STALE_GRACE_TICKS {
                        thread::sleep(Duration::from_millis(800));
                        continue;
                    }
                } else {
                    lock_only_ticks = 0;
                    if !lock_present {
                        locked_headless_ticks = 0;
                    }
                }
                missing_profile_ticks = missing_profile_ticks.saturating_add(1);
                if missing_profile_ticks >= 2
                    && (!is_process_running(tracked_pid) || !any_profile_process_running)
                {
                    eprintln!(
                        "[process-tracking] profile={} tracked_pid={} considered stopped after {} missing ticks lock_present={} any_profile_process_running={} candidates={:?}",
                        profile_id,
                        tracked_pid,
                        missing_profile_ticks,
                        librewolf_profile_lock_present(&profile_data_dir),
                        any_profile_process_running,
                        describe_profile_process_candidates(&profile_data_dir)
                    );
                    clear_profile_process(&app_handle, profile_id, tracked_pid, true);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(800));
        }
    });
}


#[path = "process_tracking_core_support.rs"]
mod support;
pub(crate) use support::*;


