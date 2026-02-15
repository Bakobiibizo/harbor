//! Comments repository for storing and retrieving post comments

use crate::db::Database;
use rusqlite::{params, Result as SqliteResult};
use serde::{Deserialize, Serialize};

/// Represents a comment on a post
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostComment {
    pub id: i64,
    pub comment_id: String,
    pub post_id: String,
    pub author_peer_id: String,
    pub author_name: String,
    pub content: String,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
}

/// Data needed to create a new comment
pub struct CommentData {
    pub comment_id: String,
    pub post_id: String,
    pub author_peer_id: String,
    pub author_name: String,
    pub content: String,
    pub created_at: i64,
}

/// Comment count for a post
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentCount {
    pub post_id: String,
    pub count: i64,
}

pub struct CommentsRepository;

impl CommentsRepository {
    /// Add a comment to a post
    pub fn add_comment(db: &Database, data: &CommentData) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO post_comments (comment_id, post_id, author_peer_id, author_name, content, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    data.comment_id,
                    data.post_id,
                    data.author_peer_id,
                    data.author_name,
                    data.content,
                    data.created_at,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get comments for a post (excluding deleted)
    pub fn get_comments(db: &Database, post_id: &str) -> SqliteResult<Vec<PostComment>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, comment_id, post_id, author_peer_id, author_name, content, created_at, deleted_at
                 FROM post_comments
                 WHERE post_id = ? AND deleted_at IS NULL
                 ORDER BY created_at ASC",
            )?;

            let rows = stmt.query_map(params![post_id], |row| {
                Ok(PostComment {
                    id: row.get(0)?,
                    comment_id: row.get(1)?,
                    post_id: row.get(2)?,
                    author_peer_id: row.get(3)?,
                    author_name: row.get(4)?,
                    content: row.get(5)?,
                    created_at: row.get(6)?,
                    deleted_at: row.get(7)?,
                })
            })?;

