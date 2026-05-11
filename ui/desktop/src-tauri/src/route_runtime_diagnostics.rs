use super::*;

pub(crate) fn tail_lines_impl(text: &str, max_lines: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed
        .lines()
        .rev()
        .take(max_lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
}

pub(crate) fn describe_process_failure_impl(output: &Output, label: &str) -> String {
    let code = output
        .status
        .code()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "terminated".to_string());
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let mut details = Vec::new();
    if !stdout.is_empty() {
        details.push(format!("stdout: {stdout}"));
    }
    if !stderr.is_empty() {
        details.push(format!("stderr: {stderr}"));
    }
    if details.is_empty() {
        format!("{label} failed with code {code}")
    } else {
        format!("{label} failed with code {code} ({})", details.join(" | "))
    }
}

pub(crate) fn parse_sc_state_code_impl(raw: &str) -> Option<u32> {
    for line in raw.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_uppercase().starts_with("STATE") {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        while let Some(part) = parts.next() {
            if part == ":" {
                break;
            }
            if let Ok(code) = part.parse::<u32>() {
                return Some(code);
            }
        }
        if let Some(value) = trimmed.split(':').nth(1) {
            let token = value.split_whitespace().next().unwrap_or_default();
            if let Ok(code) = token.parse::<u32>() {
                return Some(code);
            }
        }
    }
    None
}
