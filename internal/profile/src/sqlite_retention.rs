use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

use crate::{errors::ProfileError, wipe::WipeDataType};

pub fn normalize_site_scopes(scopes: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for scope in scopes {
        let value = scope.trim().trim_start_matches('.').to_ascii_lowercase();
        if value.is_empty() || normalized.iter().any(|item| item == &value) {
            continue;
        }
        normalized.push(value);
    }
    normalized
}

pub fn retain_scoped_engine_data(
    root: &Path,
    ty: WipeDataType,
    scopes: &[String],
) -> Result<Vec<PathBuf>, ProfileError> {
    if scopes.is_empty() {
        return Ok(Vec::new());
    }
    let engine_root = root.join("engine-profile");
    let mut preserved = Vec::new();
    match ty {
        WipeDataType::Cookies => {
            for path in [
                engine_root.join("Default").join("Network").join("Cookies"),
                engine_root.join("Default").join("Cookies"),
            ] {
                if prune_chromium_cookie_store(&path, scopes)? {
                    preserved.push(path);
                }
            }
            let firefox = engine_root.join("cookies.sqlite");
            if prune_firefox_cookie_store(&firefox, scopes)? {
                preserved.push(firefox);
            }
        }
        WipeDataType::History => {
            let chromium = engine_root.join("Default").join("History");
            if prune_chromium_history_store(&chromium, scopes)? {
                preserved.push(chromium);
            }
            let firefox = engine_root.join("places.sqlite");
            if prune_firefox_history_store(&firefox, scopes)? {
                preserved.push(firefox);
            }
        }
        _ => {}
    }
    Ok(preserved)
}

fn prune_chromium_cookie_store(path: &Path, scopes: &[String]) -> Result<bool, ProfileError> {
    if !path.exists() {
        return Ok(false);
    }
    let mut conn = Connection::open(path)?;
    if !table_exists(&conn, "cookies")? {
        finalize_sqlite(&mut conn)?;
        return Ok(true);
    }
    let row_ids = {
        let mut stmt = conn.prepare("SELECT rowid, host_key FROM cookies")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut pending = Vec::new();
        for row in rows {
            let (row_id, host) = row?;
            if !host_matches_scope(&host, scopes) {
                pending.push(row_id);
            }
        }
        pending
    };
    delete_rowids(&mut conn, "DELETE FROM cookies WHERE rowid = ?1", &row_ids)?;
    finalize_sqlite(&mut conn)?;
    Ok(true)
}

fn prune_firefox_cookie_store(path: &Path, scopes: &[String]) -> Result<bool, ProfileError> {
    if !path.exists() {
        return Ok(false);
    }
    let mut conn = Connection::open(path)?;
    if !table_exists(&conn, "moz_cookies")? {
        finalize_sqlite(&mut conn)?;
        return Ok(true);
    }
    let row_ids = {
        let mut stmt = conn.prepare("SELECT id, host FROM moz_cookies")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut pending = Vec::new();
        for row in rows {
            let (row_id, host) = row?;
            if !host_matches_scope(&host, scopes) {
                pending.push(row_id);
            }
        }
        pending
    };
    delete_rowids(&mut conn, "DELETE FROM moz_cookies WHERE id = ?1", &row_ids)?;
    finalize_sqlite(&mut conn)?;
    Ok(true)
}

fn prune_chromium_history_store(path: &Path, scopes: &[String]) -> Result<bool, ProfileError> {
    if !path.exists() {
        return Ok(false);
    }
    let mut conn = Connection::open(path)?;
    if !table_exists(&conn, "urls")? || !table_exists(&conn, "visits")? {
        finalize_sqlite(&mut conn)?;
        return Ok(true);
    }
    let url_ids = {
        let mut stmt = conn.prepare("SELECT id, url FROM urls")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut pending = Vec::new();
        for row in rows {
            let (row_id, url) = row?;
            if !url_matches_scope(&url, scopes) {
                pending.push(row_id);
            }
        }
        pending
    };
    if !url_ids.is_empty() {
        let tx = conn.transaction()?;
        for row_id in &url_ids {
            tx.execute("DELETE FROM visits WHERE url = ?1", [row_id])?;
            let _ = tx.execute("DELETE FROM keyword_search_terms WHERE url_id = ?1", [row_id]);
            let _ = tx.execute("DELETE FROM segments WHERE url_id = ?1", [row_id]);
            tx.execute("DELETE FROM urls WHERE id = ?1", [row_id])?;
        }
        tx.commit()?;
    }
    finalize_sqlite(&mut conn)?;
    Ok(true)
}

