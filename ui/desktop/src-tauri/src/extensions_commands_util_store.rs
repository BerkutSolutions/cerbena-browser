use super::*;

pub(crate) fn download_store_extension_metadata(store_url: &str) -> Result<DerivedExtensionMetadata, String> {
    let lower = store_url.to_lowercase();
    if lower.contains("addons.mozilla.org") {
        return download_amo_extension_metadata(store_url);
    }
    if lower.contains("chromewebstore.google.com") || lower.contains("chrome.google.com") {
        return download_chrome_extension_metadata(store_url);
    }
    Err("unsupported store URL".to_string())
}

pub(crate) fn download_amo_extension_metadata(store_url: &str) -> Result<DerivedExtensionMetadata, String> {
    let client = extension_http_client()?;
    let slug = extract_amo_slug(store_url).ok_or_else(|| "unsupported AMO URL".to_string())?;
    let api_url = format!("https://addons.mozilla.org/api/v5/addons/addon/{slug}/");
    let details = download_json(&client, &api_url)?;
    let file_url = details
        .get("current_version")
        .and_then(|value| value.get("file"))
        .and_then(|value| value.get("url"))
        .and_then(|value| value.as_str())
        .or_else(|| {
            details
                .get("current_version")
                .and_then(|value| value.get("files"))
                .and_then(|value| value.as_array())
                .and_then(|items| items.first())
                .and_then(|value| value.get("url"))
                .and_then(|value| value.as_str())
        })
        .ok_or_else(|| "AMO package download URL not found".to_string())?;
    let file_name = file_name_from_url(file_url).unwrap_or_else(|| format!("{slug}.xpi"));
    let package_bytes = download_binary(&client, file_url)?;
    let mut metadata = super::archive_io::read_extension_archive_metadata_from_bytes(
        &package_bytes,
        &file_name,
        Some(store_url),
    )?;
    metadata.stable_id = Some(slug);
    metadata.engine_scope = Some("firefox".to_string());
    if metadata.display_name.is_none() {
        metadata.display_name = localized_json_value(details.get("name"));
    }
    if metadata.version.is_none() {
        metadata.version = details
            .get("current_version")
            .and_then(|value| value.get("version"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());
    }
    if metadata.logo_url.is_none() {
        metadata.logo_url = amo_icon_data_url(&client, &details).or_else(|| {
            details
                .get("icon_url")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
    }
    Ok(metadata)
}

pub(crate) fn download_chrome_extension_metadata(store_url: &str) -> Result<DerivedExtensionMetadata, String> {
    let client = extension_http_client()?;
    let extension_id = extract_chrome_web_store_id(store_url)
        .ok_or_else(|| "unsupported Chrome Web Store URL".to_string())?;
    let file_name = format!("{extension_id}.crx");
    let package_bytes =
        download_binary(&client, &build_chrome_web_store_download_url(&extension_id))?;
    let mut metadata = super::archive_io::read_extension_archive_metadata_from_bytes(
        &package_bytes,
        &file_name,
        Some(store_url),
    )?;
    metadata.stable_id = Some(extension_id);
    metadata.engine_scope = Some("chromium".to_string());
    if metadata.display_name.is_none() || metadata.logo_url.is_none() {
        if let Ok(page_html) = download_text(&client, store_url) {
            if metadata.display_name.is_none() {
                metadata.display_name = parse_html_meta_content(&page_html, "og:title")
                    .or_else(|| parse_html_title(&page_html));
            }
            if metadata.logo_url.is_none() {
                metadata.logo_url = parse_html_meta_content(&page_html, "og:image")
                    .and_then(|url| download_data_url(&client, &url).or(Some(url)));
            }
        }
    }
    Ok(metadata)
}

pub(crate) fn extension_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Cerbena/"
                .to_string()
                + env!("CARGO_PKG_VERSION"),
        )
        .build()
        .map_err(|e| format!("extension http client: {e}"))
}

pub(crate) fn download_binary(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("download {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("download {url}: http {}", response.status()));
    }
    response
        .bytes()
        .map(|value| value.to_vec())
        .map_err(|e| e.to_string())
}

pub(crate) fn download_text(client: &Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("download {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("download {url}: http {}", response.status()));
    }
    response.text().map_err(|e| format!("download {url}: {e}"))
}

pub(crate) fn download_json(client: &Client, url: &str) -> Result<serde_json::Value, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("download {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("download {url}: http {}", response.status()));
    }
    response
        .json::<serde_json::Value>()
        .map_err(|e| format!("parse {url}: {e}"))
}

