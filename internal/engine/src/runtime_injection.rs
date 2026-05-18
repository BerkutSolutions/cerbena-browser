use super::*;

pub(super) fn chromium_extension_version_impl(raw: &str) -> String {
    let normalized = raw.trim().trim_start_matches('v');
    let mut parts = Vec::new();
    for segment in normalized.split(['.', '-']) {
        if segment.is_empty() {
            continue;
        }
        if segment.chars().all(|ch| ch.is_ascii_digit()) {
            parts.push(segment.to_string());
        } else {
            break;
        }
        if parts.len() == 4 {
            break;
        }
    }
    if parts.is_empty() {
        "1".to_string()
    } else {
        parts.join(".")
    }
}

pub(super) fn prepare_chromium_blocking_extension_impl(
    profile_root: &Path,
) -> Result<Option<PathBuf>, EngineError> {
    let blocked_domains = blocked_domains_for_profile_impl(profile_root)?
        .into_iter()
        .filter(|domain| domain != "cerbena.blocked.local")
        .collect::<Vec<_>>();
    let locked_app = load_locked_app_config_impl(profile_root)?;
    let identity = load_identity_launch_policy(profile_root);
    let accept_languages = identity
        .as_ref()
        .map(|policy| {
            normalize_accept_languages(&policy.locale.navigator_language, &policy.locale.languages)
        })
        .unwrap_or_default();
    if blocked_domains.is_empty() && locked_app.is_none() && accept_languages.is_empty() {
        return Ok(None);
    }

    let extension_dir = profile_root
        .join("policy")
        .join("chromium-policy-extension");
    fs::create_dir_all(&extension_dir)?;
    let (default_start_page, default_search_provider) = load_profile_browser_defaults(profile_root);
    let mut manifest = serde_json::json!({
        "manifest_version": 3,
        "name": "Cerbena Policy Firewall",
        "version": chromium_extension_version_impl(CHROMIUM_POLICY_EXTENSION_VERSION),
        "description": "Profile-scoped outbound policy enforcement for blocked domains.",
        "declarative_net_request": {
            "rule_resources": [
                {
                    "id": "policy_rules",
                    "enabled": true,
                    "path": "rules.json"
                }
            ]
        },
        "permissions": [
            "declarativeNetRequest",
            "declarativeNetRequestFeedback",
            "declarativeNetRequestWithHostAccess",
            "webRequest"
        ],
        "host_permissions": ["<all_urls>"],
        "background": {
            "service_worker": "service-worker.js"
        },
        "web_accessible_resources": [
            {
                "resources": ["blocked.html", "blocked.css"],
                "matches": ["<all_urls>"]
            }
        ]
    });
    if let Some(overrides) =
        chromium_settings_overrides(default_start_page.as_deref(), default_search_provider.as_deref())
    {
        manifest["chrome_settings_overrides"] = overrides;
    }
    let mut rules = Vec::new();
    if !accept_languages.is_empty() {
        let accept_language_header = build_accept_language_header(&accept_languages);
        rules.push(serde_json::json!({
            "id": 1,
            "priority": 1,
            "action": {
                "type": "modifyHeaders",
                "requestHeaders": [
                    {
                        "header": "Accept-Language",
                        "operation": "set",
                        "value": accept_language_header
                    }
                ]
            },
            "condition": {
                "regexFilter": "^https?://",
                "resourceTypes": [
                    "main_frame", "sub_frame", "stylesheet", "script", "image", "font",
                    "object", "xmlhttprequest", "ping", "media", "websocket", "webtransport", "other"
                ]
            }
        }));
    }
    let base_rule_id = rules.len() + 1;
    let blocked_domains_for_worker = blocked_domains.clone();
    for (index, domain) in blocked_domains.into_iter().enumerate() {
        let rule_id = base_rule_id + index * 2;
        let rule_host = domain.clone();
        let rule_url = format!("https://{rule_host}/");
        let redirect_path = format!("/blocked.html?host={rule_host}&url={rule_url}");
        rules.push(serde_json::json!({
            "id": rule_id,
            "priority": 2,
            "action": {
                "type": "redirect",
                "redirect": { "extensionPath": redirect_path }
            },
            "condition": {
                "urlFilter": format!("||{domain}^"),
                "resourceTypes": ["main_frame", "sub_frame"]
            }
        }));
        rules.push(serde_json::json!({
            "id": rule_id + 1,
            "priority": 2,
            "action": { "type": "block" },
            "condition": {
                "urlFilter": format!("||{domain}^"),
                "resourceTypes": [
                    "stylesheet", "script", "image", "font",
                    "object", "xmlhttprequest", "ping", "media", "websocket", "webtransport", "other"
                ]
            }
        }));
    }
    if let Some(config) = locked_app {
        let allowed_hosts = config
            .allowed_hosts
            .into_iter()
            .map(|host| host.trim().trim_start_matches('.').to_lowercase())
            .filter(|host| !host.is_empty())
            .collect::<Vec<_>>();
        if !allowed_hosts.is_empty() {
            rules.push(serde_json::json!({
                "id": rules.len() + 1,
                "priority": 3,
                "action": { "type": "block" },
                "condition": {
                    "regexFilter": "^https?://",
                    "excludedRequestDomains": allowed_hosts,
                    "resourceTypes": [
                        "main_frame", "sub_frame", "stylesheet", "script", "image", "font",
                        "object", "xmlhttprequest", "ping", "media", "websocket", "webtransport", "other"
                    ]
                }
            }));
        }
    }

    fs::write(
        extension_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        extension_dir.join("rules.json"),
        serde_json::to_vec_pretty(&rules)?,
    )?;
    fs::write(extension_dir.join("blocked.html"), chromium_blocked_page_html())?;
    fs::write(extension_dir.join("blocked.css"), chromium_blocked_page_css())?;
    fs::write(extension_dir.join("blocked.js"), chromium_blocked_page_script_js())?;
    fs::write(
        extension_dir.join("service-worker.js"),
        chromium_blocked_page_worker_js(&blocked_domains_for_worker),
    )?;
    Ok(Some(extension_dir))
}

