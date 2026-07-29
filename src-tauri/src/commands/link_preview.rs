//! Privacy-preserving link metadata fetching.
//!
//! All network access happens here rather than in the webview. Each redirect is
//! resolved and pinned independently so DNS changes cannot move a request onto a
//! private address after validation.

use base64::Engine;
use reqwest::{header, StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const MAX_REDIRECTS: usize = 5;
const MAX_HTML_BYTES: usize = 512 * 1024;
const MAX_IMAGE_BYTES: usize = 256 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(12);
const DNS_TIMEOUT: Duration = Duration::from_secs(3);
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const CACHE_CAPACITY: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkPreview {
    /// Canonical, normalized HTTP(S) URL after safe redirects.
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    /// A backend-fetched, bounded data URI. Never a third-party network URL.
    pub image_url: Option<String>,
    pub site_name: Option<String>,
}

#[derive(Clone)]
struct CacheEntry {
    inserted_at: Instant,
    preview: LinkPreview,
}

#[derive(Default)]
struct PreviewCache {
    entries: HashMap<String, CacheEntry>,
    order: VecDeque<String>,
}

impl PreviewCache {
    fn get(&mut self, key: &str, now: Instant) -> Option<LinkPreview> {
        let entry = self.entries.get(key)?.clone();
        if now.duration_since(entry.inserted_at) >= CACHE_TTL {
            self.entries.remove(key);
            self.order.retain(|candidate| candidate != key);
            return None;
        }
        self.order.retain(|candidate| candidate != key);
        self.order.push_back(key.to_string());
        Some(entry.preview)
    }

    fn insert(&mut self, key: String, preview: LinkPreview, now: Instant) {
        self.order.retain(|candidate| candidate != &key);
        self.order.push_back(key.clone());
        self.entries.insert(
            key,
            CacheEntry {
                inserted_at: now,
                preview,
            },
        );
        while self.entries.len() > CACHE_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

static PREVIEW_CACHE: OnceLock<Mutex<PreviewCache>> = OnceLock::new();

fn preview_cache() -> &'static Mutex<PreviewCache> {
    PREVIEW_CACHE.get_or_init(|| Mutex::new(PreviewCache::default()))
}

#[tauri::command]
pub async fn fetch_link_preview(url: String) -> Result<LinkPreview, String> {
    let normalized = normalize_url(&url)?;
    let cache_key = normalized.as_str().to_string();
    if let Some(preview) = preview_cache()
        .lock()
        .map_err(|_| "Link preview cache is unavailable".to_string())?
        .get(&cache_key, Instant::now())
    {
        return Ok(preview);
    }

    let preview = tokio::time::timeout(TOTAL_TIMEOUT, fetch_uncached(normalized))
        .await
        .map_err(|_| "Link preview request timed out".to_string())??;

    preview_cache()
        .lock()
        .map_err(|_| "Link preview cache is unavailable".to_string())?
        .insert(cache_key, preview.clone(), Instant::now());
    Ok(preview)
}

async fn fetch_uncached(url: Url) -> Result<LinkPreview, String> {
    let (mut response, final_url) = safe_get(url, None).await?;
    if !response.status().is_success() {
        return Err(format!("Link returned HTTP {}", response.status().as_u16()));
    }

    let site_name = final_url.host_str().map(clean_site_name);
    let content_type = response_content_type(&response);
    if !is_html_content_type(content_type.as_deref()) {
        return Ok(LinkPreview {
            url: final_url.to_string(),
            title: None,
            description: None,
            image_url: None,
            site_name,
        });
    }

    let body = read_limited(&mut response, MAX_HTML_BYTES).await?;
    let (title, description, site_name, canonical_url, image_candidate) = {
        let body = String::from_utf8_lossy(&body);
        let document = scraper::Html::parse_document(&body);
        let title = extract_meta_property(&document, "og:title")
            .or_else(|| extract_title(&document))
            .and_then(|value| sanitize_metadata(&value, 300));
        let description = extract_meta_property(&document, "og:description")
            .or_else(|| extract_meta_name(&document, "description"))
            .and_then(|value| sanitize_metadata(&value, 600));
        let site_name = extract_meta_property(&document, "og:site_name")
            .and_then(|value| sanitize_metadata(&value, 100))
            .or(site_name);
        let canonical_url =
            extract_canonical_url(&document, &final_url).unwrap_or(final_url.clone());
        let image_candidate = extract_meta_property(&document, "og:image")
            .and_then(|candidate| final_url.join(candidate.trim()).ok())
            .and_then(|candidate| normalize_parsed_url(candidate).ok())
            .filter(|candidate| same_origin(candidate, &final_url));
        (
            title,
            description,
            site_name,
            canonical_url,
            image_candidate,
        )
    };
    let image_url = match image_candidate {
        Some(candidate) => fetch_safe_image(candidate, &final_url).await.ok(),
        None => None,
    };

    Ok(LinkPreview {
        url: canonical_url.to_string(),
        title,
        description,
        image_url,
        site_name,
    })
}

async fn safe_get(
    mut url: Url,
    required_origin: Option<&Url>,
) -> Result<(reqwest::Response, Url), String> {
    for redirect_count in 0..=MAX_REDIRECTS {
        if let Some(origin) = required_origin {
            if !same_origin(&url, origin) {
                return Err("Link preview resource redirected to a different site".to_string());
            }
        }
        let addresses = resolve_public_addresses(&url).await?;
        let host = url
            .host_str()
            .ok_or_else(|| "Link URL has no host".to_string())?
            .to_string();
        let client = reqwest::Client::builder()
            .connect_timeout(DNS_TIMEOUT)
            .read_timeout(DNS_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .user_agent("Harbor-Link-Preview/1.0")
            .resolve_to_addrs(&host, &addresses)
            .build()
            .map_err(|_| "Could not initialize the link preview client".to_string())?;
        let response = client
            .get(url.clone())
            .header(
                header::ACCEPT,
                "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1",
            )
            .header(header::CACHE_CONTROL, "no-cache")
            .send()
            .await
            .map_err(|_| "Could not fetch link metadata".to_string())?;

        if !is_redirect(response.status()) {
            return Ok((response, url));
        }
        if redirect_count == MAX_REDIRECTS {
            return Err("Link redirected too many times".to_string());
        }
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| "Link redirect was malformed".to_string())?;
        url = normalize_joined_url(&url, location)?;
    }
    Err("Link redirected too many times".to_string())
}

async fn resolve_public_addresses(url: &Url) -> Result<Vec<SocketAddr>, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "Link URL has no host".to_string())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "Link URL has no port".to_string())?;
    let lookup_host = host.trim_matches(['[', ']']);
    let resolved = tokio::time::timeout(DNS_TIMEOUT, tokio::net::lookup_host((lookup_host, port)))
        .await
        .map_err(|_| "Link hostname lookup timed out".to_string())?
        .map_err(|_| "Link hostname could not be resolved".to_string())?;
    let mut addresses = Vec::new();
    for address in resolved {
        if !is_public_ip(address.ip()) {
            return Err("Link target is not a public Internet address".to_string());
        }
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    if addresses.is_empty() {
        return Err("Link hostname did not resolve".to_string());
    }
    Ok(addresses)
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || ip.is_unspecified()
        || a == 0
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0)
        || (a == 192 && b == 88)
        || (a == 198 && (18..=19).contains(&b))
        || a >= 240)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = ip.segments();
    let first = segments[0];
    let is_documentation = first == 0x2001 && segments[1] == 0x0db8;
    let is_special_2001 = first == 0x2001 && segments[1] <= 0x01ff;
    let is_discard_only = first == 0x0100 && segments[1..].iter().all(|segment| *segment == 0);
    let is_site_local = first & 0xffc0 == 0xfec0;
    let is_six_to_four = first == 0x2002;
    first & 0xe000 == 0x2000
        && !ip.is_loopback()
        && !ip.is_unspecified()
        && !ip.is_multicast()
        && !is_documentation
        && !is_special_2001
        && !is_discard_only
        && !is_site_local
        && !is_six_to_four
}

