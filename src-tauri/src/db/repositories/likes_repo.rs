//! Likes repository for storing and retrieving post likes/reactions

use crate::db::Database;
use rusqlite::{params, Result as SqliteResult};
use serde::{Deserialize, Serialize};

use crate::db::sql_utils::build_in_clause_placeholders;

/// Represents a like on a post
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostLike {
    pub id: i64,
    pub post_id: String,
    pub liker_peer_id: String,
    pub reaction_type: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
    pub created_at: i64,
}

/// Data needed to create a new like
pub struct LikeData {
    pub post_id: String,
    pub liker_peer_id: String,
    pub reaction_type: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Summary of likes for a post
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LikeSummary {
    pub post_id: String,
    pub total_likes: i64,
    pub user_has_liked: bool,
}

pub struct LikesRepository;

impl LikesRepository {
    /// Add a like to a post
    pub fn add_like(db: &Database, data: &LikeData) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO post_likes (post_id, liker_peer_id, reaction_type, timestamp, signature)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(post_id, liker_peer_id) DO UPDATE SET
                     reaction_type = excluded.reaction_type,
                     timestamp = excluded.timestamp,
                     signature = excluded.signature",
                params![
                    data.post_id,
                    data.liker_peer_id,
                    data.reaction_type,
                    data.timestamp,
                    data.signature,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Remove a like from a post
    pub fn remove_like(db: &Database, post_id: &str, liker_peer_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows_affected = conn.execute(
                "DELETE FROM post_likes WHERE post_id = ? AND liker_peer_id = ?",
                params![post_id, liker_peer_id],
            )?;
            Ok(rows_affected > 0)
        })
    }

