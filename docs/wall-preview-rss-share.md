# Wall preview, RSS, and share surfaces

Harbor exposes wall preview and sharing controls from **Wall → Preview and share your wall**.

## Preview modes

Preview modes call the production Tauri backend command `get_wall_preview` and display the rows returned by SQLite, rather than filtering mock data in the UI.

- **Guest preview** uses `perspective: "guest"` and shows only posts marked `public`.
- **Contact preview** uses `perspective: "contact"` and shows posts visible to contacts with `WallRead`: `public` plus `contacts` posts.
- **Owner preview** uses `perspective: "owner"` and shows every non-deleted local wall post regardless of visibility.

The panel also calls `get_wall_visibility_stats` so authors can see how many total, public, and contacts-only posts exist before sharing.

## RSS export

The **Copy RSS XML** and **Export .xml** actions call `generate_rss_feed`. The backend RSS path queries only `PostVisibility::Public`, so contacts-only posts are excluded before XML reaches the UI.

RSS is generated locally. Harbor does **not** currently host RSS over HTTP, so the UI exposes RSS as copied/exported XML and does not present it as a network-hosted feed URL.

## Share links

- **Copy public feed URI** calls `get_rss_feed_url` and copies the Harbor app URI for the public wall/feed identity. This URI is not an HTTP-hosted RSS URL.
- **Copy contact invite** calls `get_shareable_contact_string` and copies a contact bundle containing reachable addresses and public identity keys only.

Share/export actions must not include private key material, passphrases, or encrypted backup payloads.
