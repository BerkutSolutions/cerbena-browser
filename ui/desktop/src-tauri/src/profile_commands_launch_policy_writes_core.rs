use super::*;

pub(crate) fn apply_librewolf_website_filter_impl(
    state: &State<'_, AppState>,
    profile_id: &Uuid,
    binary_path: &std::path::Path,
) -> Result<(), String> {
    let Some(engine_dir) = binary_path.parent() else {
        return Ok(());
    };
    let distribution_dir = engine_dir.join("distribution");
    fs::create_dir_all(&distribution_dir)
        .map_err(|error| format!("create LibreWolf distribution dir: {error}"))?;
    write_firefox_search_plugin_bundle_impl(&distribution_dir)
        .map_err(|error| format!("write Firefox search plugin bundle: {error}"))?;

    let mut blocked_domains: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    if let Ok(store) = state.network_store.lock() {
        if let Some(dns) = store.dns.get(&profile_id.to_string()) {
            for domain in &dns.domain_denylist {
                let trimmed = domain.trim().to_lowercase();
                if !trimmed.is_empty() {
                    blocked_domains.insert(trimmed);
                }
            }
            for (_, service) in &dns.selected_services {
                for domain in service_domain_seeds(service) {
                    blocked_domains.insert(domain.to_string());
                }
            }
            for list in &dns.selected_blocklists {
                for domain in &list.domains {
                    let trimmed = domain
                        .trim()
                        .trim_start_matches("*.")
                        .trim_start_matches('.')
                        .to_lowercase();
                    if !trimmed.is_empty() {
                        blocked_domains.insert(trimmed);
                    }
                }
            }
        }
    }
    for domain in global_active_blocklist_domains_impl(state) {
        blocked_domains.insert(domain);
    }
    for suffix in global_domain_suffixes_impl(state) {
        blocked_domains.insert(suffix);
    }

    let block_entries: Vec<String> = blocked_domains
        .into_iter()
        .flat_map(|domain| vec![format!("*://{domain}/*"), format!("*://*.{domain}/*")])
        .collect();
    let mut cert_paths: Vec<String> = Vec::new();
    if let Ok(manager) = state.manager.lock() {
        if let Ok(profile) = manager.get_profile(*profile_id) {
            cert_paths.extend(prepare_librewolf_profile_certificates_for_state(
                state.inner(),
                *profile_id,
                &profile.tags,
            )?);
        }
    }
    cert_paths.sort();
    cert_paths.dedup();
    let default_search_engine = state
        .manager
        .lock()
        .ok()
        .and_then(|manager| manager.get_profile(*profile_id).ok())
        .and_then(|profile| {
            map_search_provider_to_firefox_engine_impl(profile.default_search_provider.as_deref())
                .map(ToString::to_string)
        });
    let mut search_engines_policy = serde_json::json!({
        "Add": firefox_search_engine_policy_entries_impl(),
        "PreventInstalls": true
    });
    if let Some(default_search_engine) = default_search_engine {
        search_engines_policy["Default"] = serde_json::Value::String(default_search_engine);
    }
    let policy = serde_json::json!({
        "policies": {
            "WebsiteFilter": {
                "Block": block_entries,
                "Exceptions": []
            },
            "Certificates": {
                "Install": cert_paths
            },
            "SearchEngines": search_engines_policy,
            "OverrideFirstRunPage": "",
            "OverridePostUpdatePage": ""
        }
    });
    fs::write(
        distribution_dir.join("policies.json"),
        serde_json::to_vec_pretty(&policy).unwrap_or_default(),
    )
    .map_err(|error| format!("write LibreWolf policies.json: {error}"))?;
    Ok(())
}

