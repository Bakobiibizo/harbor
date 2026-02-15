//! Link preview fetching command
//!
//! Fetches Open Graph metadata from a URL for generating preview cards.

use serde::{Deserialize, Serialize};

/// Open Graph metadata extracted from a web page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkPreview {
    /// The original URL that was fetched
    pub url: String,
    /// og:title or <title> fallback
    pub title: Option<String>,
    /// og:description or <meta name="description"> fallback
    pub description: Option<String>,
    /// og:image URL
    pub image_url: Option<String>,
    /// og:site_name
    pub site_name: Option<String>,
}

/// Fetch Open Graph metadata from a URL and return a LinkPreview.
///
/// This runs the HTTP request from the Rust backend to avoid CSP issues
/// in the frontend webview.
#[tauri::command]
pub async fn fetch_link_preview(url: String) -> Result<LinkPreview, String> {
    // Validate the URL
    let parsed = reqwest::Url::parse(&url).map_err(|e| format!("Invalid URL: {}", e))?;

    // Only allow http and https schemes
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => return Err(format!("Unsupported URL scheme: {}", scheme)),
    }

    // Build an HTTP client with a reasonable timeout and a browser-like user-agent
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (compatible; HarborBot/1.0)")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Fetch the page
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch URL: {}", e))?;

    // Check status
    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    // Only process HTML responses
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.contains("text/html") && !content_type.contains("application/xhtml") {
        // Return a minimal preview for non-HTML content
        return Ok(LinkPreview {
            url,
            title: None,
            description: None,
            image_url: None,
            site_name: parsed.host_str().map(String::from),
        });
    }

    // Read the body (limit to 512KB to avoid memory issues)
    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    let body = if body.len() > 512 * 1024 {
        body[..512 * 1024].to_string()
    } else {
        body
    };

    // Parse HTML with scraper
    let document = scraper::Html::parse_document(&body);

    // Extract Open Graph metadata
    let og_title = extract_meta_property(&document, "og:title");
    let og_description = extract_meta_property(&document, "og:description");
    let og_image = extract_meta_property(&document, "og:image");
    let og_site_name = extract_meta_property(&document, "og:site_name");

    // Fallbacks
    let title = og_title.or_else(|| extract_title(&document));
    let description = og_description.or_else(|| extract_meta_name(&document, "description"));
    let site_name = og_site_name.or_else(|| parsed.host_str().map(String::from));

    // Resolve relative image URLs to absolute
    let image_url = og_image.and_then(|img| {
        if img.starts_with("http://") || img.starts_with("https://") {
            Some(img)
        } else if img.starts_with("//") {
            Some(format!("https:{}", img))
        } else {
            // Try to resolve as relative URL
            parsed.join(&img).ok().map(|u| u.to_string())
        }
    });

    Ok(LinkPreview {
        url,
        title,
        description,
        image_url,
        site_name,
    })
}

/// Extract content from <meta property="..." content="..."> tags
fn extract_meta_property(document: &scraper::Html, property: &str) -> Option<String> {
    let selector = scraper::Selector::parse(&format!("meta[property=\"{}\"]", property)).ok()?;
    document
        .select(&selector)
        .next()
        .and_then(|el| el.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Extract content from <meta name="..." content="..."> tags
fn extract_meta_name(document: &scraper::Html, name: &str) -> Option<String> {
    let selector = scraper::Selector::parse(&format!("meta[name=\"{}\"]", name)).ok()?;
    document
        .select(&selector)
        .next()
        .and_then(|el| el.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Extract the <title> tag content
fn extract_title(document: &scraper::Html) -> Option<String> {
    let selector = scraper::Selector::parse("title").ok()?;
    document
        .select(&selector)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_preview_serialization() {
        let preview = LinkPreview {
            url: "https://example.com".to_string(),
            title: Some("Example".to_string()),
            description: Some("An example website".to_string()),
            image_url: Some("https://example.com/image.png".to_string()),
            site_name: Some("Example".to_string()),
        };

        let json = serde_json::to_string(&preview).unwrap();
        assert!(json.contains("Example"));
        assert!(json.contains("example.com"));
    }
}
