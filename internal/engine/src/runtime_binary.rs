use super::*;

pub(super) fn candidate_names_impl(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

pub(super) fn find_first_match_impl(root: &Path, candidates: &[String]) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let file_name = path.file_name()?.to_string_lossy().to_lowercase();
            if candidates
                .iter()
                .any(|item| item.eq_ignore_ascii_case(&file_name))
            {
                return Some(path);
            }
        }
    }
    None
}

pub(super) fn prefer_librewolf_browser_binary_impl(current: &Path) -> PathBuf {
    let Some(root) = current.parent() else {
        return current.to_path_buf();
    };
    let librewolf_candidates = candidate_names_impl(&["librewolf.exe", "librewolf"]);
    if let Some(path) = find_first_match_impl(root, &librewolf_candidates) {
        return path;
    }
    let firefox_candidates = candidate_names_impl(&["firefox.exe", "firefox"]);
    if let Some(path) = find_first_match_impl(root, &firefox_candidates) {
        return path;
    }
    let private_candidates = candidate_names_impl(&["private_browsing.exe", "private_browsing"]);
    if let Some(path) = find_first_match_impl(root, &private_candidates) {
        return path;
    }
    current.to_path_buf()
}

pub(super) fn prefer_chromium_vendor_binary_impl(current: &Path) -> PathBuf {
    let Some(parent) = current.parent() else {
        return current.to_path_buf();
    };
    let current_name = current
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if current_name == "chrome.exe" || current_name == "chrome" {
        return current.to_path_buf();
    }
    let chrome_candidates = candidate_names_impl(&["chrome.exe", "chrome"]);
    if let Some(path) = find_first_match_impl(parent, &chrome_candidates) {
        return path;
    }
    current.to_path_buf()
}