fn normalize_url(input: &str) -> Result<Url, String> {
    let parsed = Url::parse(input.trim()).map_err(|_| "Invalid link URL".to_string())?;
    normalize_parsed_url(parsed)
}

fn normalize_joined_url(base: &Url, location: &str) -> Result<Url, String> {
    let joined = base
        .join(location.trim())
        .map_err(|_| "Link redirect URL was invalid".to_string())?;
    normalize_parsed_url(joined)
}

fn normalize_parsed_url(mut url: Url) -> Result<Url, String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Only HTTP and HTTPS links can be previewed".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Links containing credentials cannot be previewed".to_string());
    }
    if url.host_str().is_none() {
        return Err("Link URL has no host".to_string());
    }
    url.set_fragment(None);
    if (url.scheme() == "http" && url.port() == Some(80))
        || (url.scheme() == "https" && url.port() == Some(443))
    {
        let _ = url.set_port(None);
    }
    if let Some(ip) = url
        .host_str()
        .and_then(|host| host.trim_matches(['[', ']']).parse::<IpAddr>().ok())
    {
        if !is_public_ip(ip) {
            return Err("Link target is not a public Internet address".to_string());
        }
    }
    Ok(url)
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn is_redirect(status: StatusCode) -> bool {
    matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
}

fn response_content_type(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        })
}