pub(crate) fn download_data_url(client: &Client, url: &str) -> Option<String> {
    let bytes = download_binary(client, url).ok()?;
    let mime = guess_mime_from_url(url);
    Some(format!("data:{mime};base64,{}", BASE64_STANDARD.encode(bytes)))
}

pub(crate) fn guess_mime_from_url(url: &str) -> &'static str {
    let lower = url.to_lowercase();
    if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/png"
    }
}

pub(crate) fn localized_json_value(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value.as_str() {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        return Some(text.to_string());
    }
    if let Some(map) = value.as_object() {
        for key in ["en-US", "en_US", "en", "ru", "default"] {
            if let Some(text) = map.get(key).and_then(|item| item.as_str()) {
                let text = text.trim();
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn amo_icon_data_url(client: &Client, details: &serde_json::Value) -> Option<String> {
    let icons = details.get("icons")?;
    let icon_url = icons
        .as_object()?
        .iter()
        .filter_map(|(size, value)| Some((size.parse::<u32>().ok()?, value.as_str()?)))
        .max_by_key(|(size, _)| *size)
        .map(|(_, value)| value.to_string())?;
    download_data_url(client, &icon_url).or(Some(icon_url))
}

pub(crate) fn extract_amo_slug(url: &str) -> Option<String> {
    let marker = "/addon/";
    let lower = url.to_lowercase();
    let index = lower.find(marker)?;
    let tail = &url[index + marker.len()..];
    let slug = tail
        .split(['/', '?', '#'])
        .find(|segment| !segment.trim().is_empty())?;
    Some(slug.trim().to_string())
}

pub(crate) fn extract_chrome_web_store_id(url: &str) -> Option<String> {
    let marker = "/detail/";
    let lower = url.to_lowercase();
    let detail_index = lower.find(marker)?;
    let tail = &url[detail_index + marker.len()..];
    let mut segments = tail.split('/');
    let _slug = segments.next()?;
    let extension_id = segments
        .next()
        .map(str::trim)
        .filter(|value| value.len() == 32)?;
    Some(extension_id.to_string())
}

pub(crate) fn build_chrome_web_store_download_url(extension_id: &str) -> String {
    format!(
        "https://clients2.google.com/service/update2/crx?response=redirect&prodversion=131.0.0.0&acceptformat=crx2,crx3&x=id%3D{extension_id}%26installsource%3Dondemand%26uc"
    )
}

pub(crate) fn file_name_from_url(url: &str) -> Option<String> {
    let path = url.split('?').next().unwrap_or(url);
    let segment = path
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(segment.to_string())
}

pub(crate) fn parse_html_meta_content(html: &str, marker: &str) -> Option<String> {
    let marker = format!("property=\"{marker}\"");
    let index = html.find(&marker)?;
    let fragment = &html[index..];
    extract_html_attribute(fragment, "content")
}

pub(crate) fn parse_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title>")?;
    let rest = &html[start + "<title>".len()..];
    let end = rest.to_lowercase().find("</title>")?;
    let title = html_entity_decode(rest[..end].trim());
    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}

pub(crate) fn extract_html_attribute(fragment: &str, attr: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let marker = format!("{attr}={quote}");
        let start = fragment.find(&marker)? + marker.len();
        let tail = &fragment[start..];
        let end = tail.find(quote)?;
        let value = html_entity_decode(tail[..end].trim());
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

pub(crate) fn html_entity_decode(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}