pub(crate) fn write_profile_blocked_domains_impl(
    state: &State<'_, AppState>,
    profile_id: &Uuid,
    profile_root: &std::path::Path,
) -> Result<(), std::io::Error> {
    let mut blocked_domains: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    if let Ok(store) = state.network_store.lock() {
        if let Some(dns) = store.dns.get(&profile_id.to_string()) {
            for domain in &dns.domain_denylist {
                let trimmed = domain.trim().to_lowercase();
                if !trimmed.is_empty() {
                    blocked_domains.insert(trimmed);
                }
            }
            for (_, service) in &dns.selected_services {
                for domain in service_domain_seeds(service) {
                    blocked_domains.insert(domain.to_string());
                }
            }
            for list in &dns.selected_blocklists {
                for domain in &list.domains {
                    let trimmed = domain
                        .trim()
                        .trim_start_matches("*.")
                        .trim_start_matches('.')
                        .to_lowercase();
                    if !trimmed.is_empty() {
                        blocked_domains.insert(trimmed);
                    }
                }
            }
        }
    }
    for domain in global_active_blocklist_domains_impl(state) {
        blocked_domains.insert(domain);
    }
    for suffix in global_domain_suffixes_impl(state) {
        blocked_domains.insert(suffix);
    }

    let policy_dir = profile_root.join("policy");
    fs::create_dir_all(&policy_dir)?;
    fs::write(
        policy_dir.join("blocked-domains.json"),
        serde_json::to_vec(&blocked_domains.into_iter().collect::<Vec<_>>()).unwrap_or_default(),
    )?;
    Ok(())
}