fn chromium_blocked_page_html() -> &'static str {
    r#"<!doctype html>
<html lang="ru">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Cerbena - доступ ограничен</title>
    <link rel="stylesheet" href="./blocked.css" />
  </head>
  <body>
    <main class="blocked-card">
      <p class="badge">Cerbena Policy Firewall</p>
      <h1>Сайт заблокирован политикой профиля</h1>
      <p class="hint">Этот домен находится в denylist профиля. Откройте Cerbena и измените настройки доступа, если это требуется.</p>
      <div class="meta">
        <div>Домен:</div>
        <div id="blocked-host" class="value">-</div>
        <div>URL:</div>
        <div id="blocked-url" class="value">-</div>
      </div>
      <div class="actions">
        <button id="go-back" type="button">Назад</button>
      </div>
    </main>
    <script src="./blocked.js"></script>
  </body>
</html>
"#
}

fn chromium_blocked_page_script_js() -> &'static str {
    r#"(async () => {
  const hostNode = document.getElementById("blocked-host");
  const urlNode = document.getElementById("blocked-url");
  const fallbackFromPage = () => {
    const params = new URLSearchParams(location.search);
    const url = params.get("url") || "";
    const host = params.get("host") || "";
    if (url || host) {
      return { url, host };
    }
    return null;
  };
  const setValues = (url) => {
    try {
      const parsed = new URL(url);
      hostNode.textContent = parsed.host || "-";
      urlNode.textContent = url || "-";
    } catch {
      hostNode.textContent = "-";
      urlNode.textContent = url || "-";
    }
  };
  try {
    let details = fallbackFromPage();
    if (!details?.host && !details?.url) {
      details = await chrome.runtime.sendMessage({ type: "cerbena:get-last-blocked-request" });
    }
    if (!details?.host && !details?.url) {
      await new Promise((resolve) => setTimeout(resolve, 120));
      details = await chrome.runtime.sendMessage({ type: "cerbena:get-last-blocked-request" });
    }
    const blockedUrl = details?.url || "";
    const blockedHost = details?.host || "";
    setValues(blockedUrl);
    if (blockedHost && hostNode.textContent === "-") {
      hostNode.textContent = blockedHost;
    }
    const isServiceLoopHost = blockedHost.toLowerCase() === "cerbena.blocked.local";
    const isServiceLoopUrl = blockedUrl.toLowerCase().includes("cerbena.blocked.local/__cerbena_blocked__");
    if ((blockedHost || blockedUrl) && !isServiceLoopHost && !isServiceLoopUrl) {
      const reportUrl = new URL("http://cerbena.blocked.local/__cerbena_blocked__");
      if (blockedHost) reportUrl.searchParams.set("host", blockedHost);
      if (blockedUrl) reportUrl.searchParams.set("url", blockedUrl);
      fetch(reportUrl.toString(), { method: "GET", cache: "no-store" }).catch(() => {});
    }
  } catch {
    setValues("");
  }
  document.getElementById("go-back")?.addEventListener("click", () => history.back());
})();"#
}

fn chromium_blocked_page_css() -> &'static str {
    r#"* { box-sizing: border-box; }
