//! Parameterised SQL helpers.

use std::collections::{BTreeMap, HashMap};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::db::models::{Asset, FilterOptions, FolderNode, NewAsset, SemanticSearchStatus, Tag, TagWithCount};
use crate::error::SoundScoutError;

fn asset_from_row(row: &Row<'_>) -> Result<Asset, rusqlite::Error> {
    Ok(Asset {
        id: row.get(0)?,
        path: row.get(1)?,
        filename: row.get(2)?,
        extension: row.get(3)?,
        folder: row.get(4)?,
        duration_ms: row.get(5)?,
        sample_rate: row.get(6)?,
        channels: row.get(7)?,
        bit_depth: row.get(8)?,
        file_size: row.get(9)?,
        category: row.get(10)?,
        publisher: row.get(11)?,
        favorite: row.get::<_, i64>(12)? != 0,
        rating: row.get::<_, i64>(13)? as u8,
        notes: row.get(14)?,
        play_count: row.get(15)?,
    })
}

/// Batch insert; skips duplicate paths. Returns number of inserted rows.
pub fn insert_asset_batch(conn: &Connection, assets: &[NewAsset]) -> Result<usize, SoundScoutError> {
    let tx = conn.unchecked_transaction().map_err(SoundScoutError::Database)?;
    let mut inserted = 0usize;
    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT OR IGNORE INTO assets (path, filename, extension, folder, duration_ms, sample_rate, channels, bit_depth, file_size, category, publisher, modified_at, indexed_at, peaks)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )
            .map_err(SoundScoutError::Database)?;
        for a in assets {
            let n = stmt
                .execute(params![
                    a.path,
                    a.filename,
                    a.extension,
                    a.folder,
                    a.duration_ms,
                    a.sample_rate,
                    a.channels,
                    a.bit_depth,
                    a.file_size,
                    a.category,
                    a.publisher,
                    a.modified_at,
                    a.indexed_at,
                    a.peaks.as_deref(),
                ])
                .map_err(SoundScoutError::Database)?;
            inserted += n;
        }
    }
    tx.commit().map_err(SoundScoutError::Database)?;
    Ok(inserted)
}

/// Update metadata for an existing row matched by `path`.
pub fn update_asset_metadata(conn: &Connection, path: &str, asset: &NewAsset) -> Result<(), SoundScoutError> {
    conn.execute(
        "UPDATE assets SET duration_ms = ?1, sample_rate = ?2, channels = ?3, bit_depth = ?4, file_size = ?5,
         category = ?6, publisher = ?7, modified_at = ?8, indexed_at = ?9, peaks = ?10,
         filename = ?11, extension = ?12, folder = ?13
         WHERE path = ?14",
        params![
            asset.duration_ms,
            asset.sample_rate,
            asset.channels,
            asset.bit_depth,
            asset.file_size,
            asset.category,
            asset.publisher,
            asset.modified_at,
            asset.indexed_at,
            asset.peaks.as_deref(),
            asset.filename,
            asset.extension,
            asset.folder,
            path,
        ],
    )
    .map_err(SoundScoutError::Database)?;
    Ok(())
}

/// Fetch asset by primary key.
pub fn get_asset_by_id(conn: &Connection, id: i64) -> Result<Option<Asset>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, path, filename, extension, folder, duration_ms, sample_rate, channels, bit_depth, file_size, category, publisher, favorite, rating, notes, play_count FROM assets WHERE id = ?1",
        )
        .map_err(SoundScoutError::Database)?;
    stmt.query_row(params![id], |row| asset_from_row(row))
        .optional()
        .map_err(SoundScoutError::Database)
}

/// Fetch asset by filesystem path.
pub fn get_asset_by_path(conn: &Connection, path: &str) -> Result<Option<Asset>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, path, filename, extension, folder, duration_ms, sample_rate, channels, bit_depth, file_size, category, publisher, favorite, rating, notes, play_count FROM assets WHERE path = ?1",
        )
        .map_err(SoundScoutError::Database)?;
    stmt.query_row(params![path], |row| asset_from_row(row))
        .optional()
        .map_err(SoundScoutError::Database)
}

