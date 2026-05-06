use regex::Regex;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[test]
fn docs_live_only_under_ru_and_eng_roots() {
    let docs_root = repo_root().join("docs");
    let mut unexpected = Vec::new();

    for path in collect_markdown_files(&docs_root) {
        let rel = relative_to(&docs_root, &path);
        if rel == "README.md" {
            continue;
        }
        let top = rel.split('/').next().unwrap_or_default();
        if top != "ru" && top != "eng" {
            unexpected.push(rel);
        }
    }

    if !unexpected.is_empty() {
        unexpected.sort();
        panic!(
            "docs files are outside the configured wiki roots: {}",
            sample(&unexpected)
        );
    }
}

#[test]
fn docs_ru_and_eng_have_matching_markdown_trees() {
    let repo = repo_root();
    let ru = markdown_rel_set(&repo.join("docs").join("ru"));
    let eng = markdown_rel_set(&repo.join("docs").join("eng"));

    let only_ru = difference(&ru, &eng);
    let only_eng = difference(&eng, &ru);

    if !only_ru.is_empty() || !only_eng.is_empty() {
        panic!(
            "wiki branches diverged; only_ru={}, only_eng={}",
            sample(&only_ru),
            sample(&only_eng)
        );
    }
}

#[test]
fn docs_mandatory_pages_exist() {
    let repo = repo_root();
    let required = [
        "README.md",
        "README.en.md",
        "docs/README.md",
        "docs/ru/README.md",
        "docs/ru/index.md",
        "docs/ru/navigator.md",
        "docs/ru/core-docs/ui.md",
        "docs/ru/core-docs/network-routing.md",
        "docs/ru/core-docs/dns-and-filters.md",
        "docs/ru/core-docs/security.md",
        "docs/ru/release-runbook.md",
        "docs/ru/release-troubleshooting.md",
        "docs/eng/README.md",
        "docs/eng/index.md",
        "docs/eng/navigator.md",
        "docs/eng/core-docs/ui.md",
        "docs/eng/core-docs/network-routing.md",
        "docs/eng/core-docs/dns-and-filters.md",
        "docs/eng/core-docs/security.md",
        "docs/eng/release-runbook.md",
        "docs/eng/release-troubleshooting.md",
    ];

    let mut missing = Vec::new();
    for rel in required {
        let path = repo.join(rel);
        if !path.is_file() {
            missing.push(rel.replace('\\', "/"));
        }
    }

    if !missing.is_empty() {
        missing.sort();
        panic!(
            "mandatory documentation files are missing: {}",
            sample(&missing)
        );
    }
}

#[test]
fn live_docs_and_operator_scripts_do_not_reference_camoufox() {
    let repo = repo_root();
    let allowlisted_docs = BTreeSet::from([
        "README.md",
        "README.en.md",
        "docs/ru/operators/managed-runtime.md",
        "docs/eng/operators/managed-runtime.md",
    ]);
    let mut scan_targets = vec![repo.join("README.md"), repo.join("README.en.md")];
    scan_targets.extend(collect_markdown_files(&repo.join("docs").join("ru")));
    scan_targets.extend(collect_markdown_files(&repo.join("docs").join("eng")));
    scan_targets.extend([
        repo.join("scripts").join("local-ci-preflight.ps1"),
        repo.join("scripts").join("release.ps1"),
        repo.join("scripts").join("published-updater-e2e.ps1"),
    ]);

    let mut offenders = Vec::new();
    for path in scan_targets {
        let rel = relative_to(&repo, &path);
        let content =
            fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", rel));
        let contains_camoufox = content.contains("Camoufox") || content.contains("camoufox");
        if contains_camoufox && !allowlisted_docs.contains(rel.as_str()) {
            offenders.push(rel);
        }
    }

    if !offenders.is_empty() {
        offenders.sort();
        panic!(
            "live docs/operator scripts still reference retired Camoufox path outside the approved decommission notes: {}",
            sample(&offenders)
        );
    }
}