body {
  margin: 0;
  min-height: 100vh;
  display: grid;
  place-items: center;
  font-family: "Segoe UI", Tahoma, sans-serif;
  background: radial-gradient(circle at 10% 10%, #16243a, #0b1220 60%);
  color: #e5edf8;
}
.blocked-card {
  width: min(760px, calc(100vw - 32px));
  border: 1px solid #2d4469;
  border-radius: 16px;
  padding: 24px;
  background: rgba(13, 20, 34, 0.95);
  box-shadow: 0 22px 60px rgba(0, 0, 0, 0.45);
}
.badge {
  display: inline-block;
  margin: 0 0 10px;
  padding: 5px 10px;
  border-radius: 999px;
  font-size: 12px;
  letter-spacing: .04em;
  color: #d6e6ff;
  background: #1d3557;
}
h1 { margin: 0 0 10px; font-size: 28px; line-height: 1.2; }
.hint { margin: 0 0 18px; color: #c4d3eb; line-height: 1.5; }
.meta {
  display: grid;
  grid-template-columns: 120px 1fr;
  gap: 8px 12px;
  margin-bottom: 20px;
  padding: 14px;
  border-radius: 12px;
  background: #121c2f;
}
.value {
  overflow-wrap: anywhere;
  color: #f4f8ff;
}
.actions { display: flex; justify-content: flex-end; }
button {
  border: 0;
  border-radius: 10px;
  padding: 10px 14px;
  font-weight: 600;
  color: #08203d;
  background: #8bb7ff;
  cursor: pointer;
}
button:hover { background: #a5c9ff; }
"#
}

fn chromium_blocked_page_worker_js(domains: &[String]) -> String {
    let domains_json = serde_json::to_string(domains).unwrap_or_else(|_| "[]".to_string());
    r#"const BLOCKED_DOMAINS = __CERBENA_BLOCKED_DOMAINS__;
let lastBlockedRequestByTab = {};
let lastBlockedRequestGlobal = null;

function rememberBlocked(tabId, url, host) {
  const payload = { url, host, ts: Date.now() };
  if (typeof tabId === "number" && tabId >= 0) {
    lastBlockedRequestByTab[tabId] = payload;
  }
  lastBlockedRequestGlobal = payload;
}

function normalizeHost(value) {
  return String(value || "").trim().toLowerCase().replace(/^\.+/, "");
}

function hostMatches(host, suffix) {
  const h = normalizeHost(host);
  const s = normalizeHost(suffix);
  if (!h || !s) return false;
  return h === s || h.endsWith("." + s);
}

function isBlockedHost(host) {
  return BLOCKED_DOMAINS.some((suffix) => hostMatches(host, suffix));
}

chrome.webRequest.onBeforeRequest.addListener((details) => {
  try {
    const tabId = details?.tabId;
    const url = String(details?.url || "");
    if (typeof tabId !== "number" || tabId < 0 || !url) return;
    const parsed = new URL(url);
    if (!isBlockedHost(parsed.host)) return;
    rememberBlocked(tabId, url, parsed.host);
  } catch {
    // ignore parse errors
  }
}, { urls: ["<all_urls>"] });

chrome.declarativeNetRequest.onRuleMatchedDebug.addListener((event) => {
  const tabId = event?.request?.tabId;
  const url = event?.request?.url;
  if (typeof tabId === "number" && tabId >= 0 && typeof url === "string" && url.length > 0) {
    let host = "";
    try {
      host = new URL(url).host || "";
    } catch {
      host = "";
    }
    if (isBlockedHost(host)) {
      rememberBlocked(tabId, url, host);
    }
  }
});

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message?.type !== "cerbena:get-last-blocked-request") {
    return;
  }
  const tabId = sender?.tab?.id;
  if (typeof tabId === "number" && tabId >= 0) {
    sendResponse(lastBlockedRequestByTab[tabId] || lastBlockedRequestGlobal);
    return;
  }
  sendResponse(lastBlockedRequestGlobal);
});
"#
    .replace("__CERBENA_BLOCKED_DOMAINS__", &domains_json)
}

fn load_profile_browser_defaults(profile_root: &Path) -> (Option<String>, Option<String>) {
    let path = profile_root.join("metadata.json");
    let raw = match fs::read(&path) {
        Ok(value) => value,
        Err(_) => return (None, None),
    };
    let parsed = match serde_json::from_slice::<serde_json::Value>(&raw) {
        Ok(value) => value,
        Err(_) => return (None, None),
    };
    let default_start_page = parsed
        .get("default_start_page")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let default_search_provider = parsed
        .get("default_search_provider")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    (default_start_page, default_search_provider)
}