/// Map indexed path → `modified_at` for incremental scans.
pub fn get_indexed_paths_with_mtime(conn: &Connection) -> Result<HashMap<String, i64>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached("SELECT path, modified_at FROM assets")
        .map_err(SoundScoutError::Database)?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(SoundScoutError::Database)?;
    let mut m = HashMap::new();
    for r in rows {
        let (p, t) = r.map_err(SoundScoutError::Database)?;
        m.insert(p, t);
    }
    Ok(m)
}

/// Delete assets by id; returns deleted row count.
pub fn delete_assets_by_ids(conn: &Connection, ids: &[i64]) -> Result<usize, SoundScoutError> {
    let mut n = 0usize;
    for id in ids {
        n += conn
            .execute("DELETE FROM assets WHERE id = ?1", params![id])
            .map_err(SoundScoutError::Database)?;
    }
    Ok(n)
}

/// Remove every indexed asset and tag (embeddings and asset-tag links cascade from `assets`).
/// FTS stays aligned via delete triggers on `assets`.
pub fn wipe_library_data(conn: &Connection) -> Result<(), SoundScoutError> {
    conn.execute("DELETE FROM assets", [])
        .map_err(SoundScoutError::Database)?;
    conn.execute("DELETE FROM tags", [])
        .map_err(SoundScoutError::Database)?;
    Ok(())
}

/// Set favorite flag.
pub fn update_favorite(conn: &Connection, id: i64, favorite: bool) -> Result<(), SoundScoutError> {
    let v: i64 = if favorite { 1 } else { 0 };
    conn.execute("UPDATE assets SET favorite = ?1 WHERE id = ?2", params![v, id])
        .map_err(SoundScoutError::Database)?;
    Ok(())
}

/// Set rating `0..=5`.
pub fn update_rating(conn: &Connection, id: i64, rating: u8) -> Result<(), SoundScoutError> {
    if rating > 5 {
        return Err(SoundScoutError::Validation(
            "rating must be between 0 and 5".into(),
        ));
    }
    conn.execute(
        "UPDATE assets SET rating = ?1 WHERE id = ?2",
        params![i64::from(rating), id],
    )
    .map_err(SoundScoutError::Database)?;
    Ok(())
}

/// Replace peaks BLOB.
pub fn update_peaks(conn: &Connection, id: i64, peaks: &[u8]) -> Result<(), SoundScoutError> {
    conn.execute(
        "UPDATE assets SET peaks = ?1 WHERE id = ?2",
        params![peaks, id],
    )
    .map_err(SoundScoutError::Database)?;
    Ok(())
}

/// Read peaks BLOB.
pub fn get_peaks(conn: &Connection, id: i64) -> Result<Option<Vec<u8>>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached("SELECT peaks FROM assets WHERE id = ?1")
        .map_err(SoundScoutError::Database)?;
    let v: Option<Vec<u8>> = stmt
        .query_row(params![id], |row| row.get(0))
        .optional()
        .map_err(SoundScoutError::Database)?;
    Ok(v)
}

/// `(folder_path, file_count)` rows sorted by folder.
pub fn get_folder_tree(conn: &Connection) -> Result<Vec<(String, u64)>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached("SELECT folder, COUNT(*) FROM assets GROUP BY folder ORDER BY folder")
        .map_err(SoundScoutError::Database)?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64)))
        .map_err(SoundScoutError::Database)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(SoundScoutError::Database)?);
    }
    Ok(out)
}

#[derive(Default)]
struct TrieNode {
    count: u64,
    children: BTreeMap<String, TrieNode>,
    /// Full path for this node (set when this node corresponds to a DB folder row).
    path: Option<String>,
}

