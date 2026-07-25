use crate::db::Database;
use rusqlite::{OptionalExtension, Result as SqliteResult};

pub const MAX_CALL_SIGNALING_REPLAYS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalingReplayRecord {
    Recorded,
    Duplicate,
}

pub struct CallSignalingReplayRepository;

impl CallSignalingReplayRepository {
    /// Atomically prune the expired replay window, record one authenticated
    /// envelope, and enforce the hard profile-local storage cap.
    pub fn check_and_record(
        db: &Database,
        fingerprint: &str,
        sender_peer_id: &str,
        now: i64,
        expires_at: i64,
    ) -> SqliteResult<SignalingReplayRecord> {
        db.with_connection_mut(|connection| {
            let transaction = connection.transaction()?;
            let existing_expiry = transaction
                .query_row(
                    "SELECT expires_at FROM call_signaling_replay WHERE fingerprint = ?",
                    [fingerprint],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if existing_expiry.is_some_and(|expiry| expiry > now) {
                transaction.commit()?;
                return Ok(SignalingReplayRecord::Duplicate);
            }

            transaction.execute(
                "DELETE FROM call_signaling_replay WHERE expires_at <= ?",
                [now],
            )?;
            let inserted = transaction.execute(
                "INSERT OR IGNORE INTO call_signaling_replay
                    (fingerprint, sender_peer_id, seen_at, expires_at)
                 VALUES (?, ?, ?, ?)",
                rusqlite::params![fingerprint, sender_peer_id, now, expires_at],
            )?;
            debug_assert_eq!(inserted, 1);

            transaction.execute(
                "DELETE FROM call_signaling_replay
                 WHERE fingerprint NOT IN (
                     SELECT fingerprint
                     FROM call_signaling_replay
                     ORDER BY sequence DESC
                     LIMIT ?
                 )",
                [MAX_CALL_SIGNALING_REPLAYS as i64],
            )?;
            transaction.commit()?;
            Ok(SignalingReplayRecord::Recorded)
        })
    }

    pub fn prune_expired(db: &Database, now: i64) -> SqliteResult<usize> {
        db.with_connection(|connection| {
            connection.execute(
                "DELETE FROM call_signaling_replay WHERE expires_at <= ?",
                [now],
            )
        })
    }

    #[cfg(test)]
    pub fn count(db: &Database) -> SqliteResult<usize> {
        db.with_connection(|connection| {
            connection.query_row("SELECT COUNT(*) FROM call_signaling_replay", [], |row| {
                row.get(0)
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn churn_is_capped_and_expired_rows_prune() {
        let db = Database::in_memory().unwrap();
        for index in 0..(MAX_CALL_SIGNALING_REPLAYS + 200) {
            assert_eq!(
                CallSignalingReplayRepository::check_and_record(
                    &db,
                    &format!("fingerprint-{index}"),
                    "peer",
                    100,
                    200,
                )
                .unwrap(),
                SignalingReplayRecord::Recorded
            );
        }
        assert_eq!(
            CallSignalingReplayRepository::count(&db).unwrap(),
            MAX_CALL_SIGNALING_REPLAYS
        );

        assert_eq!(
            CallSignalingReplayRepository::prune_expired(&db, 200).unwrap(),
            MAX_CALL_SIGNALING_REPLAYS
        );
        assert_eq!(CallSignalingReplayRepository::count(&db).unwrap(), 0);
    }

    #[test]
    fn duplicate_survives_repository_recreation_within_replay_window() {
        let db = Database::in_memory().unwrap();
        assert_eq!(
            CallSignalingReplayRepository::check_and_record(&db, "same", "peer", 100, 200).unwrap(),
            SignalingReplayRecord::Recorded
        );
        assert_eq!(
            CallSignalingReplayRepository::check_and_record(&db, "same", "peer", 101, 200).unwrap(),
            SignalingReplayRecord::Duplicate
        );
        let seen_at = db
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT seen_at FROM call_signaling_replay WHERE fingerprint = 'same'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap();
        assert_eq!(seen_at, 100, "duplicates must not mutate replay state");
        assert_eq!(
            CallSignalingReplayRepository::check_and_record(&db, "same", "peer", 200, 300).unwrap(),
            SignalingReplayRecord::Recorded
        );
    }
}
