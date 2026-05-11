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
    std::thread::spawn(move || {
        let mut tracked_pid = pid;
        let mut known_session_pids = vec![pid];
        let mut missing_profile_ticks = 0u8;
        let mut lock_only_ticks = 0u8;
        loop {
            if !is_profile_still_tracked(&app_handle, profile_id, tracked_pid) {
                finalize_untracked_profile_state(&app_handle, profile_id, &profile_data_dir);
                eprintln!(
                    "[process-tracking] profile={} tracker exit: profile no longer tracked for pid={}",
                    profile_id, tracked_pid
                );
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
                let candidates = describe_profile_process_candidates(&profile_data_dir);
                if candidates.len() > 1
                    && candidates.iter().any(|value| {
                        value.contains("librewolf")
                            || value.contains("firefox")
                            || value.contains("private_browsing")
                    })
                {
                    eprintln!(
                        "[process-tracking] profile={} candidate_set={:?}",
                        profile_id, candidates
                    );
                }
                missing_profile_ticks = 0;
                lock_only_ticks = 0;
                if actual_pid != tracked_pid {
                    if is_process_running(tracked_pid) && !known_session_pids.contains(&actual_pid) {
                        eprintln!(
                            "[process-tracking] profile={} ignore pid switch while tracked pid is alive old={} candidate={}",
                            profile_id, tracked_pid, actual_pid
                        );
                        thread::sleep(Duration::from_millis(800));
                        continue;
                    }
                    eprintln!(
                        "[process-tracking] profile={} tracked_pid_replaced old={} new={}",
                        profile_id, tracked_pid, actual_pid
                    );
                    replace_tracked_pid(&app_handle, profile_id, tracked_pid, actual_pid);
                    tracked_pid = actual_pid;
                    if !known_session_pids.contains(&actual_pid) {
                        known_session_pids.push(actual_pid);
                    }
                }
            } else {
                let lock_present = librewolf_profile_lock_present(&profile_data_dir);
                let profile_candidates = find_profile_process_pids_for_dir(&profile_data_dir);
                let any_profile_process_running = !profile_candidates.is_empty();
                if lock_present && any_profile_process_running {
                    missing_profile_ticks = 0;
                    lock_only_ticks = 0;
                    thread::sleep(Duration::from_millis(800));
                    continue;
                }
                if lock_present && !any_profile_process_running {
                    lock_only_ticks = lock_only_ticks.saturating_add(1);
                    missing_profile_ticks = 0;
                    if lock_only_ticks < LOCK_STALE_GRACE_TICKS {
                        thread::sleep(Duration::from_millis(800));
                        continue;
                    }
                } else {
                    lock_only_ticks = 0;
                }
                missing_profile_ticks = missing_profile_ticks.saturating_add(1);
                if !is_process_running(tracked_pid) && missing_profile_ticks >= 2 {
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