fn is_html_content_type(content_type: Option<&str>) -> bool {
    matches!(content_type, Some("text/html" | "application/xhtml+xml"))
}

async fn read_limited(response: &mut reqwest::Response, limit: usize) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err("Link preview response was too large".to_string());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Could not read link preview response".to_string())?
    {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err("Link preview response was too large".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn fetch_safe_image(url: Url, page_url: &Url) -> Result<String, String> {
    let (mut response, _) = safe_get(url, Some(page_url)).await?;
    if !response.status().is_success() {
        return Err("Preview image request failed".to_string());
    }
    let content_type = response_content_type(&response)
        .filter(|value| is_safe_image_content_type(value))
        .ok_or_else(|| "Preview image type is not supported".to_string())?;
    let bytes = read_limited(&mut response, MAX_IMAGE_BYTES).await?;
    if !image_matches_content_type(&content_type, &bytes) {
        return Err("Preview image contents did not match its declared type".to_string());
    }
    Ok(format!(
        "data:{content_type};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn is_safe_image_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "image/jpeg" | "image/png" | "image/gif" | "image/webp"
    )
}

fn image_matches_content_type(content_type: &str, bytes: &[u8]) -> bool {
    match content_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        _ => false,
    }
}

fn extract_meta_property(document: &scraper::Html, property: &str) -> Option<String> {
    let selector = scraper::Selector::parse("meta").ok()?;
    document
        .select(&selector)
        .find(|element| {
            element
                .value()
                .attr("property")
                .is_some_and(|value| value.eq_ignore_ascii_case(property))
        })
        .and_then(|element| element.value().attr("content"))
        .map(str::to_string)
}

fn extract_meta_name(document: &scraper::Html, name: &str) -> Option<String> {
    let selector = scraper::Selector::parse("meta").ok()?;
    document
        .select(&selector)
        .find(|element| {
            element
                .value()
                .attr("name")
                .is_some_and(|value| value.eq_ignore_ascii_case(name))
        })
        .and_then(|element| element.value().attr("content"))
        .map(str::to_string)
}

fn extract_title(document: &scraper::Html) -> Option<String> {
    let selector = scraper::Selector::parse("title").ok()?;
    document
        .select(&selector)
        .next()
        .map(|element| element.text().collect::<String>())
}

fn extract_canonical_url(document: &scraper::Html, page_url: &Url) -> Option<Url> {
    let selector = scraper::Selector::parse("link").ok()?;
    let candidate = document
        .select(&selector)
        .find(|element| {
            element.value().attr("rel").is_some_and(|rel| {
                rel.split_whitespace()
                    .any(|value| value.eq_ignore_ascii_case("canonical"))
            })
        })?
        .value()
        .attr("href")?;
    let candidate = normalize_joined_url(page_url, candidate).ok()?;
    same_origin(&candidate, page_url).then_some(candidate)
}

fn sanitize_metadata(value: &str, max_chars: usize) -> Option<String> {
    let clean = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .filter(|character| {
            !matches!(
                *character,
                '\u{200b}'..='\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2060}'..='\u{206f}'
                    | '\u{feff}'
            )
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if clean.is_empty() {
        None
    } else {
        Some(clean.chars().take(max_chars).collect())
    }
}

fn clean_site_name(host: &str) -> String {
    host.trim_start_matches("www.").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_public_http_urls_and_rejects_unsafe_schemes_or_credentials() {
        assert_eq!(
            normalize_url(" HTTPS://Example.COM:443/path?q=1#fragment ")
                .unwrap()
                .as_str(),
            "https://example.com/path?q=1"
        );
        assert!(normalize_url("file:///etc/passwd").is_err());
        assert!(normalize_url("javascript:alert(1)").is_err());
        assert!(normalize_url("https://user:secret@example.com/").is_err());
    }

    #[test]
    fn blocks_loopback_private_link_local_metadata_and_encoded_ipv4_targets() {
        for target in [
            "http://127.0.0.1/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://0177.0.0.1/",
            "http://10.0.0.1/",
            "http://172.16.0.1/",
            "http://192.168.0.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://0.0.0.0/",
            "http://100.64.0.1/",
            "http://224.0.0.1/",
        ] {
            assert!(normalize_url(target).is_err(), "accepted {target}");
        }
        assert!(normalize_url("https://8.8.8.8/").is_ok());
    }

    #[test]
    fn blocks_non_public_ipv6_and_ipv4_mapped_targets() {
        for target in [
            "http://[::1]/",
            "http://[::]/",
            "http://[fe80::1]/",
            "http://[fc00::1]/",
            "http://[ff02::1]/",
            "http://[2001:db8::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:169.254.169.254]/",
        ] {
            assert!(normalize_url(target).is_err(), "accepted {target}");
        }
        assert!(normalize_url("https://[2606:4700:4700::1111]/").is_ok());
    }

    #[test]
    fn extracts_and_bounds_well_formed_metadata() {
        let html = scraper::Html::parse_document(
            r#"<html><head>
              <meta property="og:title" content="  A   useful\n title  ">
              <meta property="og:description" content="Description">
              <meta property="og:site_name" content="Example Site">
              <meta property="og:image" content="/preview.png">
              <link rel="canonical" href="/canonical#tracking">
            </head></html>"#,
        );
        let page = normalize_url("https://example.com/post").unwrap();
        assert_eq!(
            sanitize_metadata(&extract_meta_property(&html, "og:title").unwrap(), 300),
            Some("A useful\\n title".to_string())
        );
        assert_eq!(
            extract_canonical_url(&html, &page).unwrap().as_str(),
            "https://example.com/canonical"
        );
        assert_eq!(sanitize_metadata(&"x".repeat(400), 300).unwrap().len(), 300);
        assert_eq!(
            sanitize_metadata("first\nsecond\u{202e}third", 100),
            Some("first secondthird".to_string())
        );
    }

    #[test]
    fn refuses_cross_origin_canonical_links() {
        let html = scraper::Html::parse_document(
            r#"<link rel="canonical" href="https://tracker.example/collect">"#,
        );
        let page = normalize_url("https://example.com/post").unwrap();
        assert!(extract_canonical_url(&html, &page).is_none());
    }

    #[test]
    fn rejects_redirects_to_local_or_non_http_targets_before_requesting_them() {
        let page = normalize_url("https://example.com/post").unwrap();
        assert!(normalize_joined_url(&page, "http://169.254.169.254/latest/meta-data").is_err());
        assert!(normalize_joined_url(&page, "file:///etc/passwd").is_err());
        assert_eq!(
            normalize_joined_url(&page, "/safe#fragment")
                .unwrap()
                .as_str(),
            "https://example.com/safe"
        );
    }

    #[tokio::test]
    async fn rejects_hostnames_that_resolve_to_non_public_addresses() {
        let local = normalize_url("http://localhost/").unwrap();
        assert!(resolve_public_addresses(&local).await.is_err());
    }

    #[test]
    fn cache_is_bounded_and_expires_entries() {
        let now = Instant::now();
        let preview = LinkPreview {
            url: "https://example.com/".to_string(),
            title: None,
            description: None,
            image_url: None,
            site_name: Some("example.com".to_string()),
        };
        let mut cache = PreviewCache::default();
        for index in 0..=CACHE_CAPACITY {
            cache.insert(format!("key-{index}"), preview.clone(), now);
        }
        assert_eq!(cache.entries.len(), CACHE_CAPACITY);
        assert!(cache.get("key-0", now).is_none());
        assert!(cache
            .get("key-1", now + CACHE_TTL + Duration::from_secs(1))
            .is_none());
    }

    #[test]
    fn only_accepts_expected_html_and_raster_image_types() {
        assert!(is_html_content_type(Some("text/html")));
        assert!(is_html_content_type(Some("application/xhtml+xml")));
        assert!(!is_html_content_type(Some("application/json")));
        assert!(!is_html_content_type(Some("image/svg+xml")));
        assert!(is_safe_image_content_type("image/png"));
        assert!(!is_safe_image_content_type("image/svg+xml"));
        assert!(!is_safe_image_content_type("text/html"));
        assert!(image_matches_content_type(
            "image/png",
            b"\x89PNG\r\n\x1a\nrest"
        ));
        assert!(!image_matches_content_type("image/png", b"<script>"));
        assert!(!image_matches_content_type("image/svg+xml", b"<svg>"));
    }
}
