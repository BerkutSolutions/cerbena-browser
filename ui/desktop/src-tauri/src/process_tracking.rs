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

use crate::profile_runtime_logs::append_profile_log;
use crate::launch_sessions::revoke_launch_session;
use crate::network_sandbox_lifecycle::stop_profile_network_stack;
use crate::panic_frame::close_panic_frame;
use crate::certificate_runtime::clear_librewolf_profile_certificates;
use crate::state::AppState;

pub fn track_profile_process(
    app_handle: AppHandle,
    profile_id: Uuid,
    pid: u32,
    profile_data_dir: PathBuf,
) {
    std::thread::spawn(move || {
        let mut tracked_pid = pid;
        let mut known_session_pids = vec![pid];
        let mut missing_profile_ticks = 0u8;
        loop {
            if let Some(actual_pid) = find_profile_process_pid(&profile_data_dir) {
                missing_profile_ticks = 0;
                if actual_pid != tracked_pid {
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
                if let Some(descendant_pid) = find_session_descendant_browser_pid(&known_session_pids)
                {
                    missing_profile_ticks = 0;
                    if descendant_pid != tracked_pid {
                        eprintln!(
                            "[process-tracking] profile={} descendant_pid_replaced old={} new={}",
                            profile_id, tracked_pid, descendant_pid
                        );
                        replace_tracked_pid(&app_handle, profile_id, tracked_pid, descendant_pid);
                        tracked_pid = descendant_pid;
                        if !known_session_pids.contains(&descendant_pid) {
                            known_session_pids.push(descendant_pid);
                        }
                    }
                } else {
                    missing_profile_ticks = missing_profile_ticks.saturating_add(1);
                }
                if !is_process_running(tracked_pid) && missing_profile_ticks >= 4 {
                    eprintln!(
                        "[process-tracking] profile={} tracked_pid={} considered stopped after {} missing ticks",
                        profile_id, tracked_pid, missing_profile_ticks
                    );
                    clear_profile_process(&app_handle, profile_id, tracked_pid, true);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(800));
        }
    });
}

fn replace_tracked_pid(app_handle: &AppHandle, profile_id: Uuid, old_pid: u32, new_pid: u32) {
    let state = app_handle.state::<AppState>();
    let mut launched = match state.launched_processes.lock() {
        Ok(value) => value,
        Err(_) => return,
    };
    let Some(current_pid) = launched.get(&profile_id).copied() else {
        return;
    };
    if current_pid != old_pid {
        return;
    }
    launched.insert(profile_id, new_pid);
}

pub fn clear_profile_process(app_handle: &AppHandle, profile_id: Uuid, pid: u32, emit_event: bool) {
    let state = app_handle.state::<AppState>();
    let mut launched = match state.launched_processes.lock() {
        Ok(value) => value,
        Err(_) => return,
    };

    let Some(current_pid) = launched.get(&profile_id).copied() else {
        return;
    };
    let effective_pid = if current_pid != pid {
        eprintln!(
            "[process-tracking] profile={} clear requested with stale pid={} current_pid={}; proceeding with current pid",
            profile_id, pid, current_pid
        );
        current_pid
    } else {
        pid
    };
    launched.remove(&profile_id);
    drop(launched);
    eprintln!(
        "[process-tracking] clearing profile={} pid={} emit_event={}",
        profile_id, effective_pid, emit_event
    );
    append_profile_log(
        app_handle,
        profile_id,
        "launcher",
        format!("Browser process stopped pid={effective_pid}"),
    );
    let _ = revoke_launch_session(state.inner(), profile_id, Some(effective_pid));
    stop_profile_network_stack(app_handle, profile_id);
    close_panic_frame(app_handle, profile_id);
    clear_librewolf_profile_certificates(app_handle, profile_id);

    if let Ok(manager) = state.manager.lock() {
        let _ = manager.update_profile(
            profile_id,
            PatchProfileInput {
                state: Some(ProfileState::Stopped),
                ..PatchProfileInput::default()
            },
        );
    }

    if emit_event {
        let _ = app_handle.emit(
            "profile-state-changed",
            serde_json::json!({
                "profileId": profile_id.to_string(),
                "state": "stopped"
            }),
        );
    }
}

pub fn stop_all_profile_processes(app_handle: &AppHandle) {
    let (sessions, profile_root) = {
        let state = app_handle.state::<AppState>();
        let launched = match state.launched_processes.lock() {
            Ok(value) => value,
            Err(_) => return,
        };
        (
            launched
                .iter()
                .map(|(profile_id, pid)| (*profile_id, *pid))
                .collect::<Vec<_>>(),
            state.profile_root.clone(),
        )
    };
    for (profile_id, pid) in sessions {
        terminate_process_tree(pid);
        clear_profile_process(app_handle, profile_id, pid, false);
    }
    if let Ok(entries) = std::fs::read_dir(&profile_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let profile_data_dir = path.join("engine-profile");
                terminate_profile_processes(&profile_data_dir);
            }
        }
    }
}