#[test]
fn docs_ru_wiki_is_fully_russian_except_allowed_terms() {
    let docs_ru_root = repo_root().join("docs").join("ru");
    let re_front_matter = Regex::new(r"(?s)\A---.*?---\s*").expect("front matter regex");
    let re_code_fence = Regex::new(r"(?s)```.*?```").expect("code fence regex");
    let re_inline_code = Regex::new(r"`[^`]*`").expect("inline code regex");
    let re_markdown_link = Regex::new(r"\[[^\]]*\]\([^)]+\)").expect("link regex");
    let re_url = Regex::new(r"https?://[^\s)]+").expect("url regex");
    let re_email =
        Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").expect("email regex");
    let re_path_like =
        Regex::new(r"(?m)(^|[\s(])(?:/[A-Za-z0-9._/\-]+|[A-Za-z]:\\[^\s]+)").expect("path regex");
    let re_template = Regex::new(r"\{\{[^{}]+\}\}|\{[^{}]+\}").expect("template regex");
    let re_english_word = Regex::new(r"\b[A-Za-z][A-Za-z0-9_-]*\b").expect("english word regex");
    let re_mojibake = Regex::new(r"(?:Р.|С.){2,}").expect("mojibake regex");
    let allowed = allowed_ru_terms();

    let mut issues = Vec::new();

    for path in collect_markdown_files(&docs_ru_root) {
        let rel = relative_to(&docs_ru_root, &path);
        let raw = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", rel));
        if std::str::from_utf8(&raw).is_err() {
            issues.push(format!("{rel}: invalid UTF-8"));
            continue;
        }
        let content = String::from_utf8(raw).expect("utf8 content");
        if content.contains('\u{FFFD}') || re_mojibake.is_match(&content) {
            issues.push(format!(
                "{rel}: mojibake or replacement characters detected"
            ));
            continue;
        }

        let mut sanitized = re_front_matter.replace(&content, " ").into_owned();
        sanitized = re_code_fence.replace_all(&sanitized, " ").into_owned();
        sanitized = re_inline_code.replace_all(&sanitized, " ").into_owned();
        sanitized = re_markdown_link.replace_all(&sanitized, " ").into_owned();
        sanitized = re_url.replace_all(&sanitized, " ").into_owned();
        sanitized = re_email.replace_all(&sanitized, " ").into_owned();
        sanitized = re_path_like.replace_all(&sanitized, " ").into_owned();
        sanitized = re_template.replace_all(&sanitized, " ").into_owned();

        let mut unexpected = BTreeSet::new();
        for matched in re_english_word.find_iter(&sanitized) {
            let word = matched
                .as_str()
                .trim_matches(&['-', '_'][..])
                .to_ascii_lowercase();
            if word.is_empty() || allowed.contains(word.as_str()) {
                continue;
            }
            unexpected.insert(word);
        }
        if !unexpected.is_empty() {
            issues.push(format!(
                "{rel}: {}",
                unexpected.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }

    if !issues.is_empty() {
        issues.sort();
        panic!(
            "ru wiki contains mixed-language fragments: {}",
            sample(&issues)
        );
    }
}

fn allowed_ru_terms() -> BTreeSet<&'static str> {
    [
        "acme",
        "adr",
        "admin",
        "aio",
        "allowlist",
        "amnezia",
        "amneziawg",
        "anti-bot",
        "anti-ddos",
        "api",
        "asn",
        "audit",
        "auth",
        "backend",
        "backup",
        "backups",
        "bad",
        "base",
        "basic",
        "behavior",
        "binary",
        "binaries",
        "blacklist",
        "block",
        "blocklists",
        "browser",
        "build",
        "cache",
        "librewolf",
        "ca",
        "cerbena",
        "check",
        "checks",
        "cidr",
        "cli",
        "chromium",
        "cloudflare",
        "compose",
        "contract",
        "contracts",
        "cookie",
        "cookies",
        "core",
        "cors",
        "cpu",
        "crs",
        "custom",
        "ddos",
        "default",
        "deny",
        "denylist",
        "desktop",
        "desktop-ui",
        "diagnostics",
        "discord",
        "docusaurus",
        "dns",
        "doh",
        "dot",
        "download",
        "downloads",
        "e2e",
        "email",
        "en",
        "endpoint",
        "endpoints",
        "engine",
        "engines",
        "eng",
        "env",
        "ephemeral",
        "exception",
        "exceptions",
        "export",
        "extensions",
        "filter",
        "filters",
        "fingerprint",
        "firefox",
        "first-launch",
        "flow",
        "frontend",
        "gateway",
        "geo",
        "github",
        "global",
        "guardrails",
        "health",
        "healthcheck",
        "home",
        "http",
        "https",
        "i18n",
        "id",
        "identity",
        "import",
        "irc",
        "json",
        "kill-switch",
        "launcher",
        "lifecycle",
        "local",
        "lock",
        "logs",
        "manual",
        "mailto",
        "mcp",
        "msi",
        "msiexec",
        "md",
        "metrics",
        "mixed-language",
        "mht",
        "mhtml",
        "mms",
        "mode",
        "modes",
        "native",
        "network",
        "news",
        "nntp",
        "node",
        "nodes",
        "npm",
        "openvpn",
        "os",
        "operator",
        "operators",
        "passkey",
        "passkeys",
        "path",
        "paths",
        "pdf",
        "policy",
        "preview",
        "profile",
        "profiles",
        "proxy",
        "qubes",
        "reason",
        "reasons",
        "readme",
        "release",
        "release-gates",
        "release-runbook",
        "restore",
        "route",
        "routing",
        "ru",
        "runtime",
        "rust",
        "security",
        "server",
        "service",
        "services",
        "settings",
        "shell",
        "sign",
        "signature",
        "signatures",
        "sing",
        "sing-box",
        "site",
        "shtml",
        "smoke",
        "sms",
        "smsto",
        "snews",
        "snapshot",
        "snapshots",
        "socks4",
        "socks5",
        "sqlite",
        "standalone",
        "start",
        "state",
        "states",
        "status",
        "stop",
        "svg",
        "sync",
        "tauri",
        "tel",
        "telegram",
        "template",
        "templates",
        "tls",
        "tooling",
        "tor",
        "tos",
        "traffic",
        "troubleshooting",
        "ui",
        "user-facing",
        "unlock",
        "update",
        "url",
        "utf",
        "validator",
        "version",
        "versions",
        "vpn",
        "urn",
        "wayfern",
        "webcal",
        "webgl",
        "wiki",
        "window",
        "windows",
        "workflow",
        "workflows",
        "xht",
        "xhtml",
        "xhy",
        "zero",
        "zero-trust",
    ]
    .into_iter()
    .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn markdown_rel_set(root: &Path) -> BTreeSet<String> {
    collect_markdown_files(root)
        .into_iter()
        .map(|path| relative_to(root, &path))
        .collect()
}

fn difference(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.iter()
        .filter(|value| !right.contains(*value))
        .cloned()
        .collect()
}

fn collect_markdown_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_markdown_files_inner(root, &mut files);
    files
}

fn collect_markdown_files_inner(root: &Path, files: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(root).unwrap_or_else(|error| panic!("read_dir {}: {error}", root.display()));
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let meta = entry.metadata().expect("metadata");
        if meta.is_dir() {
            collect_markdown_files_inner(&path, files);
        } else if meta.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
        {
            files.push(path);
        }
    }
}

fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("relative path")
        .to_string_lossy()
        .replace('\\', "/")
}

fn sample(items: &[String]) -> String {
    let limit = 8usize;
    if items.is_empty() {
        return "[]".to_string();
    }
    let mut view = items.iter().take(limit).cloned().collect::<Vec<_>>();
    if items.len() > limit {
        view.push(format!("... (+{} more)", items.len() - limit));
    }
    format!("[{}]", view.join("; "))
}
