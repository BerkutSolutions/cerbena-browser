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

use crate::launch_sessions::revoke_launch_session;
use crate::panic_frame::close_panic_frame;
use crate::route_runtime::stop_profile_route_runtime;
use crate::state::AppState;

pub fn track_profile_process(
    app_handle: AppHandle,
    profile_id: Uuid,
    pid: u32,
    profile_data_dir: PathBuf,
) {
    std::thread::spawn(move || {
        let mut tracked_pid = pid;
        let mut missing_profile_ticks = 0u8;
        loop {
            if let Some(actual_pid) = find_profile_process_pid(&profile_data_dir) {
                missing_profile_ticks = 0;
                if actual_pid != tracked_pid {
                    replace_tracked_pid(&app_handle, profile_id, tracked_pid, actual_pid);
                    tracked_pid = actual_pid;
                }
            } else {
                missing_profile_ticks = missing_profile_ticks.saturating_add(1);
                if !is_process_running(tracked_pid) || missing_profile_ticks >= 4 {
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
    if current_pid != pid {
        return;
    }
    launched.remove(&profile_id);
    drop(launched);
    let _ = revoke_launch_session(state.inner(), profile_id, Some(pid));
    stop_profile_route_runtime(app_handle, profile_id);
    close_panic_frame(app_handle, profile_id);

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
    let sessions = {
        let state = app_handle.state::<AppState>();
        let launched = match state.launched_processes.lock() {
            Ok(value) => value,
            Err(_) => return,
        };
        launched
            .iter()
            .map(|(profile_id, pid)| (*profile_id, *pid))
            .collect::<Vec<_>>()
    };
    for (profile_id, pid) in sessions {
        terminate_process_tree(pid);
        clear_profile_process(app_handle, profile_id, pid, false);
    }
}

pub fn terminate_process_tree(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
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
    #[cfg(target_os = "windows")]
    {
        let filter = format!("PID eq {pid}");
        return Command::new("tasklist")
            .args(["/FI", &filter, "/FO", "CSV", "/NH"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                let body = String::from_utf8_lossy(&output.stdout);
                body.lines().any(|line| {
                    line.contains(&format!(",\"{pid}\",")) || line.contains(&format!("\"{pid}\""))
                })
            })
            .unwrap_or(false);
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

fn find_profile_process_pid(profile_data_dir: &Path) -> Option<u32> {
    let target = profile_data_dir.to_string_lossy().to_lowercase();
    #[cfg(target_os = "windows")]
    if let Some(pid) = find_profile_process_pid_windows(profile_data_dir, &target, false) {
        return Some(pid);
    }
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    for process in system.processes().values() {
        let name = process.name().to_lowercase();
        let cmdline = process
            .cmd()
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let firefox_profile_match = (name.contains("camoufox")
            || name.contains("firefox")
            || name.contains("private_browsing"))
            && cmdline.contains("-profile")
            && cmdline.contains(&target);
        let chromium_profile_match =
            (name.contains("wayfern") || name.contains("chrome") || name.contains("chromium"))
                && (cmdline.contains("--user-data-dir=") || cmdline.contains("--user-data-dir "))
                && cmdline.contains(&target);
        if firefox_profile_match || chromium_profile_match {
            return Some(process.pid().as_u32());
        }
    }
    find_profile_process_pid_windows(profile_data_dir, &target, false)
}

#[cfg(target_os = "windows")]
fn find_profile_process_pid_windows(
    profile_data_dir: &Path,
    target: &str,
    require_main_window: bool,
) -> Option<u32> {
    let normalized = profile_data_dir.to_string_lossy().replace('\\', "\\\\");
    let script = format!(
        "$path='{normalized}'; \
         $items = Get-CimInstance Win32_Process | \
         Where-Object {{ \
           $_.CommandLine -and ( \
             ((($_.Name -like 'camoufox*') -or ($_.Name -like 'firefox*') -or ($_.Name -like 'private_browsing*')) -and $_.CommandLine -like '*-profile*' -and $_.CommandLine -like \"*${{path}}*\") -or \
             ((($_.Name -like 'wayfern*') -or ($_.Name -like 'chrome*') -or ($_.Name -like 'chromium*')) -and (($_.CommandLine -like '*--user-data-dir=*') -or ($_.CommandLine -like '*--user-data-dir *')) -and $_.CommandLine -like \"*${{path}}*\") \
           ) \
         }} | ForEach-Object {{ \
           $proc = Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue; \
           [pscustomobject]@{{ ProcessId = $_.ProcessId; MainWindowHandle = if ($proc) {{ $proc.MainWindowHandle }} else {{ 0 }}; CommandLineLength = if ($_.CommandLine) {{ $_.CommandLine.Length }} else {{ 0 }}; }} \
         }}; \
         if ({require_main_window}) {{ $items = $items | Where-Object {{ $_.MainWindowHandle -ne 0 }}; }}; \
         $items | Sort-Object @{{ Expression = {{ if ($_.MainWindowHandle -ne 0) {{ 0 }} else {{ 1 }} }} }}, @{{ Expression = 'CommandLineLength'; Descending = $false }}, @{{ Expression = 'ProcessId'; Descending = $false }} | Select-Object -First 1 ProcessId | ConvertTo-Json -Compress"
    );
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json = String::from_utf8_lossy(&output.stdout);
    let parsed = serde_json::from_str::<serde_json::Value>(&json).ok()?;
    let pid = parsed.get("ProcessId").and_then(|v| v.as_u64())? as u32;
    if target.is_empty() {
        None
    } else {
        Some(pid)
    }
}

#[cfg(not(target_os = "windows"))]
fn find_profile_process_pid_windows(_profile_data_dir: &Path, _target: &str) -> Option<u32> {
    None
}

#[cfg(target_os = "windows")]
fn find_profile_process_pids_windows(profile_data_dir: &Path, _target: &str) -> Vec<u32> {
    let normalized = profile_data_dir.to_string_lossy().replace('\\', "\\\\");
    let script = format!(
        "$path='{normalized}'; \
         Get-CimInstance Win32_Process | \
         Where-Object {{ \
           $_.CommandLine -and ( \
             ((($_.Name -like 'camoufox*') -or ($_.Name -like 'firefox*') -or ($_.Name -like 'private_browsing*')) -and $_.CommandLine -like '*-profile*' -and $_.CommandLine -like \"*${{path}}*\") -or \
             ((($_.Name -like 'wayfern*') -or ($_.Name -like 'chrome*') -or ($_.Name -like 'chromium*')) -and (($_.CommandLine -like '*--user-data-dir=*') -or ($_.CommandLine -like '*--user-data-dir *')) -and $_.CommandLine -like \"*${{path}}*\") \
           ) \
         }} | Select-Object -ExpandProperty ProcessId | ConvertTo-Json -Compress"
    );
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let json = String::from_utf8_lossy(&output.stdout);
    let trimmed = json.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(serde_json::Value::Array(values)) => values
            .into_iter()
            .filter_map(|value| value.as_u64().map(|pid| pid as u32))
            .collect(),
        Ok(serde_json::Value::Number(value)) => value
            .as_u64()
            .map(|pid| vec![pid as u32])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(not(target_os = "windows"))]
fn find_profile_process_pids_windows(_profile_data_dir: &Path, _target: &str) -> Vec<u32> {
    Vec::new()
}

pub fn find_profile_process_pid_for_dir(profile_data_dir: &Path) -> Option<u32> {
    find_profile_process_pid(profile_data_dir)
}

pub fn find_profile_main_window_pid_for_dir(profile_data_dir: &Path) -> Option<u32> {
    #[cfg(target_os = "windows")]
    {
        let target = profile_data_dir.to_string_lossy().to_lowercase();
        return find_profile_process_pid_windows(profile_data_dir, &target, true);
    }
    #[cfg(not(target_os = "windows"))]
    {
        find_profile_process_pid(profile_data_dir)
    }
}

pub fn find_profile_process_pids_for_dir(profile_data_dir: &Path) -> Vec<u32> {
    #[cfg(target_os = "windows")]
    {
        let target = profile_data_dir.to_string_lossy().to_lowercase();
        return find_profile_process_pids_windows(profile_data_dir, &target);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let target = profile_data_dir.to_string_lossy().to_lowercase();
        let system = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
        );
        let mut matches = Vec::new();
        for process in system.processes().values() {
            let name = process.name().to_lowercase();
            let cmdline = process
                .cmd()
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            let firefox_profile_match = (name.contains("camoufox")
                || name.contains("firefox")
                || name.contains("private_browsing"))
                && cmdline.contains("-profile")
                && cmdline.contains(&target);
            let chromium_profile_match =
                (name.contains("wayfern") || name.contains("chrome") || name.contains("chromium"))
                    && (cmdline.contains("--user-data-dir=") || cmdline.contains("--user-data-dir "))
                    && cmdline.contains(&target);
            if firefox_profile_match || chromium_profile_match {
                matches.push(process.pid().as_u32());
            }
        }
        matches
    }
}
