//! Run search SQL and assemble [`SearchResults`](crate::db::models::SearchResults).

use rusqlite::{params_from_iter, Connection};

use crate::db::models::{SearchMode, SearchQuery, SearchResults};
use crate::error::SoundScoutError;
use crate::search::query_builder;
use crate::search::vector;

fn execute_search_lexical(conn: &Connection, query: &SearchQuery) -> Result<SearchResults, SoundScoutError> {
    let (count_sql, count_params) = query_builder::build_count_sql(query);
    let total: i64 = conn
        .query_row(&count_sql, params_from_iter(count_params), |row| row.get(0))
        .map_err(SoundScoutError::Database)?;

    let (sql, params) = query_builder::build_search_sql(query);
    let mut stmt = conn.prepare(&sql).map_err(SoundScoutError::Database)?;
    let rows = stmt
        .query_map(params_from_iter(params), |row| {
            Ok(crate::db::models::Asset {
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
        })
        .map_err(SoundScoutError::Database)?;

    let mut assets = Vec::new();
    for r in rows {
        assets.push(r.map_err(SoundScoutError::Database)?);
    }

    Ok(SearchResults {
        assets,
        total: u64::try_from(total).unwrap_or(0),
        offset: query.offset,
        relevance_scores: None,
    })
}

/// Execute search; returns page + total count.
pub fn execute_search(conn: &Connection, query: &SearchQuery) -> Result<SearchResults, SoundScoutError> {
    let empty_text = query.text.trim().is_empty();

    match query.search_mode {
        SearchMode::Lexical => execute_search_lexical(conn, query),
        SearchMode::Vector => {
            if empty_text {
                execute_search_lexical(conn, query)
            } else {
                vector::execute_search_vector(conn, query)
            }
        }
        SearchMode::Both => {
            if empty_text {
                execute_search_lexical(conn, query)
            } else {
                vector::execute_search_hybrid(conn, query)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::DbPool;
    use crate::db::models::NewAsset;
    use crate::db::queries;

    fn seed(conn: &Connection) {
        queries::insert_asset_batch(
            conn,
            &[NewAsset {
                path: "/Footstep_Concrete.wav".into(),
                filename: "Footstep_Concrete".into(),
                extension: "wav".into(),
                folder: "/".into(),
                duration_ms: Some(2500),
                sample_rate: Some(48_000),
                channels: Some(2),
                bit_depth: Some(16),
                file_size: 100,
                category: Some("Footsteps".into()),
                publisher: Some("Boom".into()),
                modified_at: 0,
                indexed_at: 1,
                peaks: None,
            }],
        )
        .expect("ins");
    }

    #[test]
    fn search_finds_by_filename() {
        let pool = DbPool::new_in_memory().expect("p");
        let conn = pool.get().expect("c");
        seed(&conn);
        let q = SearchQuery {
            text: "Foot".into(),
            ..Default::default()
        };
        let r = execute_search(&conn, &q).expect("s");
        assert!(!r.assets.is_empty());
    }

    #[test]
    fn search_empty_query_returns_all() {
        let pool = DbPool::new_in_memory().expect("p");
        let conn = pool.get().expect("c");
        seed(&conn);
        let r = execute_search(&conn, &SearchQuery::default()).expect("s");
        assert_eq!(r.total, 1);
    }
}
