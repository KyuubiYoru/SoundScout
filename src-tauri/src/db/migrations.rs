//! Embedded SQL migrations.

use rusqlite::Connection;

use crate::error::SoundScoutError;

const V001_INITIAL: &str = r"
CREATE TABLE IF NOT EXISTS assets (
    id          INTEGER PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    filename    TEXT NOT NULL,
    extension   TEXT NOT NULL,
    folder      TEXT NOT NULL,
    duration_ms INTEGER,
    sample_rate INTEGER,
    channels    INTEGER,
    bit_depth   INTEGER,
    file_size   INTEGER NOT NULL,
    category    TEXT,
    publisher   TEXT,
    favorite    INTEGER NOT NULL DEFAULT 0,
    rating      INTEGER NOT NULL DEFAULT 0 CHECK(rating >= 0 AND rating <= 5),
    notes       TEXT,
    play_count  INTEGER NOT NULL DEFAULT 0,
    modified_at INTEGER NOT NULL,
    indexed_at  INTEGER NOT NULL,
    peaks       BLOB
);

CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
    filename, folder, category, publisher, notes,
    content='assets', content_rowid='id', tokenize='trigram'
);

CREATE TRIGGER IF NOT EXISTS assets_ai AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, filename, folder, category, publisher, notes)
    VALUES (new.id, new.filename, new.folder, new.category, new.publisher, new.notes);
END;

CREATE TRIGGER IF NOT EXISTS assets_ad AFTER DELETE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, filename, folder, category, publisher, notes)
    VALUES ('delete', old.id, old.filename, old.folder, old.category, old.publisher, old.notes);
END;

CREATE TRIGGER IF NOT EXISTS assets_au AFTER UPDATE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, filename, folder, category, publisher, notes)
    VALUES ('delete', old.id, old.filename, old.folder, old.category, old.publisher, old.notes);
    INSERT INTO assets_fts(rowid, filename, folder, category, publisher, notes)
    VALUES (new.id, new.filename, new.folder, new.category, new.publisher, new.notes);
END;

CREATE INDEX IF NOT EXISTS idx_assets_folder ON assets(folder);
CREATE INDEX IF NOT EXISTS idx_assets_duration ON assets(duration_ms);
CREATE INDEX IF NOT EXISTS idx_assets_sample_rate ON assets(sample_rate);
CREATE INDEX IF NOT EXISTS idx_assets_favorite ON assets(favorite) WHERE favorite = 1;
CREATE INDEX IF NOT EXISTS idx_assets_extension ON assets(extension);
";

/// CLAP / semantic search vectors (populated when Phase 3 ML pipeline is enabled).
const V003_EMBEDDINGS: &str = r"
CREATE TABLE IF NOT EXISTS embeddings (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    vector BLOB NOT NULL,
    model_id TEXT,
    created_at INTEGER NOT NULL DEFAULT 0
);
";

const V002_TAGS: &str = r"
CREATE TABLE IF NOT EXISTS tags (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);
CREATE TABLE IF NOT EXISTS asset_tags (
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    tag_id   INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (asset_id, tag_id)
);
";

/// Remove legacy playlist tables (`collections` / `collection_items`) from older installs.
const V004_DROP_COLLECTIONS: &str = r"
DROP TABLE IF EXISTS collection_items;
DROP TABLE IF EXISTS collections;
";

/// Apply pending migrations in order.
pub fn run_migrations(conn: &Connection) -> Result<(), SoundScoutError> {
    let current: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(SoundScoutError::Database)?;

    if current < 1 {
        conn.execute_batch(V001_INITIAL)
            .map_err(SoundScoutError::Database)?;
        conn.pragma_update(None, "user_version", 1i32)
            .map_err(SoundScoutError::Database)?;
    }
    let current: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(SoundScoutError::Database)?;
    if current < 2 {
        conn.execute_batch(V002_TAGS)
            .map_err(SoundScoutError::Database)?;
        conn.pragma_update(None, "user_version", 2i32)
            .map_err(SoundScoutError::Database)?;
    }
    let current: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(SoundScoutError::Database)?;
    if current < 3 {
        conn.execute_batch(V003_EMBEDDINGS)
            .map_err(SoundScoutError::Database)?;
        conn.pragma_update(None, "user_version", 3i32)
            .map_err(SoundScoutError::Database)?;
    }
    let current: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(SoundScoutError::Database)?;
    if current < 4 {
        conn.execute_batch(V004_DROP_COLLECTIONS)
            .map_err(SoundScoutError::Database)?;
        conn.pragma_update(None, "user_version", 4i32)
            .map_err(SoundScoutError::Database)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn v001_creates_assets_table() {
        let conn = Connection::open_in_memory().expect("mem");
        run_migrations(&conn).expect("migrate");
        conn.execute("INSERT INTO assets (path, filename, extension, folder, file_size, modified_at, indexed_at) VALUES ('/a', 'a', 'wav', '/b', 1, 0, 0)", [])
            .expect("insert");
    }

    #[test]
    fn v001_creates_fts_table() {
        let conn = Connection::open_in_memory().expect("mem");
        run_migrations(&conn).expect("migrate");
        let n: i32 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'assets_fts'", [], |r| r.get(0))
            .expect("q");
        assert_eq!(n, 1);
    }

    #[test]
    fn v002_creates_tags_table() {
        let conn = Connection::open_in_memory().expect("mem");
        run_migrations(&conn).expect("migrate");
        conn.execute("INSERT INTO tags (name) VALUES ('fx')", [])
            .expect("ins");
    }

    #[test]
    fn v004_drops_collections_tables() {
        let conn = Connection::open_in_memory().expect("mem");
        run_migrations(&conn).expect("migrate");
        for name in ["collections", "collection_items"] {
            let n: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [name],
                    |r| r.get(0),
                )
                .expect("q");
            assert_eq!(n, 0, "table {name} should not exist");
        }
    }

    #[test]
    fn migration_skips_already_applied() {
        let conn = Connection::open_in_memory().expect("mem");
        run_migrations(&conn).expect("m1");
        run_migrations(&conn).expect("m2");
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("v");
        assert_eq!(v, 4);
    }

    #[test]
    fn v003_creates_embeddings_table() {
        let conn = Connection::open_in_memory().expect("mem");
        run_migrations(&conn).expect("migrate");
        let n: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'embeddings'",
                [],
                |r| r.get(0),
            )
            .expect("q");
        assert_eq!(n, 1);
    }
}
