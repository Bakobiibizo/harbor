-- Migration 013: Durable call session state/history
--
-- Column additions are guarded in Rust because SQLite versions used by
-- packaged apps do not consistently support ALTER TABLE ... ADD COLUMN IF NOT
-- EXISTS. This file documents the migration and carries the final schema
-- version update for normal execution.

UPDATE schema_version SET version = 13 WHERE id = 1;
