use super::*;

pub(crate) fn finalize_untracked_profile_state(
    app_handle: &AppHandle,
    profile_id: Uuid,
    profile_data_dir: &Path,
) {
    // If tracking entry disappears before tracker observes a clean process exit,
    // force a stopped state once profile processes are really gone.
    if find_profile_process_pid(profile_data_dir).is_some() {
        return;
    }
    let state = app_handle.state::<AppState>();
    if let Ok(manager) = state.manager.lock() {
        let _ = manager.update_profile(
            profile_id,
            PatchProfileInput {
                state: Some(ProfileState::Stopped),
                ..PatchProfileInput::default()
            },
        );
    }
    let _ = app_handle.emit(
        "profile-state-changed",
        serde_json::json!({
            "profileId": profile_id.to_string(),
            "state": "stopped"
        }),
    );
}

pub(crate) fn is_profile_still_tracked(app_handle: &AppHandle, profile_id: Uuid, tracked_pid: u32) -> bool {
    let state = app_handle.state::<AppState>();
    let launched = match state.launched_processes.lock() {
        Ok(value) => value,
        Err(_) => return false,
    };
    matches!(launched.get(&profile_id).copied(), Some(current_pid) if current_pid == tracked_pid)
}

pub(crate) fn replace_tracked_pid(app_handle: &AppHandle, profile_id: Uuid, old_pid: u32, new_pid: u32) {
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

    // Always emit stopped once the profile is cleared to keep UI state in sync.
    let _ = app_handle.emit(
        "profile-state-changed",
        serde_json::json!({
            "profileId": profile_id.to_string(),
            "state": "stopped"
        }),
    );
    if !emit_event {
        eprintln!(
            "[process-tracking] profile={} stop emitted even with emit_event=false to preserve state sync",
            profile_id
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
        clear_profile_process(app_handle, profile_id, pid, true);
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

pub(crate) fn find_profile_process_pid(profile_data_dir: &Path) -> Option<u32> {
    let target = profile_data_dir.to_string_lossy().to_lowercase();
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    matching_profile_processes(&system, &target)
        .into_iter()
        .next()
        .map(|candidate| candidate.pid)
}

pub(crate) fn find_profile_process_pid_preferred(
    profile_data_dir: &Path,
    tracked_pid: u32,
    known_session_pids: &[u32],
    ignored_pids: &std::collections::BTreeSet<u32>,
) -> Option<u32> {
    let target = profile_data_dir.to_string_lossy().to_lowercase();
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    let candidates = matching_profile_processes(&system, &target)
        .into_iter()
        .filter(|candidate| candidate.pid == tracked_pid || !ignored_pids.contains(&candidate.pid))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }

    if candidates.iter().any(|candidate| candidate.pid == tracked_pid) {
        return Some(tracked_pid);
    }

    let known = known_session_pids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut known_candidates = candidates
        .iter()
        .filter(|candidate| known.contains(&candidate.pid))
        .map(|candidate| candidate.pid)
        .collect::<Vec<_>>();
    known_candidates.sort_unstable();
    if let Some(pid) = known_candidates.pop() {
        return Some(pid);
    }

    let mut newer_unknown = candidates
        .iter()
        .map(|candidate| candidate.pid)
        .filter(|pid| *pid > tracked_pid)
        .collect::<Vec<_>>();
    newer_unknown.sort_unstable();
    newer_unknown.pop()
}

pub fn find_profile_process_pid_for_dir(profile_data_dir: &Path) -> Option<u32> {
    find_profile_process_pid(profile_data_dir)
}

pub fn find_profile_main_window_pid_for_dir(profile_data_dir: &Path) -> Option<u32> {
    let target = profile_data_dir.to_string_lossy().to_lowercase();
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    let candidates = matching_profile_processes(&system, &target);
    find_main_window_pid_for_candidates(&candidates).or_else(|| find_profile_process_pid(profile_data_dir))
}

pub fn firefox_profile_lock_present_for_dir(profile_data_dir: &Path) -> bool {
    librewolf_profile_lock_present(profile_data_dir)
}

pub fn describe_profile_process_candidates(profile_data_dir: &Path) -> Vec<String> {
    let target = profile_data_dir.to_string_lossy().to_lowercase();
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    matching_profile_processes(&system, &target)
        .into_iter()
        .map(|candidate| {
            format!(
                "pid={} parent={:?} name={} started={} status={} exe={} has_window={} cmd={}",
                candidate.pid,
                candidate.parent_pid,
                candidate.name,
                candidate.start_time,
                candidate.status,
                candidate.exe_path,
                candidate.has_main_window,
                candidate.command_line
            )
        })
        .collect()
}

pub fn describe_firefox_family_processes() -> Vec<String> {
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    let mut matches = system
        .processes()
        .values()
        .filter_map(|process| {
            let name = process.name().to_lowercase();
            if !(name.contains("librewolf")
                || name.contains("firefox")
                || name.contains("private_browsing"))
            {
                return None;
            }
            let cmdline = process
                .cmd()
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            Some(format!(
                "pid={} parent={:?} name={} started={} status={:?} exe={:?} cmd={}",
                process.pid().as_u32(),
                process.parent().map(|pid| pid.as_u32()),
                name,
                process.start_time(),
                process.status(),
                process.exe(),
                cmdline
            ))
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches
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
pub(crate) struct ProfileProcessCandidate {
    pid: u32,
    parent_pid: Option<u32>,
    name: String,
    exe_path: String,
    status: String,
    start_time: u64,
    has_main_window: bool,
    command_line: String,
    command_line_length: usize,
}

pub(crate) fn matching_profile_processes(system: &System, target: &str) -> Vec<ProfileProcessCandidate> {
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
                && cmdline.contains(target)
                && !cmdline.contains("-backgroundtask")
                && !cmdline.contains("--backgroundtask");
            let chromium_profile_match = (name.contains("chromium")
                || name.contains("chrome")
                || name.contains("chromium"))
                && (cmdline.contains("--user-data-dir=") || cmdline.contains("--user-data-dir "))
                && cmdline.contains(target);

            if firefox_profile_match || chromium_profile_match {
                Some(ProfileProcessCandidate {
                    pid: process.pid().as_u32(),
                    parent_pid: process.parent().map(|pid| pid.as_u32()),
                    name,
                    exe_path: format!("{:?}", process.exe()),
                    status: format!("{:?}", process.status()),
                    start_time: process.start_time(),
                    has_main_window: crate::panic_frame::process_has_visible_main_window(
                        process.pid().as_u32(),
                    ),
                    command_line: cmdline.clone(),
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
            .then_with(|| right.pid.cmp(&left.pid))
    });
    matches
}

pub(crate) fn librewolf_profile_lock_present(profile_data_dir: &Path) -> bool {
    profile_data_dir.join("parent.lock").exists()
        || profile_data_dir.join(".parentlock").exists()
        || profile_data_dir.join("lock").exists()
}

pub(crate) fn find_main_window_pid_for_candidates(candidates: &[ProfileProcessCandidate]) -> Option<u32> {
    candidates
        .iter()
        .filter(|candidate| candidate.has_main_window)
        .max_by_key(|candidate| candidate.start_time)
        .map(|candidate| candidate.pid)
}

