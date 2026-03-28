//! SQLite connection pool (`r2d2`).

use std::path::Path;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use crate::db::migrations;
use crate::error::SoundScoutError;

/// Shared database pool.
pub struct DbPool(Pool<SqliteConnectionManager>);

impl DbPool {
    /// Open (or create) database at `path` with WAL and migrations applied.
    pub fn new(path: &Path) -> Result<Self, SoundScoutError> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::builder()
            .max_size(4)
            .build(manager)
            .map_err(|e| SoundScoutError::Pool(e.to_string()))?;
        {
            let conn = pool.get().map_err(|e| SoundScoutError::Pool(e.to_string()))?;
            conn.pragma_update(None, "journal_mode", "WAL")
                .map_err(SoundScoutError::Database)?;
            conn.pragma_update(None, "foreign_keys", true)
                .map_err(SoundScoutError::Database)?;
            conn.pragma_update(None, "busy_timeout", 5000i32)
                .map_err(SoundScoutError::Database)?;
            migrations::run_migrations(&conn)?;
        }
        Ok(Self(pool))
    }

    /// In-memory database for unit tests.
    pub fn new_in_memory() -> Result<Self, SoundScoutError> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder()
            .max_size(4)
            .build(manager)
            .map_err(|e| SoundScoutError::Pool(e.to_string()))?;
        {
            let conn = pool.get().map_err(|e| SoundScoutError::Pool(e.to_string()))?;
            conn.pragma_update(None, "journal_mode", "WAL")
                .map_err(SoundScoutError::Database)?;
            conn.pragma_update(None, "foreign_keys", true)
                .map_err(SoundScoutError::Database)?;
            conn.pragma_update(None, "busy_timeout", 5000i32)
                .map_err(SoundScoutError::Database)?;
            migrations::run_migrations(&conn)?;
        }
        Ok(Self(pool))
    }

    /// Checkout a connection from the pool.
    pub fn get(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, SoundScoutError> {
        self.0
            .get()
            .map_err(|e| SoundScoutError::Pool(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn new_in_memory_creates_tables() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("conn");
        let n: i32 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='assets'", [], |r| {
                r.get(0)
            })
            .expect("q");
        assert_eq!(n, 1);
    }

    #[test]
    fn migrations_are_idempotent() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("conn");
        migrations::run_migrations(&conn).expect("again");
    }

    #[test]
    fn wal_mode_enabled_for_file_db() {
        let dir = TempDir::new().expect("dir");
        let db = dir.path().join("t.db");
        let pool = DbPool::new(&db).expect("pool");
        let conn = pool.get().expect("conn");
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .expect("pragma");
        assert_eq!(mode.to_lowercase(), "wal");
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("conn");
        let on: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .expect("fk");
        assert_eq!(on, 1);
    }

    #[test]
    fn user_version_matches_latest_migration() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("conn");
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("v");
        assert_eq!(v, 4);
    }
}
