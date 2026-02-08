use crate::db::Database;
use rusqlite::{OptionalExtension, Result as SqliteResult};
use serde::{Deserialize, Serialize};

/// Bootstrap node configuration stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapNodeConfig {
    pub id: i64,
    pub address: String,
    pub name: Option<String>,
    pub is_enabled: bool,
    pub priority: i32,
    pub is_default: bool,
    pub last_connected_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Input for adding a new bootstrap node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddBootstrapNodeInput {
    pub address: String,
    pub name: Option<String>,
    pub priority: Option<i32>,
    pub is_default: Option<bool>,
}

pub struct BootstrapNodesRepo;

impl BootstrapNodesRepo {
    /// Get all bootstrap nodes, optionally filtered to enabled only
    pub fn get_all(db: &Database, enabled_only: bool) -> SqliteResult<Vec<BootstrapNodeConfig>> {
        db.with_connection(|conn| {
            let query = if enabled_only {
                "SELECT id, address, name, is_enabled, priority, is_default, last_connected_at, created_at, updated_at
                 FROM bootstrap_nodes
                 WHERE is_enabled = 1
                 ORDER BY priority ASC, created_at ASC"
            } else {
                "SELECT id, address, name, is_enabled, priority, is_default, last_connected_at, created_at, updated_at
                 FROM bootstrap_nodes
                 ORDER BY priority ASC, created_at ASC"
            };

            let mut stmt = conn.prepare(query)?;
            let nodes = stmt
                .query_map([], |row| {
                    Ok(BootstrapNodeConfig {
                        id: row.get(0)?,
                        address: row.get(1)?,
                        name: row.get(2)?,
                        is_enabled: row.get::<_, i32>(3)? != 0,
                        priority: row.get(4)?,
                        is_default: row.get::<_, i32>(5)? != 0,
                        last_connected_at: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(nodes)
        })
    }

    /// Get a bootstrap node by ID
    pub fn get_by_id(db: &Database, id: i64) -> SqliteResult<Option<BootstrapNodeConfig>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, address, name, is_enabled, priority, is_default, last_connected_at, created_at, updated_at
                 FROM bootstrap_nodes
                 WHERE id = ?",
            )?;

            let node = stmt
                .query_row([id], |row| {
                    Ok(BootstrapNodeConfig {
                        id: row.get(0)?,
                        address: row.get(1)?,
                        name: row.get(2)?,
                        is_enabled: row.get::<_, i32>(3)? != 0,
                        priority: row.get(4)?,
                        is_default: row.get::<_, i32>(5)? != 0,
                        last_connected_at: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })
                .optional()?;

            Ok(node)
        })
    }

    /// Add a new bootstrap node
    pub fn add(db: &Database, input: AddBootstrapNodeInput) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();

            conn.execute(
                "INSERT INTO bootstrap_nodes (address, name, priority, is_default, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    input.address,
                    input.name,
                    input.priority.unwrap_or(0),
                    input.is_default.unwrap_or(false) as i32,
                    now,
                    now
                ],
            )?;

            Ok(conn.last_insert_rowid())
        })
    }

    /// Update a bootstrap node.
    ///
    /// Only the provided `Option` fields are included in the SET clause. The SQL
    /// is built dynamically with `format!()`, but the only values interpolated
    /// into the query structure are hardcoded column-name fragments (see the
    /// `SAFETY` comment below). All user-supplied data is bound via parameterized
    /// placeholders (`?`).
    pub fn update(
        db: &Database,
        id: i64,
        name: Option<String>,
        is_enabled: Option<bool>,
        priority: Option<i32>,
    ) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();

            // Each entry is a hardcoded "column = ?" fragment -- never user input.
            let mut set_clauses: Vec<&str> = vec!["updated_at = ?"];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];

            if let Some(ref node_name) = name {
                set_clauses.push("name = ?");
                params.push(Box::new(node_name.clone()));
            }
            if let Some(enabled) = is_enabled {
                set_clauses.push("is_enabled = ?");
                params.push(Box::new(enabled as i32));
            }
            if let Some(prio) = priority {
                set_clauses.push("priority = ?");
                params.push(Box::new(prio));
            }

            params.push(Box::new(id));

            // SAFETY: `set_clauses` contains only hardcoded string literals defined
            // directly above (e.g., "updated_at = ?", "name = ?"). No user input is
            // interpolated into the SQL structure. All actual values are bound via
            // parameterized placeholders through `params_from_iter`.
            let query = format!(
                "UPDATE bootstrap_nodes SET {} WHERE id = ?",
                set_clauses.join(", ")
            );

            let rows = conn.execute(
                &query,
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            )?;

            Ok(rows > 0)
        })
    }

    /// Remove a bootstrap node (only non-default nodes can be removed)
    pub fn remove(db: &Database, id: i64) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "DELETE FROM bootstrap_nodes WHERE id = ? AND is_default = 0",
                [id],
            )?;

            Ok(rows > 0)
        })
    }

    /// Record a successful connection to a bootstrap node
    pub fn record_connection(db: &Database, id: i64) -> SqliteResult<()> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();

            conn.execute(
                "UPDATE bootstrap_nodes SET last_connected_at = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![now, now, id],
            )?;

            Ok(())
        })
    }

    /// Record a successful connection by address
    pub fn record_connection_by_address(db: &Database, address: &str) -> SqliteResult<()> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();

            conn.execute(
                "UPDATE bootstrap_nodes SET last_connected_at = ?, updated_at = ? WHERE address = ?",
                rusqlite::params![now, now, address],
            )?;

            Ok(())
        })
    }

    /// Get enabled addresses in priority order
    pub fn get_enabled_addresses(db: &Database) -> SqliteResult<Vec<String>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT address FROM bootstrap_nodes
                 WHERE is_enabled = 1
                 ORDER BY priority ASC, last_connected_at DESC NULLS LAST",
            )?;

            let addresses = stmt
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;

            Ok(addresses)
        })
    }

    /// Check if an address already exists
    pub fn exists(db: &Database, address: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let count: i32 = conn.query_row(
                "SELECT COUNT(*) FROM bootstrap_nodes WHERE address = ?",
                [address],
                |row| row.get(0),
            )?;

            Ok(count > 0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_bootstrap_node() {
        let db = Database::in_memory().unwrap();

        // Add a bootstrap node
        let input = AddBootstrapNodeInput {
            address: "/ip4/1.2.3.4/tcp/9000/p2p/12D3KooWTestPeer".to_string(),
            name: Some("Test Node".to_string()),
            priority: Some(1),
            is_default: None,
        };

        let id = BootstrapNodesRepo::add(&db, input).unwrap();
        assert!(id > 0);

        // Get by ID
        let node = BootstrapNodesRepo::get_by_id(&db, id).unwrap().unwrap();
        assert_eq!(node.address, "/ip4/1.2.3.4/tcp/9000/p2p/12D3KooWTestPeer");
        assert_eq!(node.name, Some("Test Node".to_string()));
        assert_eq!(node.priority, 1);
        assert!(node.is_enabled);
        assert!(!node.is_default);
    }

    #[test]
    fn test_get_enabled_addresses() {
        let db = Database::in_memory().unwrap();

        // Add multiple nodes
        let input1 = AddBootstrapNodeInput {
            address: "/ip4/1.1.1.1/tcp/9000/p2p/Peer1".to_string(),
            name: None,
            priority: Some(2),
            is_default: None,
        };
        let id1 = BootstrapNodesRepo::add(&db, input1).unwrap();

        let input2 = AddBootstrapNodeInput {
            address: "/ip4/2.2.2.2/tcp/9000/p2p/Peer2".to_string(),
            name: None,
            priority: Some(1),
            is_default: None,
        };
        BootstrapNodesRepo::add(&db, input2).unwrap();

        // Disable the first one
        BootstrapNodesRepo::update(&db, id1, None, Some(false), None).unwrap();

        // Get enabled addresses
        let addresses = BootstrapNodesRepo::get_enabled_addresses(&db).unwrap();
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0], "/ip4/2.2.2.2/tcp/9000/p2p/Peer2");
    }

    #[test]
    fn test_remove_node() {
        let db = Database::in_memory().unwrap();

        // Add a non-default node
        let input = AddBootstrapNodeInput {
            address: "/ip4/1.2.3.4/tcp/9000/p2p/12D3KooWTestPeer".to_string(),
            name: None,
            priority: None,
            is_default: None,
        };
        let id = BootstrapNodesRepo::add(&db, input).unwrap();

        // Remove it
        let removed = BootstrapNodesRepo::remove(&db, id).unwrap();
        assert!(removed);

        // Should be gone
        let node = BootstrapNodesRepo::get_by_id(&db, id).unwrap();
        assert!(node.is_none());
    }
}
