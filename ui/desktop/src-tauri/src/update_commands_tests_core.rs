use super::{
    asset_rank, build_msi_apply_helper_script, build_release_http_client,
    build_zip_apply_helper_script, can_auto_apply_asset, can_auto_apply_asset_for_os,
    default_auto_update_enabled, download_release_bytes,
    ensure_asset_matches_verified_checksum, extract_checksum_for_asset,
    fetch_latest_release_from_url, is_version_newer, normalize_version,
    pick_release_asset_for_context, reconcile_update_store_with_current_version,
    release_signing_public_keys, resolve_latest_release_api_url,
    resolve_relaunch_executable_path, sha256_hex,
    should_auto_close_updater_after_ready_to_restart, should_run_auto_update_check,
    signature_verification_variants, AppUpdateStore, GithubReleaseAsset, SelectedAssetKind,
    UpdaterLaunchMode, VerifiedReleaseSecurityBundle, CURRENT_VERSION,
    RELEASE_CHECKSUMS_B64_ENV, RELEASE_CHECKSUMS_SIGNATURE_B64_ENV, RELEASE_LATEST_API_URL_ENV,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

fn next_release_version(version: &str) -> String {
    let normalized = normalize_version(version);
    let mut base_and_suffix = normalized.splitn(2, '-');
    let base = base_and_suffix.next().unwrap_or_default();
    let suffix = base_and_suffix.next();

    if let Some(hotfix_suffix) = suffix.filter(|value| {
        !value.is_empty()
            && value.split('.').all(|segment| {
                !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_digit())
            })
    }) {
        let mut hotfix_parts = hotfix_suffix
            .split('.')
            .map(|value| value.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>();
        if let Some(last) = hotfix_parts.last_mut() {
            *last += 1;
        } else {
            hotfix_parts.push(1);
        }
        return format!(
            "{base}-{}",
            hotfix_parts
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(".")
        );
    }

    let mut parts = base
        .split('.')
        .map(|value| value.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    if let Some(last) = parts.last_mut() {
        *last += 1;
    } else {
        parts.push(1);
    }
    parts
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn spawn_http_server(
    routes: Vec<(
        String,
        Vec<u8>,
        &'static str,
        Vec<(&'static str, &'static str)>,
    )>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test http server");
    let addr = listener.local_addr().expect("local addr");
    thread::spawn(move || {
        for _ in 0..routes.len() {
            let (mut stream, _) = listener.accept().expect("accept test connection");
            let mut buffer = [0u8; 8192];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            let first_line = request.lines().next().unwrap_or_default();
            let path = first_line.split_whitespace().nth(1).unwrap_or("/");
            let (status, body, content_type, extra_headers) = routes
                .iter()
                .find(|(route, _, _, _)| route == path)
                .map(|(_, body, content_type, headers)| {
                    ("200 OK", body.clone(), *content_type, headers.clone())
                })
                .unwrap_or_else(|| {
                    (
                        "404 Not Found",
                        b"not found".to_vec(),
                        "text/plain",
                        Vec::new(),
                    )
                });
            let mut headers = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n",
                body.len()
            );
            for (name, value) in extra_headers {
                headers.push_str(&format!("{name}: {value}\r\n"));
            }
            headers.push_str("\r\n");
            stream.write_all(headers.as_bytes()).expect("write headers");
            stream.write_all(&body).expect("write body");
            stream.flush().expect("flush response");
        }
    });
    format!("http://{}", addr)
}

#[path = "update_commands_tests_core_cases.rs"]
mod cases;
