//! RSS feed generation commands for Wall posts

use crate::db::repositories::{PostVisibility, PostsRepository};
use crate::db::Database;
use crate::error::{AppError, Result};
use crate::services::IdentityService;
use std::sync::Arc;
use tauri::State;

/// RSS feed configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RssFeedConfig {
    /// Base URL for the feed (e.g., "harbor://peer/12D3KooW...")
    pub base_url: String,
    /// Feed title
    pub title: String,
    /// Feed description
    pub description: String,
    /// Maximum number of items
    pub max_items: usize,
}

impl Default for RssFeedConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            title: "Harbor Wall".to_string(),
            description: "Posts from my Harbor wall".to_string(),
            max_items: 50,
        }
    }
}

/// RSS feed item
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RssItem {
    pub title: String,
    pub link: String,
    pub description: String,
    pub pub_date: String,
    pub guid: String,
}

/// Generate RSS 2.0 XML for the current user's public wall posts
#[tauri::command]
pub async fn generate_rss_feed(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
    config: Option<RssFeedConfig>,
) -> Result<String> {
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;

    let config = config.unwrap_or_else(|| RssFeedConfig {
        base_url: format!("harbor://peer/{}", identity.peer_id),
        title: format!("{}'s Wall", identity.display_name),
        ..Default::default()
    });

    let public_posts = get_public_rss_posts(&db, &identity.peer_id, config.max_items)?;

    // Generate RSS XML
    let rss_xml = generate_rss_xml(&config, &public_posts, &identity.peer_id);

    Ok(rss_xml)
}

/// Generate RSS feed for a specific peer's public posts (for viewing others' feeds)
#[tauri::command]
pub async fn get_peer_rss_feed(
    db: State<'_, Arc<Database>>,
    peer_id: String,
    max_items: Option<usize>,
) -> Result<String> {
    let max_items = max_items.unwrap_or(50);

    let public_posts = get_public_rss_posts(&db, &peer_id, max_items)?;

    let config = RssFeedConfig {
        base_url: format!("harbor://peer/{}", peer_id),
        title: format!("Harbor Wall - {}", &peer_id[..12.min(peer_id.len())]),
        description: "Public posts from a Harbor user".to_string(),
        max_items,
    };

    let rss_xml = generate_rss_xml(&config, &public_posts, &peer_id);

    Ok(rss_xml)
}

fn get_public_rss_posts(
    db: &Database,
    peer_id: &str,
    max_items: usize,
) -> Result<Vec<crate::db::repositories::Post>> {
    PostsRepository::get_by_author_with_visibility(
        db,
        peer_id,
        Some(PostVisibility::Public),
        max_items as i64,
        None,
    )
    .map_err(|e| AppError::DatabaseString(e.to_string()))
}

/// Get RSS feed URL for sharing
#[tauri::command]
pub async fn get_rss_feed_url(identity_service: State<'_, Arc<IdentityService>>) -> Result<String> {
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;

    // Return a shareable RSS feed URL
    Ok(format!("harbor://feed/{}", identity.peer_id))
}

/// Generate RSS 2.0 XML from posts
fn generate_rss_xml(
    config: &RssFeedConfig,
    posts: &[crate::db::repositories::Post],
    peer_id: &str,
) -> String {
    let now = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S GMT")
        .to_string();

    let items: Vec<String> = posts
        .iter()
        .map(|post| {
            let title = post
                .content_text
                .as_ref()
                .map(|t| {
                    // Use first line or first 100 chars as title
                    let first_line = t.lines().next().unwrap_or(t);
                    if first_line.len() > 100 {
                        format!("{}...", &first_line[..100])
                    } else {
                        first_line.to_string()
                    }
                })
                .unwrap_or_else(|| format!("{} post", post.content_type));

            let description = post
                .content_text
                .as_ref()
                .map(|t| xml_escape(t))
                .unwrap_or_default();

            let pub_date = chrono::DateTime::from_timestamp(post.created_at, 0)
                .map(|dt| dt.format("%a, %d %b %Y %H:%M:%S GMT").to_string())
                .unwrap_or_default();

            let link = format!("{}/post/{}", config.base_url, post.post_id);
            let guid = format!("harbor:post:{}:{}", peer_id, post.post_id);

            format!(
                r#"    <item>
      <title>{}</title>
      <link>{}</link>
      <description>{}</description>
      <pubDate>{}</pubDate>
      <guid isPermaLink="false">{}</guid>
    </item>"#,
                xml_escape(&title),
                xml_escape(&link),
                xml_escape(&description),
                pub_date,
                xml_escape(&guid)
            )
        })
        .collect();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>{}</title>
    <link>{}</link>
    <description>{}</description>
    <language>en-us</language>
    <lastBuildDate>{}</lastBuildDate>
    <atom:link href="{}/feed.xml" rel="self" type="application/rss+xml"/>
{}
  </channel>
</rss>"#,
        xml_escape(&config.title),
        xml_escape(&config.base_url),
        xml_escape(&config.description),
        now,
        xml_escape(&config.base_url),
        items.join("\n")
    )
}

/// Escape special XML characters
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::PostData;

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("Hello & World"), "Hello &amp; World");
        assert_eq!(xml_escape("<script>"), "&lt;script&gt;");
        assert_eq!(xml_escape("\"quotes\""), "&quot;quotes&quot;");
    }

    #[test]
    fn test_rss_feed_generation() {
        let config = RssFeedConfig {
            base_url: "harbor://peer/test123".to_string(),
            title: "Test Feed".to_string(),
            description: "A test feed".to_string(),
            max_items: 10,
        };

        let posts = vec![];
        let xml = generate_rss_xml(&config, &posts, "test123");

        assert!(xml.contains("<?xml version=\"1.0\""));
        assert!(xml.contains("<rss version=\"2.0\""));
        assert!(xml.contains("<title>Test Feed</title>"));
        assert!(xml.contains("<channel>"));
    }

    #[test]
    fn test_get_public_rss_posts_filters_contacts_only_in_sql() {
        let db = Database::in_memory().unwrap();
        let peer_id = "peer-rss";
        PostsRepository::insert_post(
            &db,
            &PostData {
                post_id: "contacts-new".to_string(),
                author_peer_id: peer_id.to_string(),
                content_type: "text".to_string(),
                content_text: Some("Contacts only".to_string()),
                visibility: PostVisibility::Contacts,
                lamport_clock: 2,
                created_at: 2000,
                signature: vec![0u8; 64],
            },
        )
        .unwrap();
        PostsRepository::insert_post(
            &db,
            &PostData {
                post_id: "public-old".to_string(),
                author_peer_id: peer_id.to_string(),
                content_type: "text".to_string(),
                content_text: Some("Public".to_string()),
                visibility: PostVisibility::Public,
                lamport_clock: 1,
                created_at: 1000,
                signature: vec![0u8; 64],
            },
        )
        .unwrap();

        let posts = get_public_rss_posts(&db, peer_id, 1).unwrap();

        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].post_id, "public-old");
    }
}