/// Build nested folder nodes from flat `(folder, count)` rows.
pub fn build_folder_tree(flat: &[(String, u64)]) -> Vec<FolderNode> {
    let mut root = TrieNode::default();
    for (folder, count) in flat {
        let parts: Vec<String> = folder
            .split('/')
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string)
            .collect();
        if parts.is_empty() {
            continue;
        }
        let mut node = &mut root;
        let mut acc = String::new();
        for (i, part) in parts.iter().enumerate() {
            if acc.is_empty() {
                acc.push('/');
                acc.push_str(part);
            } else {
                acc.push('/');
                acc.push_str(part);
            }
            node = node.children.entry(part.clone()).or_default();
            if i + 1 == parts.len() {
                node.count = *count;
                node.path = Some(acc.clone());
            }
        }
    }

    fn to_folder_nodes(prefix: &str, map: &BTreeMap<String, TrieNode>) -> Vec<FolderNode> {
        let mut out = Vec::new();
        for (name, n) in map {
            let path = n
                .path
                .clone()
                .unwrap_or_else(|| format!("{prefix}/{name}").replace("//", "/"));
            let children = to_folder_nodes(&path, &n.children);
            out.push(FolderNode {
                name: name.clone(),
                path,
                count: n.count,
                children,
            });
        }
        out
    }

    to_folder_nodes("", &root.children)
}

/// Publisher with asset counts.
pub fn get_publishers(conn: &Connection) -> Result<Vec<(String, u64)>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT publisher, COUNT(*) FROM assets WHERE publisher IS NOT NULL AND publisher != '' GROUP BY publisher ORDER BY publisher",
        )
        .map_err(SoundScoutError::Database)?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64)))
        .map_err(SoundScoutError::Database)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(SoundScoutError::Database)?);
    }
    Ok(out)
}

/// Aggregate values for filter UI.
pub fn get_filter_options(conn: &Connection) -> Result<FilterOptions, SoundScoutError> {
    let mut extensions: Vec<String> = conn
        .prepare_cached("SELECT DISTINCT extension FROM assets ORDER BY extension")
        .map_err(SoundScoutError::Database)?
        .query_map([], |row| row.get(0))
        .map_err(SoundScoutError::Database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SoundScoutError::Database)?;

    extensions.sort();

    let mut rates: Vec<i32> = conn
        .prepare_cached("SELECT DISTINCT sample_rate FROM assets WHERE sample_rate IS NOT NULL ORDER BY sample_rate")
        .map_err(SoundScoutError::Database)?
        .query_map([], |row| row.get(0))
        .map_err(SoundScoutError::Database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SoundScoutError::Database)?;

    rates.sort();

    let (min_d, max_d): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT MIN(duration_ms), MAX(duration_ms) FROM assets WHERE duration_ms IS NOT NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((None, None));

    let mut publishers: Vec<String> = conn
        .prepare_cached("SELECT DISTINCT publisher FROM assets WHERE publisher IS NOT NULL AND publisher != '' ORDER BY publisher")
        .map_err(SoundScoutError::Database)?
        .query_map([], |row| row.get(0))
        .map_err(SoundScoutError::Database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SoundScoutError::Database)?;

    publishers.sort();

    Ok(FilterOptions {
        extensions,
        sample_rates: rates,
        min_duration_ms: min_d.unwrap_or(0),
        max_duration_ms: max_d.unwrap_or(0),
        publishers,
    })
}

/// List assets whose `folder` matches.
pub fn get_assets_by_folder(
    conn: &Connection,
    folder: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Asset>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, path, filename, extension, folder, duration_ms, sample_rate, channels, bit_depth, file_size, category, publisher, favorite, rating, notes, play_count FROM assets WHERE folder = ?1 ORDER BY filename LIMIT ?2 OFFSET ?3",
        )
        .map_err(SoundScoutError::Database)?;
    let rows = stmt
        .query_map(params![folder, limit, offset], |row| asset_from_row(row))
        .map_err(SoundScoutError::Database)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(SoundScoutError::Database)?);
    }
    Ok(out)
}

/// Attach tag to asset (creates tag row if needed).
pub fn add_tag(conn: &Connection, asset_id: i64, tag_name: &str) -> Result<(), SoundScoutError> {
    conn.execute(
        "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
        params![tag_name],
    )
    .map_err(SoundScoutError::Database)?;
    let tag_id: i64 = conn
        .query_row("SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE", params![tag_name], |r| {
            r.get(0)
        })
        .map_err(SoundScoutError::Database)?;
    conn.execute(
        "INSERT OR IGNORE INTO asset_tags (asset_id, tag_id) VALUES (?1, ?2)",
        params![asset_id, tag_id],
    )
    .map_err(SoundScoutError::Database)?;
    Ok(())
}