pub(crate) fn global_domain_suffixes_impl(state: &State<'_, AppState>) -> Vec<String> {
    load_global_security_record(state)
        .map(|record| {
            record
                .blocked_domain_suffixes
                .into_iter()
                .map(|value| {
                    value
                        .trim()
                        .trim_start_matches("*.")
                        .trim_start_matches('.')
                        .to_string()
                })
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn global_active_blocklist_domains_impl(state: &State<'_, AppState>) -> Vec<String> {
    load_global_security_record(state)
        .map(|record| {
            let mut domains = std::collections::BTreeSet::new();
            for item in record.blocklists {
                if !item.active {
                    continue;
                }
                for domain in item.domains {
                    let trimmed = domain
                        .trim()
                        .trim_start_matches("*.")
                        .trim_start_matches('.')
                        .to_lowercase();
                    if !trimmed.is_empty() {
                        domains.insert(trimmed);
                    }
                }
            }
            domains.into_iter().collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn neutralize_librewolf_builtin_theme_impl(
    binary_path: &std::path::Path,
) -> Result<(), std::io::Error> {
    let Some(engine_dir) = binary_path.parent() else {
        return Ok(());
    };
    let chrome_css = engine_dir.join("chrome.css");
    if !chrome_css.exists() {
        return Ok(());
    }

    let backup = engine_dir.join("chrome.css.launcher-backup");
    if !backup.exists() {
        fs::copy(&chrome_css, &backup)?;
    }

    let current = fs::read_to_string(&chrome_css).unwrap_or_default();
    if current.contains("launcher-neutralized") {
        return Ok(());
    }

    fs::write(
        &chrome_css,
        "/* launcher-neutralized: restore default Firefox chrome UI */\n",
    )?;
    eprintln!(
        "[profile-launch] librewolf builtin chrome.css neutralized path={}",
        chrome_css.display()
    );
    Ok(())
}

pub(crate) fn firefox_search_engine_policy_entries_impl() -> Vec<serde_json::Value> {
    vec![
        firefox_search_engine_entry_impl(
            "DuckDuckGo",
            "https://duckduckgo.com/?q={searchTerms}",
            Some("https://duckduckgo.com/ac/?q={searchTerms}&type=list"),
        ),
        firefox_search_engine_entry_impl(
            "Google",
            "https://www.google.com/search?q={searchTerms}",
            Some(
                "https://suggestqueries.google.com/complete/search?output=firefox&q={searchTerms}",
            ),
        ),
        firefox_search_engine_entry_impl(
            "Bing",
            "https://www.bing.com/search?q={searchTerms}",
            Some("https://www.bing.com/osjson.aspx?query={searchTerms}"),
        ),
        firefox_search_engine_entry_impl(
            "Yandex",
            "https://yandex.com/search/?text={searchTerms}",
            Some("https://suggest.yandex.com/suggest-ff.cgi?part={searchTerms}"),
        ),
        firefox_search_engine_entry_impl(
            "Brave",
            "https://search.brave.com/search?q={searchTerms}",
            Some("https://search.brave.com/api/suggest?q={searchTerms}"),
        ),
        firefox_search_engine_entry_impl(
            "Ecosia",
            "https://www.ecosia.org/search?q={searchTerms}",
            Some("https://ac.ecosia.org/autocomplete?q={searchTerms}"),
        ),
        firefox_search_engine_entry_impl(
            "Startpage",
            "https://www.startpage.com/sp/search?query={searchTerms}",
            Some("https://www.startpage.com/suggestions?q={searchTerms}"),
        ),
    ]
}

pub(crate) fn build_firefox_search_plugin_xml_impl(
    name: &str,
    url_template: &str,
    suggest_template: Option<&str>,
) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/">
  <ShortName>{name}</ShortName>
  <Description>{name}</Description>
  <InputEncoding>UTF-8</InputEncoding>
  <Url type="text/html" method="GET" template="{url_template}"/>
"#
    );
    if let Some(suggest_template) = suggest_template {
        xml.push_str(&format!(
            "  <Url type=\"application/x-suggestions+json\" method=\"GET\" template=\"{suggest_template}\"/>\n"
        ));
    }
    xml.push_str("</OpenSearchDescription>\n");
    xml
}

fn firefox_search_engine_entry_impl(
    name: &str,
    url_template: &str,
    suggest_url_template: Option<&str>,
) -> serde_json::Value {
    let mut entry = serde_json::json!({
        "Name": name,
        "URLTemplate": url_template,
    });
    if let Some(suggest_url_template) = suggest_url_template {
        entry["SuggestURLTemplate"] = serde_json::Value::String(suggest_url_template.to_string());
    }
    entry
}

fn firefox_search_engine_catalog_impl() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
)> {
    vec![
        (
            "duckduckgo",
            "DuckDuckGo",
            "https://duckduckgo.com/?q={searchTerms}",
            Some("https://duckduckgo.com/ac/?q={searchTerms}&type=list"),
        ),
        (
            "google",
            "Google",
            "https://www.google.com/search?q={searchTerms}",
            Some(
                "https://suggestqueries.google.com/complete/search?output=firefox&q={searchTerms}",
            ),
        ),
        (
            "bing",
            "Bing",
            "https://www.bing.com/search?q={searchTerms}",
            Some("https://www.bing.com/osjson.aspx?query={searchTerms}"),
        ),
        (
            "yandex",
            "Yandex",
            "https://yandex.com/search/?text={searchTerms}",
            Some("https://suggest.yandex.com/suggest-ff.cgi?part={searchTerms}"),
        ),
        (
            "brave",
            "Brave",
            "https://search.brave.com/search?q={searchTerms}",
            Some("https://search.brave.com/api/suggest?q={searchTerms}"),
        ),
        (
            "ecosia",
            "Ecosia",
            "https://www.ecosia.org/search?q={searchTerms}",
            Some("https://ac.ecosia.org/autocomplete?q={searchTerms}"),
        ),
        (
            "startpage",
            "Startpage",
            "https://www.startpage.com/sp/search?query={searchTerms}",
            Some("https://www.startpage.com/suggestions?q={searchTerms}"),
        ),
    ]
}

fn write_firefox_search_plugin_bundle_impl(distribution_dir: &Path) -> Result<(), std::io::Error> {
    let searchplugins_dir = distribution_dir.join("searchplugins").join("common");
    fs::create_dir_all(&searchplugins_dir)?;
    for (id, name, url_template, suggest_template) in firefox_search_engine_catalog_impl() {
        let xml = build_firefox_search_plugin_xml_impl(name, url_template, suggest_template);
        fs::write(searchplugins_dir.join(format!("{id}.xml")), xml)?;
    }
    Ok(())
}
