use super::{LaunchContext, Duration, Instant, thread};

pub(super) fn select_reusable_discovered_pid(discovered_pids: &[u32]) -> Option<u32> {
    discovered_pids
        .iter()
        .copied()
        .find(|pid| *pid > 0)
}

pub(super) fn wait_for_discovered_profile_pids(context: &LaunchContext) -> Vec<u32> {
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut discovered = Vec::new();
    while Instant::now() < deadline {
        discovered = crate::process_tracking::find_profile_process_pids_for_dir(&context.user_data_dir);
        if !discovered.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(300));
    }
    discovered
}