fn prune_firefox_history_store(path: &Path, scopes: &[String]) -> Result<bool, ProfileError> {
    if !path.exists() {
        return Ok(false);
    }
    let mut conn = Connection::open(path)?;
    if !table_exists(&conn, "moz_places")? || !table_exists(&conn, "moz_historyvisits")? {
        finalize_sqlite(&mut conn)?;
        return Ok(true);
    }
    let place_ids = {
        let mut stmt = conn.prepare("SELECT id, url FROM moz_places")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        let mut pending = Vec::new();
        for row in rows {
            let (row_id, url) = row?;
            let keep = url
                .as_deref()
                .map(|value| url_matches_scope(value, scopes))
                .unwrap_or(false);
            if !keep {
                pending.push(row_id);
            }
        }
        pending
    };
    if !place_ids.is_empty() {
        let tx = conn.transaction()?;
        for row_id in &place_ids {
            tx.execute("DELETE FROM moz_historyvisits WHERE place_id = ?1", [row_id])?;
            let _ = tx.execute("DELETE FROM moz_inputhistory WHERE place_id = ?1", [row_id]);
            tx.execute(
                "DELETE FROM moz_places WHERE id = ?1 AND id NOT IN (SELECT fk FROM moz_bookmarks)",
                [row_id],
            )?;
        }
        tx.commit()?;
    }
    finalize_sqlite(&mut conn)?;
    Ok(true)
}

fn finalize_sqlite(conn: &mut Connection) -> Result<(), ProfileError> {
    conn.execute_batch(
        "PRAGMA wal_checkpoint(TRUNCATE);
         PRAGMA optimize;
         VACUUM;",
    )?;
    Ok(())
}

fn delete_rowids(
    conn: &mut Connection,
    sql: &str,
    row_ids: &[i64],
) -> Result<(), ProfileError> {
    if row_ids.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    for row_id in row_ids {
        tx.execute(sql, params![row_id])?;
    }
    tx.commit()?;
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, ProfileError> {
    let exists = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists == 1)
}

fn host_matches_scope(host: &str, scopes: &[String]) -> bool {
    let normalized = host.trim().trim_start_matches('.').to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    scopes
        .iter()
        .any(|scope| normalized == *scope || normalized.ends_with(&format!(".{scope}")))
}

fn url_matches_scope(url: &str, scopes: &[String]) -> bool {
    extract_url_host(url)
        .map(|host| host_matches_scope(host, scopes))
        .unwrap_or(false)
}

fn extract_url_host(url: &str) -> Option<&str> {
    let trimmed = url.trim();
    let after_scheme = trimmed.split_once("://")?.1;
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim();
    if authority.is_empty() {
        return None;
    }
    if let Some(rest) = authority.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = &rest[..end];
        return if host.is_empty() { None } else { Some(host) };
    }
    let host = authority.split(':').next().unwrap_or_default().trim();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_url_host, host_matches_scope, normalize_site_scopes, url_matches_scope};

    #[test]
    fn normalize_scopes_trims_and_deduplicates() {
        let scopes = normalize_site_scopes(&[
            " Example.com ".to_string(),
            ".example.com".to_string(),
            "sub.example.com".to_string(),
        ]);
        assert_eq!(scopes, vec!["example.com".to_string(), "sub.example.com".to_string()]);
    }

    #[test]
    fn host_scope_matching_supports_subdomains() {
        let scopes = vec!["example.com".to_string()];
        assert!(host_matches_scope(".cdn.example.com", &scopes));
        assert!(host_matches_scope("example.com", &scopes));
        assert!(!host_matches_scope("evil-example.com", &scopes));
    }

    #[test]
    fn url_scope_matching_extracts_hosts() {
        let scopes = vec!["example.com".to_string()];
        assert_eq!(
            extract_url_host("https://sub.example.com/path?q=1"),
            Some("sub.example.com")
        );
        assert!(url_matches_scope("https://sub.example.com/path?q=1", &scopes));
        assert!(!url_matches_scope("file:///tmp/demo", &scopes));
    }
}