pub fn remove_tag(conn: &Connection, asset_id: i64, tag_id: i64) -> Result<(), SoundScoutError> {
    conn.execute(
        "DELETE FROM asset_tags WHERE asset_id = ?1 AND tag_id = ?2",
        params![asset_id, tag_id],
    )
    .map_err(SoundScoutError::Database)?;
    Ok(())
}

pub fn get_tags_for_asset(conn: &Connection, asset_id: i64) -> Result<Vec<Tag>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT t.id, t.name FROM tags t INNER JOIN asset_tags at ON at.tag_id = t.id WHERE at.asset_id = ?1 ORDER BY t.name",
        )
        .map_err(SoundScoutError::Database)?;
    let rows = stmt
        .query_map(params![asset_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(SoundScoutError::Database)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(SoundScoutError::Database)?);
    }
    Ok(out)
}

pub fn get_all_tags(conn: &Connection) -> Result<Vec<TagWithCount>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT t.id, t.name, COUNT(at.asset_id) FROM tags t LEFT JOIN asset_tags at ON at.tag_id = t.id GROUP BY t.id ORDER BY t.name",
        )
        .map_err(SoundScoutError::Database)?;
    let rows = stmt.query_map([], |row| {
        Ok(TagWithCount {
            id: row.get(0)?,
            name: row.get(1)?,
            count: row.get::<_, i64>(2)? as u64,
        })
    }).map_err(SoundScoutError::Database)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(SoundScoutError::Database)?);
    }
    Ok(out)
}

pub fn bulk_add_tag(conn: &Connection, asset_ids: &[i64], tag_name: &str) -> Result<(), SoundScoutError> {
    if asset_ids.is_empty() {
        return Ok(());
    }
    let tx = conn.unchecked_transaction().map_err(SoundScoutError::Database)?;
    for &id in asset_ids {
        add_tag(&tx, id, tag_name)?;
    }
    tx.commit().map_err(SoundScoutError::Database)?;
    Ok(())
}

pub fn bulk_set_favorite(conn: &Connection, asset_ids: &[i64], favorite: bool) -> Result<(), SoundScoutError> {
    if asset_ids.is_empty() {
        return Ok(());
    }
    let v: i64 = if favorite { 1 } else { 0 };
    let tx = conn.unchecked_transaction().map_err(SoundScoutError::Database)?;
    for &id in asset_ids {
        tx.execute("UPDATE assets SET favorite = ?1 WHERE id = ?2", params![v, id])
            .map_err(SoundScoutError::Database)?;
    }
    tx.commit().map_err(SoundScoutError::Database)?;
    Ok(())
}

pub fn bulk_set_rating(conn: &Connection, asset_ids: &[i64], rating: u8) -> Result<(), SoundScoutError> {
    if rating > 5 {
        return Err(SoundScoutError::Validation(
            "rating must be between 0 and 5".into(),
        ));
    }
    if asset_ids.is_empty() {
        return Ok(());
    }
    let tx = conn.unchecked_transaction().map_err(SoundScoutError::Database)?;
    for &id in asset_ids {
        tx.execute(
            "UPDATE assets SET rating = ?1 WHERE id = ?2",
            params![i64::from(rating), id],
        )
        .map_err(SoundScoutError::Database)?;
    }
    tx.commit().map_err(SoundScoutError::Database)?;
    Ok(())
}

pub fn count_embeddings(conn: &Connection) -> Result<i64, SoundScoutError> {
    let mid = crate::embedding::TEXT_EMBEDDING_MODEL_ID;
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM embeddings WHERE model_id = ?1",
            rusqlite::params![mid],
            |r| r.get(0),
        )
        .map_err(SoundScoutError::Database)?;
    Ok(n)
}

pub fn count_all_assets(conn: &Connection) -> Result<i64, SoundScoutError> {
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM assets", [], |r| r.get(0))
        .map_err(SoundScoutError::Database)?;
    Ok(n)
}

pub fn list_all_asset_ids(conn: &Connection) -> Result<Vec<i64>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached("SELECT id FROM assets ORDER BY id")
        .map_err(SoundScoutError::Database)?;
    let rows = stmt.query_map([], |r| r.get(0)).map_err(SoundScoutError::Database)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(SoundScoutError::Database)?);
    }
    Ok(out)
}