    /// Check if a user has liked a post
    pub fn has_liked(db: &Database, post_id: &str, liker_peer_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM post_likes WHERE post_id = ? AND liker_peer_id = ?",
                params![post_id, liker_peer_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Get the total number of likes for a post
    pub fn get_like_count(db: &Database, post_id: &str) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM post_likes WHERE post_id = ?",
                params![post_id],
                |row| row.get(0),
            )
        })
    }

    /// Get a summary of likes for a post (count + whether current user liked)
    pub fn get_like_summary(
        db: &Database,
        post_id: &str,
        current_user_peer_id: &str,
    ) -> SqliteResult<LikeSummary> {
        db.with_connection(|conn| {
            let total_likes: i64 = conn.query_row(
                "SELECT COUNT(*) FROM post_likes WHERE post_id = ?",
                params![post_id],
                |row| row.get(0),
            )?;

            let user_has_liked: i64 = conn.query_row(
                "SELECT COUNT(*) FROM post_likes WHERE post_id = ? AND liker_peer_id = ?",
                params![post_id, current_user_peer_id],
                |row| row.get(0),
            )?;

            Ok(LikeSummary {
                post_id: post_id.to_string(),
                total_likes,
                user_has_liked: user_has_liked > 0,
            })
        })
    }

    /// Get all likes for a post
    pub fn get_likes_for_post(db: &Database, post_id: &str) -> SqliteResult<Vec<PostLike>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, post_id, liker_peer_id, reaction_type, timestamp, signature, created_at
                 FROM post_likes
                 WHERE post_id = ?
                 ORDER BY timestamp DESC",
            )?;

            let rows = stmt.query_map(params![post_id], |row| {
                Ok(PostLike {
                    id: row.get(0)?,
                    post_id: row.get(1)?,
                    liker_peer_id: row.get(2)?,
                    reaction_type: row.get(3)?,
                    timestamp: row.get(4)?,
                    signature: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?;

            rows.collect()
        })
    }

    /// Get all posts that a user has liked
    pub fn get_liked_posts(db: &Database, liker_peer_id: &str) -> SqliteResult<Vec<String>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT post_id FROM post_likes WHERE liker_peer_id = ? ORDER BY timestamp DESC",
            )?;

            let rows = stmt.query_map(params![liker_peer_id], |row| row.get(0))?;

            rows.collect()
        })
    }

    /// Get like summaries for multiple posts at once (efficient batch query)
    pub fn get_like_summaries_batch(
        db: &Database,
        post_ids: &[String],
        current_user_peer_id: &str,
    ) -> SqliteResult<Vec<LikeSummary>> {
        if post_ids.is_empty() {
            return Ok(vec![]);
        }

        db.with_connection(|conn| {
            // SAFETY: `build_in_clause_placeholders` returns only literal "?" characters
            // joined by commas (e.g., "?,?,?"). No user input is interpolated into the
            // SQL structure. All actual values are bound via `params_from_iter`.
            let placeholders_str = build_in_clause_placeholders(post_ids.len());

            let likes_query = format!(
                "SELECT post_id, COUNT(*) as count FROM post_likes WHERE post_id IN ({}) GROUP BY post_id",
                placeholders_str
            );

            let mut stmt = conn.prepare(&likes_query)?;
            let mut like_counts: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();

            let rows = stmt.query_map(
                rusqlite::params_from_iter(post_ids.iter()),
                |row| {
                    let post_id: String = row.get(0)?;
                    let count: i64 = row.get(1)?;
                    Ok((post_id, count))
                },
            )?;

            for row in rows {
                let (post_id, count) = row?;
                like_counts.insert(post_id, count);
            }

            let user_likes_query = format!(
                "SELECT post_id FROM post_likes WHERE post_id IN ({}) AND liker_peer_id = ?",
                placeholders_str
            );

            let mut params: Vec<&dyn rusqlite::ToSql> = post_ids
                .iter()
                .map(|s| s as &dyn rusqlite::ToSql)
                .collect();
            params.push(&current_user_peer_id);

            let mut stmt = conn.prepare(&user_likes_query)?;
            let mut user_liked: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| row.get(0))?;

            for row in rows {
                user_liked.insert(row?);
            }

            // Build summaries
            let summaries: Vec<LikeSummary> = post_ids
                .iter()
                .map(|post_id| LikeSummary {
                    post_id: post_id.clone(),
                    total_likes: *like_counts.get(post_id).unwrap_or(&0),
                    user_has_liked: user_liked.contains(post_id),
                })
                .collect();

            Ok(summaries)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_like() {
        let db = Database::in_memory().unwrap();

        // First we need a post to like - let's just insert directly for the test
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (post_id, author_peer_id, content_type, visibility, lamport_clock, created_at, updated_at, signature)
                 VALUES ('post1', 'author1', 'text', 'public', 1, 1000, 1000, X'00')",
                [],
            )
        }).unwrap();

        let data = LikeData {
            post_id: "post1".to_string(),
            liker_peer_id: "user1".to_string(),
            reaction_type: "like".to_string(),
            timestamp: 1000,
            signature: vec![0, 1, 2, 3],
        };

        LikesRepository::add_like(&db, &data).unwrap();

        let has_liked = LikesRepository::has_liked(&db, "post1", "user1").unwrap();
        assert!(has_liked);

        let count = LikesRepository::get_like_count(&db, "post1").unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_remove_like() {
        let db = Database::in_memory().unwrap();

        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (post_id, author_peer_id, content_type, visibility, lamport_clock, created_at, updated_at, signature)
                 VALUES ('post1', 'author1', 'text', 'public', 1, 1000, 1000, X'00')",
                [],
            )
        }).unwrap();

        let data = LikeData {
            post_id: "post1".to_string(),
            liker_peer_id: "user1".to_string(),
            reaction_type: "like".to_string(),
            timestamp: 1000,
            signature: vec![0, 1, 2, 3],
        };

        LikesRepository::add_like(&db, &data).unwrap();
        assert!(LikesRepository::has_liked(&db, "post1", "user1").unwrap());

        let removed = LikesRepository::remove_like(&db, "post1", "user1").unwrap();
        assert!(removed);
        assert!(!LikesRepository::has_liked(&db, "post1", "user1").unwrap());
    }

    #[test]
    fn test_like_summary() {
        let db = Database::in_memory().unwrap();

        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (post_id, author_peer_id, content_type, visibility, lamport_clock, created_at, updated_at, signature)
                 VALUES ('post1', 'author1', 'text', 'public', 1, 1000, 1000, X'00')",
                [],
            )
        }).unwrap();

        // Add likes from multiple users
        for i in 1..=5 {
            let data = LikeData {
                post_id: "post1".to_string(),
                liker_peer_id: format!("user{}", i),
                reaction_type: "like".to_string(),
                timestamp: 1000 + i,
                signature: vec![0, 1, 2, 3],
            };
            LikesRepository::add_like(&db, &data).unwrap();
        }

        // User3 should see they've liked it
        let summary = LikesRepository::get_like_summary(&db, "post1", "user3").unwrap();
        assert_eq!(summary.total_likes, 5);
        assert!(summary.user_has_liked);

        // User99 (who hasn't liked) should see they haven't
        let summary = LikesRepository::get_like_summary(&db, "post1", "user99").unwrap();
        assert_eq!(summary.total_likes, 5);
        assert!(!summary.user_has_liked);
    }

    #[test]
    fn test_unique_like_per_user() {
        let db = Database::in_memory().unwrap();

        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (post_id, author_peer_id, content_type, visibility, lamport_clock, created_at, updated_at, signature)
                 VALUES ('post1', 'author1', 'text', 'public', 1, 1000, 1000, X'00')",
                [],
            )
        }).unwrap();

        let data = LikeData {
            post_id: "post1".to_string(),
            liker_peer_id: "user1".to_string(),
            reaction_type: "like".to_string(),
            timestamp: 1000,
            signature: vec![0, 1, 2, 3],
        };

        // Like twice - should only count as one
        LikesRepository::add_like(&db, &data).unwrap();
        LikesRepository::add_like(&db, &data).unwrap();

        let count = LikesRepository::get_like_count(&db, "post1").unwrap();
        assert_eq!(count, 1);
    }
}