pub fn terminate_process_tree(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("taskkill");
        command.args(["/PID", &pid.to_string(), "/T", "/F"]);
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let _ = command.status();
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}

pub fn terminate_profile_processes(profile_data_dir: &Path) {
    let mut pids = find_profile_process_pids_for_dir(profile_data_dir);
    pids.sort_unstable();
    pids.dedup();
    for pid in pids {
        terminate_process_tree(pid);
    }
}

pub fn is_process_running(pid: u32) -> bool {
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    system
        .processes()
        .values()
        .any(|process| process.pid().as_u32() == pid)
}

fn find_profile_process_pid(profile_data_dir: &Path) -> Option<u32> {
    let target = profile_data_dir.to_string_lossy().to_lowercase();
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    matching_profile_processes(&system, &target)
        .into_iter()
        .next()
        .map(|candidate| candidate.pid)
}

pub fn find_profile_process_pid_for_dir(profile_data_dir: &Path) -> Option<u32> {
    find_profile_process_pid(profile_data_dir)
}

pub fn find_profile_main_window_pid_for_dir(profile_data_dir: &Path) -> Option<u32> {
    find_profile_process_pid(profile_data_dir)
}

pub fn find_profile_process_pids_for_dir(profile_data_dir: &Path) -> Vec<u32> {
    let target = profile_data_dir.to_string_lossy().to_lowercase();
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    matching_profile_processes(&system, &target)
        .into_iter()
        .map(|candidate| candidate.pid)
        .collect()
}

#[derive(Debug, Clone)]
struct ProfileProcessCandidate {
    pid: u32,
    command_line_length: usize,
}

fn matching_profile_processes(system: &System, target: &str) -> Vec<ProfileProcessCandidate> {
    if target.trim().is_empty() {
        return Vec::new();
    }

    let mut matches = system
        .processes()
        .values()
        .filter_map(|process| {
            let name = process.name().to_lowercase();
            let cmdline = process
                .cmd()
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();

            let firefox_profile_match = (name.contains("librewolf")
                || name.contains("firefox")
                || name.contains("private_browsing"))
                && cmdline.contains("-profile")
                && cmdline.contains(target);
            let chromium_profile_match = (name.contains("chromium")
                || name.contains("chrome")
                || name.contains("chromium"))
                && (cmdline.contains("--user-data-dir=") || cmdline.contains("--user-data-dir "))
                && cmdline.contains(target);

            if firefox_profile_match || chromium_profile_match {
                Some(ProfileProcessCandidate {
                    pid: process.pid().as_u32(),
                    command_line_length: cmdline.len(),
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
        left.command_line_length
            .cmp(&right.command_line_length)
            .then_with(|| left.pid.cmp(&right.pid))
    });
    matches
}

fn find_session_descendant_browser_pid(roots: &[u32]) -> Option<u32> {
    if roots.is_empty() {
        return None;
    }
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    let root_set = roots.iter().copied().collect::<std::collections::BTreeSet<_>>();
    let mut descendants = system
        .processes()
        .values()
        .filter_map(|process| {
            let name = process.name().to_lowercase();
            let browser_name = name.contains("chromium")
                || name.contains("chrome")
                || name.contains("chromium")
                || name.contains("librewolf")
                || name.contains("firefox")
                || name.contains("private_browsing");
            if !browser_name {
                return None;
            }
            let pid = process.pid().as_u32();
            is_descendant_of_any(&system, pid, &root_set).then_some(pid)
        })
        .collect::<Vec<_>>();
    descendants.sort_unstable();
    descendants.into_iter().next()
}

fn is_descendant_of_any(
    system: &System,
    pid: u32,
    roots: &std::collections::BTreeSet<u32>,
) -> bool {
    if roots.contains(&pid) {
        return true;
    }
    let mut cursor = system
        .processes()
        .values()
        .find(|process| process.pid().as_u32() == pid)
        .and_then(|process| process.parent());
    while let Some(parent_pid) = cursor {
        let parent_u32 = parent_pid.as_u32();
        if roots.contains(&parent_u32) {
            return true;
        }
        cursor = system
            .processes()
            .get(&parent_pid)
            .and_then(|process| process.parent());
    }
    false
}