            rows.collect()
        })
    }

    /// Soft delete a comment
    pub fn delete_comment(db: &Database, comment_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let deleted_at = chrono::Utc::now().timestamp();
            let rows_affected = conn.execute(
                "UPDATE post_comments SET deleted_at = ? WHERE comment_id = ? AND deleted_at IS NULL",
                params![deleted_at, comment_id],
            )?;
            Ok(rows_affected > 0)
        })
    }

    /// Get comment count for a post
    pub fn get_comment_count(db: &Database, post_id: &str) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM post_comments WHERE post_id = ? AND deleted_at IS NULL",
                params![post_id],
                |row| row.get(0),
            )
        })
    }

    /// Get comment counts for multiple posts at once (efficient batch query)
    pub fn get_comment_counts_batch(
        db: &Database,
        post_ids: &[String],
    ) -> SqliteResult<Vec<CommentCount>> {
        if post_ids.is_empty() {
            return Ok(vec![]);
        }

        db.with_connection(|conn| {
            let placeholders: Vec<&str> = post_ids.iter().map(|_| "?").collect();
            let placeholders_str = placeholders.join(",");

            let query = format!(
                "SELECT post_id, COUNT(*) as count FROM post_comments
                 WHERE post_id IN ({}) AND deleted_at IS NULL
                 GROUP BY post_id",
                placeholders_str
            );

            let mut stmt = conn.prepare(&query)?;
            let mut counts: std::collections::HashMap<String, i64> =
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
                counts.insert(post_id, count);
            }

            // Return counts for all requested post_ids (0 for posts with no comments)
            let result: Vec<CommentCount> = post_ids
                .iter()
                .map(|post_id| CommentCount {
                    post_id: post_id.clone(),
                    count: *counts.get(post_id).unwrap_or(&0),
                })
                .collect();

            Ok(result)
        })
    }

    /// Get a comment by its comment_id
    pub fn get_by_comment_id(db: &Database, comment_id: &str) -> SqliteResult<Option<PostComment>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, comment_id, post_id, author_peer_id, author_name, content, created_at, deleted_at
                 FROM post_comments
                 WHERE comment_id = ?",
            )?;

            let mut rows = stmt.query(params![comment_id])?;

            if let Some(row) = rows.next()? {
                Ok(Some(PostComment {
                    id: row.get(0)?,
                    comment_id: row.get(1)?,
                    post_id: row.get(2)?,
                    author_peer_id: row.get(3)?,
                    author_name: row.get(4)?,
                    content: row.get(5)?,
                    created_at: row.get(6)?,
                    deleted_at: row.get(7)?,
                }))
            } else {
                Ok(None)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_comment() {
        let db = Database::in_memory().unwrap();

        // Create a post first
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (post_id, author_peer_id, content_type, visibility, lamport_clock, created_at, updated_at, signature)
                 VALUES ('post1', 'author1', 'text', 'public', 1, 1000, 1000, X'00')",
                [],
            )
        }).unwrap();

        let data = CommentData {
            comment_id: "comment-1".to_string(),
            post_id: "post1".to_string(),
            author_peer_id: "user1".to_string(),
            author_name: "Alice".to_string(),
            content: "Great post!".to_string(),
            created_at: 1001,
        };

        let id = CommentsRepository::add_comment(&db, &data).unwrap();
        assert!(id > 0);

        let comments = CommentsRepository::get_comments(&db, "post1").unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].content, "Great post!");
        assert_eq!(comments[0].author_name, "Alice");
    }

    #[test]
    fn test_delete_comment() {
        let db = Database::in_memory().unwrap();

        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (post_id, author_peer_id, content_type, visibility, lamport_clock, created_at, updated_at, signature)
                 VALUES ('post1', 'author1', 'text', 'public', 1, 1000, 1000, X'00')",
                [],
            )
        }).unwrap();

        let data = CommentData {
            comment_id: "comment-1".to_string(),
            post_id: "post1".to_string(),
            author_peer_id: "user1".to_string(),
            author_name: "Alice".to_string(),
            content: "Great post!".to_string(),
            created_at: 1001,
        };

        CommentsRepository::add_comment(&db, &data).unwrap();
        assert_eq!(CommentsRepository::get_comments(&db, "post1").unwrap().len(), 1);

        let deleted = CommentsRepository::delete_comment(&db, "comment-1").unwrap();
        assert!(deleted);

        // Soft deleted - should not appear in get_comments
        let comments = CommentsRepository::get_comments(&db, "post1").unwrap();
        assert!(comments.is_empty());
    }

    #[test]
    fn test_comment_count() {
        let db = Database::in_memory().unwrap();

        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (post_id, author_peer_id, content_type, visibility, lamport_clock, created_at, updated_at, signature)
                 VALUES ('post1', 'author1', 'text', 'public', 1, 1000, 1000, X'00')",
                [],
            )
        }).unwrap();

        for i in 1..=3 {
            let data = CommentData {
                comment_id: format!("comment-{}", i),
                post_id: "post1".to_string(),
                author_peer_id: format!("user{}", i),
                author_name: format!("User {}", i),
                content: format!("Comment {}", i),
                created_at: 1000 + i,
            };
            CommentsRepository::add_comment(&db, &data).unwrap();
        }

        let count = CommentsRepository::get_comment_count(&db, "post1").unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_comment_counts_batch() {
        let db = Database::in_memory().unwrap();

        // Create two posts
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (post_id, author_peer_id, content_type, visibility, lamport_clock, created_at, updated_at, signature)
                 VALUES ('post1', 'author1', 'text', 'public', 1, 1000, 1000, X'00')",
                [],
            )?;
            conn.execute(
                "INSERT INTO posts (post_id, author_peer_id, content_type, visibility, lamport_clock, created_at, updated_at, signature)
                 VALUES ('post2', 'author1', 'text', 'public', 2, 1001, 1001, X'00')",
                [],
            )
        }).unwrap();

        // Add 2 comments to post1
        for i in 1..=2 {
            let data = CommentData {
                comment_id: format!("comment-p1-{}", i),
                post_id: "post1".to_string(),
                author_peer_id: "user1".to_string(),
                author_name: "User 1".to_string(),
                content: format!("Comment {}", i),
                created_at: 1000 + i,
            };
            CommentsRepository::add_comment(&db, &data).unwrap();
        }

        // Add 1 comment to post2
        let data = CommentData {
            comment_id: "comment-p2-1".to_string(),
            post_id: "post2".to_string(),
            author_peer_id: "user1".to_string(),
            author_name: "User 1".to_string(),
            content: "Comment on post 2".to_string(),
            created_at: 1003,
        };
        CommentsRepository::add_comment(&db, &data).unwrap();

        let post_ids = vec!["post1".to_string(), "post2".to_string(), "post3".to_string()];
        let counts = CommentsRepository::get_comment_counts_batch(&db, &post_ids).unwrap();

        assert_eq!(counts.len(), 3);
        assert_eq!(counts[0].count, 2); // post1
        assert_eq!(counts[1].count, 1); // post2
        assert_eq!(counts[2].count, 0); // post3 (no comments)
    }
}