/// Insert or replace a text embedding row (`vector` is `dim` little-endian `f32`s).
pub fn upsert_text_embedding(
    conn: &Connection,
    asset_id: i64,
    model_id: &str,
    vector: &[f32],
) -> Result<(), SoundScoutError> {
    let dim = crate::embedding::expected_dim();
    if vector.len() != dim {
        return Err(SoundScoutError::Validation(format!(
            "embedding dim {} != {}",
            vector.len(),
            dim
        )));
    }
    let bytes: Vec<u8> = bytemuck::cast_slice(vector).to_vec();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO embeddings (asset_id, vector, model_id, created_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(asset_id) DO UPDATE SET vector = excluded.vector, model_id = excluded.model_id, created_at = excluded.created_at",
        rusqlite::params![asset_id, bytes, model_id, now],
    )
    .map_err(SoundScoutError::Database)?;
    Ok(())
}

pub fn semantic_status(conn: &Connection) -> Result<SemanticSearchStatus, SoundScoutError> {
    let embedding_count = count_embeddings(conn)?;
    let asset_count = count_all_assets(conn)?;
    let has_vectors = embedding_count > 0;
    Ok(SemanticSearchStatus {
        embedding_count,
        asset_count,
        semantic_enabled: has_vectors,
        clap_pipeline_ready: has_vectors,
    })
}

/// Heuristic "similar" assets (same folder + extension) until CLAP nearest-neighbor exists.
/// Distinct non-empty categories from assets in the same folder (excludes `asset_id`).
pub fn suggest_categories_for_asset(conn: &Connection, asset_id: i64) -> Result<Vec<String>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT DISTINCT category FROM assets
             WHERE folder = (SELECT folder FROM assets WHERE id = ?1)
               AND category IS NOT NULL AND TRIM(category) != ''
               AND id != ?1
             ORDER BY category COLLATE NOCASE
             LIMIT 16",
        )
        .map_err(SoundScoutError::Database)?;
    let rows = stmt
        .query_map(params![asset_id], |row| row.get::<_, String>(0))
        .map_err(SoundScoutError::Database)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(SoundScoutError::Database)?);
    }
    Ok(out)
}

