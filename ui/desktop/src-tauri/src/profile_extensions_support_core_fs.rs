use super::{fs, BTreeSet, Path};

pub(crate) fn collect_extension_dir_names(root: &Path, ids: &mut BTreeSet<String>) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)
        .map_err(|e| format!("read profile extension dir {}: {e}", root.display()))?
    {
        let entry = entry.map_err(|e| format!("read profile extension entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        ids.insert(name.to_string());
    }
    Ok(())
}

pub(crate) fn suppress_dark_reader_install_tab(unpacked_dir: &Path) -> Result<(), String> {
    let background_path = unpacked_dir.join("background").join("index.js");
    if !background_path.is_file() {
        return Ok(());
    }
    let source = fs::read_to_string(&background_path)
        .map_err(|e| format!("read dark-reader background script: {e}"))?;
    if source.contains("cerbena_dark_reader_install_tab_suppressed") {
        let marker = unpacked_dir.join(".cerbena-prefer-external-manifest");
        let _ = fs::write(&marker, b"1");
        return Ok(());
    }
    let mut updated = source.clone();
    if let Some(help_call_pos) = source.find("chrome.tabs.create({url: getHelpURL()});") {
        let mut start = help_call_pos;
        while start > 0 && source.as_bytes()[start - 1].is_ascii_whitespace() {
            start -= 1;
        }
        let indent = &source[start..help_call_pos];
        let replacement = format!(
            "{}/* cerbena_dark_reader_install_tab_suppressed */",
            indent
        );
        updated.replace_range(
            help_call_pos..(help_call_pos + "chrome.tabs.create({url: getHelpURL()});".len()),
            &replacement,
        );
    } else if let Some(help_call_pos) = source.find("chrome.tabs.create({url:getHelpURL()})") {
        let replacement = "/* cerbena_dark_reader_install_tab_suppressed */";
        updated.replace_range(
            help_call_pos..(help_call_pos + "chrome.tabs.create({url:getHelpURL()})".len()),
            replacement,
        );
    }
    if updated == source {
        eprintln!(
            "[profile-extensions] dark-reader install tab patch skipped path={} reason=pattern_not_found",
            background_path.display()
        );
        return Ok(());
    }
    fs::write(&background_path, updated)
        .map_err(|e| format!("write patched dark-reader background script: {e}"))?;
    let marker = unpacked_dir.join(".cerbena-prefer-external-manifest");
    fs::write(&marker, b"1").map_err(|e| {
        format!(
            "write dark-reader external-manifest marker {}: {e}",
            marker.display()
        )
    })?;
    eprintln!(
        "[profile-extensions] dark-reader install tab suppressed path={}",
        background_path.display()
    );
    Ok(())
}
