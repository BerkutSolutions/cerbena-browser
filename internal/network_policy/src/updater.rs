use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlocklistSource {
    LocalFile { path: PathBuf },
    RemoteUrl {
        url: String,
        #[serde(default = "default_true")]
        require_https: bool,
        #[serde(default)]
        expected_sha256: Option<String>,
    },
    InlineDomains { domains: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsListSnapshot {
    pub list_id: String,
    pub domains: Vec<String>,
    pub updated_at_epoch: u64,
}

#[derive(Debug, Clone)]
pub struct DnsBlocklistUpdater {
    pub update_interval_hours: u64,
}

#[derive(Debug, Error)]
pub enum UpdaterError {
    #[error("invalid source: {0}")]
    InvalidSource(String),
    #[error("download error: {0}")]
    Download(String),
    #[error("integrity error: {0}")]
    Integrity(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl DnsBlocklistUpdater {
    pub fn new() -> Self {
        Self {
            update_interval_hours: 12,
        }
    }

    pub fn update_from_source(
        &self,
        list_id: &str,
        source: &BlocklistSource,
    ) -> Result<DnsListSnapshot, UpdaterError> {
        let domains = match source {
            BlocklistSource::LocalFile { path } => parse_local_file(path)?,
            BlocklistSource::RemoteUrl {
                url,
                require_https,
                expected_sha256,
            } => parse_remote_url(url, *require_https, expected_sha256.as_deref())?,
            BlocklistSource::InlineDomains { domains } => normalize(domains.clone()),
        };
        Ok(DnsListSnapshot {
            list_id: list_id.to_string(),
            domains,
            updated_at_epoch: now_epoch(),
        })
    }

    pub fn should_refresh(&self, snapshot: &DnsListSnapshot, now_epoch: u64) -> bool {
        let ttl = self.update_interval_hours.saturating_mul(3600);
        now_epoch.saturating_sub(snapshot.updated_at_epoch) >= ttl
    }
}

fn parse_local_file(path: &Path) -> Result<Vec<String>, UpdaterError> {
    if !path.exists() {
        return Err(UpdaterError::InvalidSource(format!(
            "blocklist file does not exist: {}",
            path.display()
        )));
    }
    let content = fs::read_to_string(path)?;
    let mut items = Vec::new();
    for line in content.lines() {
        let v = line.trim();
        if v.is_empty() || v.starts_with('#') {
            continue;
        }
        items.push(v.to_string());
    }
    Ok(normalize(items))
}

fn parse_remote_url(
    url: &str,
    require_https: bool,
    expected_sha256: Option<&str>,
) -> Result<Vec<String>, UpdaterError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| UpdaterError::InvalidSource(format!("bad url: {e}")))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if require_https && scheme != "https" {
        return Err(UpdaterError::InvalidSource(
            "remote blocklist URL must use https".to_string(),
        ));
    }
    if scheme != "https" && scheme != "http" {
        return Err(UpdaterError::InvalidSource(format!(
            "unsupported remote blocklist url scheme: {scheme}"
        )));
    }
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Cerbena/0.1")
        .build()
        .map_err(|e| UpdaterError::Download(e.to_string()))?;
    let response = client
        .get(parsed)
        .send()
        .map_err(|e| UpdaterError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(UpdaterError::Download(format!(
            "http status {}",
            response.status()
        )));
    }
    let content = response
        .text()
        .map_err(|e| UpdaterError::Download(e.to_string()))?;
    if let Some(expected_sha256) = expected_sha256 {
        let actual = sha256_hex(content.as_bytes());
        if !actual.eq_ignore_ascii_case(expected_sha256.trim()) {
            return Err(UpdaterError::Integrity(format!(
                "remote blocklist checksum mismatch: expected {}, got {}",
                expected_sha256.trim(),
                actual
            )));
        }
    }
    Ok(parse_blocklist_content(&content))
}

fn parse_blocklist_content(content: &str) -> Vec<String> {
    let mut items = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        if let Some(value) = parse_hosts_line(line) {
            items.push(value);
            continue;
        }
        let cleaned = line
            .trim_start_matches("||")
            .trim_start_matches('.')
            .trim_end_matches('^')
            .trim_end_matches('/')
            .trim();
        if !cleaned.is_empty() && !cleaned.contains(' ') {
            items.push(cleaned.to_string());
        }
    }
    normalize(items)
}

fn parse_hosts_line(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace();
    let first = parts.next()?;
    let second = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if first == "0.0.0.0" || first == "127.0.0.1" || first == "::1" {
        return Some(second.trim_start_matches('.').to_string());
    }
    None
}

fn normalize(items: Vec<String>) -> Vec<String> {
    let mut set = BTreeSet::new();
    for i in items {
        let v = i.trim().to_lowercase();
        if !v.is_empty() {
            set.insert(v);
        }
    }
    set.into_iter().collect()
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn default_true() -> bool {
    true
}