fn chromium_settings_overrides(
    default_start_page: Option<&str>,
    default_search_provider: Option<&str>,
) -> Option<serde_json::Value> {
    let provider_id = default_search_provider
        .unwrap_or("duckduckgo")
        .trim()
        .to_ascii_lowercase();
    let (name, keyword, search_url, favicon_url, suggestions_url) = match provider_id.as_str() {
        "google" => (
            "Google",
            "google.com",
            "https://www.google.com/search?q={searchTerms}",
            "https://www.google.com/favicon.ico",
            "https://suggestqueries.google.com/complete/search?output=chrome&q={searchTerms}",
        ),
        "bing" => (
            "Bing",
            "bing.com",
            "https://www.bing.com/search?q={searchTerms}",
            "https://www.bing.com/sa/simg/favicon-trans-bg-blue-mg.ico",
            "https://www.bing.com/osjson.aspx?query={searchTerms}",
        ),
        "yandex" => (
            "Yandex",
            "yandex.ru",
            "https://yandex.ru/search/?text={searchTerms}",
            "https://yandex.ru/favicon.ico",
            "https://suggest.yandex.ru/suggest-ff.cgi?part={searchTerms}",
        ),
        "brave" => (
            "Brave",
            "search.brave.com",
            "https://search.brave.com/search?q={searchTerms}",
            "https://search.brave.com/favicon.ico",
            "https://search.brave.com/api/suggest?q={searchTerms}",
        ),
        "ecosia" => (
            "Ecosia",
            "ecosia.org",
            "https://www.ecosia.org/search?q={searchTerms}",
            "https://www.ecosia.org/favicon.ico",
            "https://ac.ecosia.org/autocomplete?q={searchTerms}",
        ),
        "startpage" => (
            "Startpage",
            "startpage.com",
            "https://www.startpage.com/sp/search?query={searchTerms}",
            "https://www.startpage.com/sp/cdn/favicons/favicon.ico",
            "https://www.startpage.com/suggestions?q={searchTerms}",
        ),
        _ => (
            "DuckDuckGo",
            "duckduckgo.com",
            "https://duckduckgo.com/?q={searchTerms}",
            "https://duckduckgo.com/favicon.ico",
            "https://duckduckgo.com/ac/?q={searchTerms}&type=list",
        ),
    };
    let mut overrides = serde_json::json!({
        "search_provider": {
            "name": name,
            "keyword": keyword,
            "search_url": search_url,
            "favicon_url": favicon_url,
            "suggest_url": suggestions_url,
            "encoding": "UTF-8",
            "is_default": true
        }
    });
    if let Some(start_page) = default_start_page
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized = if start_page.contains("://") {
            start_page.to_string()
        } else {
            format!("https://{start_page}")
        };
        overrides["homepage"] = serde_json::Value::String(normalized);
    }
    Some(overrides)
}

pub(super) fn load_locked_app_config_impl(
    profile_root: &Path,
) -> Result<Option<LockedAppConfig>, EngineError> {
    let path = profile_root.join("policy").join("locked-app.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read(path)?;
    let config = serde_json::from_slice::<LockedAppConfig>(&raw)?;
    Ok(Some(config))
}

pub(super) fn resolve_locked_app_target_url_impl(
    config: &LockedAppConfig,
    requested_url: &str,
) -> String {
    let trimmed = requested_url.trim();
    if trimmed.is_empty() {
        return config.start_url.clone();
    }
    let Ok(parsed) = reqwest::Url::parse(trimmed) else {
        return config.start_url.clone();
    };
    let Some(host) = parsed.host_str().map(|value| value.to_ascii_lowercase()) else {
        return config.start_url.clone();
    };
    let allowed = config.allowed_hosts.iter().any(|candidate| {
        let normalized = candidate
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        host == normalized || host.ends_with(&format!(".{normalized}"))
    });
    if allowed {
        trimmed.to_string()
    } else {
        config.start_url.clone()
    }
}

pub(super) fn prepare_chromium_extension_dirs_impl(
    profile_root: &Path,
) -> Result<Vec<PathBuf>, EngineError> {
    let mut dirs = Vec::new();
    if let Some(blocking_extension) = prepare_chromium_blocking_extension_impl(profile_root)? {
        dirs.push(blocking_extension);
    }
    let managed_root = profile_root
        .join("extensions")
        .join("managed")
        .join("chromium-unpacked");
    if managed_root.is_dir() {
        let mut discovered = fs::read_dir(managed_root)?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                if !path.is_dir() {
                    return false;
                }
                let external_marker = path.join(".cerbena-prefer-external-manifest");
                !external_marker.is_file()
            })
            .collect::<Vec<_>>();
        discovered.sort();
        dirs.extend(discovered);
    }
    Ok(dirs)
}

pub(super) fn blocked_domains_for_profile_impl(
    profile_root: &Path,
) -> Result<Vec<String>, EngineError> {
    let path = profile_root.join("policy").join("blocked-domains.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read(path)?;
    let domains: Vec<String> = serde_json::from_slice(&raw)?;
    let normalized = domains
        .into_iter()
        .map(|domain| domain.trim().trim_start_matches('.').to_lowercase())
        .filter(|domain| !domain.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(normalized)
}
