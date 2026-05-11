use super::*;

pub(super) fn launch_args_firefox_family_impl(
    engine: EngineKind,
    runtime_dir: &Path,
    start_page: Option<&str>,
) -> Vec<String> {
    match engine {
        EngineKind::FirefoxEsr => {
            let mut args = vec![
                "-no-remote".to_string(),
                "-profile".to_string(),
                runtime_dir.to_string_lossy().to_string(),
            ];
            if let Some(page) = start_page.map(str::trim).filter(|value| !value.is_empty()) {
                args.push(page.to_string());
            }
            args
        }
        EngineKind::Librewolf => {
            let mut args = vec![
                "-no-remote".to_string(),
                "-new-instance".to_string(),
                "-profile".to_string(),
                runtime_dir.to_string_lossy().to_string(),
            ];
            if let Some(page) = start_page.map(str::trim).filter(|value| !value.is_empty()) {
                args.push(page.to_string());
            }
            args
        }
        _ => Vec::new(),
    }
}

pub(super) fn sanitize_librewolf_launch_args_impl(args: &mut Vec<String>) {
    let has_no_remote = args
        .iter()
        .any(|value| value.eq_ignore_ascii_case("-no-remote"));
    let has_new_instance = args
        .iter()
        .any(|value| value.eq_ignore_ascii_case("-new-instance"));
    if !has_no_remote {
        args.insert(0, "-no-remote".to_string());
    }
    if !has_new_instance {
        let insert_at = if args
            .first()
            .map(|value| value.eq_ignore_ascii_case("-no-remote"))
            .unwrap_or(false)
        {
            1
        } else {
            0
        };
        args.insert(insert_at, "-new-instance".to_string());
    }
}

pub(super) fn reopen_args_firefox_family_impl(runtime_dir: &Path, url: &str) -> Vec<String> {
    vec![
        "-profile".to_string(),
        runtime_dir.to_string_lossy().to_string(),
        "-new-tab".to_string(),
        url.trim().to_string(),
    ]
}