pub fn get_similar_assets_heuristic(
    conn: &Connection,
    asset_id: i64,
    limit: u32,
) -> Result<Vec<Asset>, SoundScoutError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT a.id, a.path, a.filename, a.extension, a.folder, a.duration_ms, a.sample_rate, a.channels, a.bit_depth, a.file_size, a.category, a.publisher, a.favorite, a.rating, a.notes, a.play_count
             FROM assets a
             WHERE a.folder = (SELECT folder FROM assets WHERE id = ?1)
               AND a.extension = (SELECT extension FROM assets WHERE id = ?1)
               AND a.id != ?1
             ORDER BY RANDOM()
             LIMIT ?2",
        )
        .map_err(SoundScoutError::Database)?;
    let rows = stmt
        .query_map(params![asset_id, limit], |row| asset_from_row(row))
        .map_err(SoundScoutError::Database)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(SoundScoutError::Database)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::DbPool;

    fn sample_asset(path: &str, folder: &str) -> NewAsset {
        NewAsset {
            path: path.into(),
            filename: "f".into(),
            extension: "wav".into(),
            folder: folder.into(),
            duration_ms: Some(1000),
            sample_rate: Some(44_100),
            channels: Some(2),
            bit_depth: Some(16),
            file_size: 100,
            category: Some("c".into()),
            publisher: Some("p".into()),
            modified_at: 1,
            indexed_at: 2,
            peaks: Some(vec![0u8; 8]),
        }
    }

    #[test]
    fn insert_and_retrieve_by_id() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        let a = sample_asset("/a/x.wav", "/a");
        assert_eq!(insert_asset_batch(&conn, &[a]).expect("ins"), 1);
        let row = get_asset_by_path(&conn, "/a/x.wav").expect("q").expect("some");
        let by_id = get_asset_by_id(&conn, row.id).expect("q2").expect("some");
        assert_eq!(by_id.path, "/a/x.wav");
    }

    #[test]
    fn insert_and_retrieve_by_path() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/p.wav", "/")]).expect("ins");
        assert!(get_asset_by_path(&conn, "/p.wav").expect("q").is_some());
    }

    #[test]
    fn insert_batch_returns_correct_count() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        let batch = vec![
            sample_asset("/1.wav", "/"),
            sample_asset("/2.wav", "/"),
        ];
        assert_eq!(insert_asset_batch(&conn, &batch).expect("ins"), 2);
    }

    #[test]
    fn insert_duplicate_path_is_ignored() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        let a = sample_asset("/d.wav", "/");
        assert_eq!(insert_asset_batch(&conn, &[a.clone()]).expect("1"), 1);
        assert_eq!(insert_asset_batch(&conn, &[a]).expect("2"), 0);
    }

    #[test]
    fn update_asset_metadata_changes_fields() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/u.wav", "/")]).expect("ins");
        let mut u = sample_asset("/u.wav", "/");
        u.file_size = 999;
        update_asset_metadata(&conn, "/u.wav", &u).expect("up");
        let a = get_asset_by_path(&conn, "/u.wav").expect("q").expect("row");
        assert_eq!(a.file_size, 999);
    }

    #[test]
    fn batch_insert_1000_under_500ms() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        let batch: Vec<NewAsset> = (0..1000)
            .map(|i| sample_asset(&format!("/many/{i}.wav"), "/many"))
            .collect();
        let t = std::time::Instant::now();
        insert_asset_batch(&conn, &batch).expect("ins");
        assert!(t.elapsed().as_millis() < 500);
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        assert!(get_asset_by_id(&conn, 999).expect("q").is_none());
    }

    #[test]
    fn update_favorite_toggles() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/f.wav", "/")]).expect("ins");
        let id = get_asset_by_path(&conn, "/f.wav").expect("q").expect("r").id;
        update_favorite(&conn, id, true).expect("fav");
        assert!(get_asset_by_id(&conn, id).expect("q").expect("r").favorite);
    }

    #[test]
    fn update_rating_valid_range() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/r.wav", "/")]).expect("ins");
        let id = get_asset_by_path(&conn, "/r.wav").expect("q").expect("r").id;
        update_rating(&conn, id, 4).expect("ok");
        assert_eq!(get_asset_by_id(&conn, id).expect("q").expect("r").rating, 4);
    }

    #[test]
    fn update_rating_rejects_over_5() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/x.wav", "/")]).expect("ins");
        let id = get_asset_by_path(&conn, "/x.wav").expect("q").expect("r").id;
        assert!(update_rating(&conn, id, 6).is_err());
    }

    #[test]
    fn delete_by_ids_removes_correct_rows() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(
            &conn,
            &[sample_asset("/d1.wav", "/"), sample_asset("/d2.wav", "/")],
        )
        .expect("ins");
        let id1 = get_asset_by_path(&conn, "/d1.wav").expect("q").expect("r").id;
        delete_assets_by_ids(&conn, &[id1]).expect("del");
        assert!(get_asset_by_path(&conn, "/d1.wav").expect("q").is_none());
    }

    #[test]
    fn delete_by_ids_returns_count() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/z.wav", "/")]).expect("ins");
        let id = get_asset_by_path(&conn, "/z.wav").expect("q").expect("r").id;
        assert_eq!(delete_assets_by_ids(&conn, &[id]).expect("n"), 1);
    }

    #[test]
    fn wipe_library_clears_assets_tags_and_fts() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(
            &conn,
            &[sample_asset("/w1.wav", "/lib"), sample_asset("/w2.wav", "/lib")],
        )
        .expect("ins");
        let id = get_asset_by_path(&conn, "/w1.wav").expect("q").expect("r").id;
        add_tag(&conn, id, "keepme").expect("tag");
        wipe_library_data(&conn).expect("wipe");
        let n_assets: i64 = conn
            .query_row("SELECT COUNT(*) FROM assets", [], |r| r.get(0))
            .expect("c");
        let n_tags: i64 = conn
            .query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))
            .expect("c");
        let n_fts: i64 = conn
            .query_row("SELECT COUNT(*) FROM assets_fts", [], |r| r.get(0))
            .expect("c");
        assert_eq!(n_assets, 0);
        assert_eq!(n_tags, 0);
        assert_eq!(n_fts, 0);
    }

    #[test]
    fn get_indexed_paths_returns_all_entries() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/i.wav", "/")]).expect("ins");
        let m = get_indexed_paths_with_mtime(&conn).expect("m");
        assert!(m.contains_key("/i.wav"));
    }

    #[test]
    fn get_folder_tree_groups_correctly() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(
            &conn,
            &[
                sample_asset("/lib/a/1.wav", "/lib/a"),
                sample_asset("/lib/a/2.wav", "/lib/a"),
            ],
        )
        .expect("ins");
        let flat = get_folder_tree(&conn).expect("f");
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].1, 2);
    }

    #[test]
    fn build_folder_tree_creates_nested_structure() {
        let flat = vec![
            ("/lib/a".into(), 2u64),
            ("/lib/a/b".into(), 1u64),
        ];
        let tree = build_folder_tree(&flat);
        assert!(!tree.is_empty());
    }

    #[test]
    fn get_publishers_groups_correctly() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/1.wav", "/")]).expect("ins");
        let p = get_publishers(&conn).expect("p");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].0, "p");
    }

    #[test]
    fn get_filter_options_reflects_data() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/1.wav", "/")]).expect("ins");
        let f = get_filter_options(&conn).expect("f");
        assert!(f.extensions.contains(&"wav".to_string()));
    }

    #[test]
    fn get_assets_by_folder_filters_correctly() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(
            &conn,
            &[
                sample_asset("/x/a.wav", "/x"),
                sample_asset("/y/b.wav", "/y"),
            ],
        )
        .expect("ins");
        let v = get_assets_by_folder(&conn, "/x", 50, 0).expect("q");
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn add_tag_creates_tag_and_association() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/t.wav", "/")]).expect("ins");
        let id = get_asset_by_path(&conn, "/t.wav").expect("q").expect("r").id;
        add_tag(&conn, id, "sfx").expect("tag");
        let tags = get_tags_for_asset(&conn, id).expect("tags");
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn remove_tag_deletes_association() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/t.wav", "/")]).expect("ins");
        let id = get_asset_by_path(&conn, "/t.wav").expect("q").expect("r").id;
        add_tag(&conn, id, "sfx").expect("tag");
        let tid = get_tags_for_asset(&conn, id).expect("tags")[0].id;
        remove_tag(&conn, id, tid).expect("rm");
        assert!(get_tags_for_asset(&conn, id).expect("tags").is_empty());
    }

    #[test]
    fn get_tags_for_asset_returns_correct_tags() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/t.wav", "/")]).expect("ins");
        let id = get_asset_by_path(&conn, "/t.wav").expect("q").expect("r").id;
        add_tag(&conn, id, "a").expect("a");
        add_tag(&conn, id, "b").expect("b");
        assert_eq!(get_tags_for_asset(&conn, id).expect("t").len(), 2);
    }

    #[test]
    fn get_all_tags_returns_counts() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(
            &conn,
            &[sample_asset("/a.wav", "/"), sample_asset("/b.wav", "/")],
        )
        .expect("ins");
        let ida = get_asset_by_path(&conn, "/a.wav").expect("q").expect("r").id;
        let idb = get_asset_by_path(&conn, "/b.wav").expect("q").expect("r").id;
        add_tag(&conn, ida, "shared").expect("t");
        add_tag(&conn, idb, "shared").expect("t");
        let all = get_all_tags(&conn).expect("all");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].count, 2);
    }

    #[test]
    fn update_peaks_and_get_peaks() {
        let pool = DbPool::new_in_memory().expect("pool");
        let conn = pool.get().expect("c");
        insert_asset_batch(&conn, &[sample_asset("/pk.wav", "/")]).expect("ins");
        let id = get_asset_by_path(&conn, "/pk.wav").expect("q").expect("r").id;
        update_peaks(&conn, id, &[1, 2, 3, 4]).expect("up");
        let b = get_peaks(&conn, id).expect("g").expect("blob");
        assert_eq!(b, vec![1, 2, 3, 4]);
    }
}
