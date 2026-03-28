//! Vector and hybrid search over stored text embeddings.

use std::cmp::Ordering;
use std::collections::HashMap;

use rusqlite::{types::Value, Connection, Row};

use crate::db::models::{Asset, SearchQuery, SearchResults, SortDirection, SortField};
use crate::db::queries;
use crate::embedding;
use crate::error::SoundScoutError;
use crate::search::query_builder::{filter_suffix, sanitize_fts_query};

fn row_to_asset(row: &Row<'_>) -> Result<Asset, rusqlite::Error> {
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

fn blob_to_f32(blob: &[u8]) -> Result<Vec<f32>, SoundScoutError> {
    bytemuck::try_cast_slice(blob)
        .map(|s| s.to_vec())
        .map_err(|_| SoundScoutError::Validation("invalid embedding blob length".into()))
}

fn l2_normalize(v: &mut [f32]) {
    let s: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if s > 1e-8 {
        for x in v.iter_mut() {
            *x /= s;
        }
    }
}

fn load_fts_bm25(
    conn: &Connection,
    query: &SearchQuery,
    trimmed: &str,
    use_fts: bool,
    use_like: bool,
) -> Result<HashMap<i64, f32>, SoundScoutError> {
    let mut m = HashMap::new();
    let (filt_sql, filt_params) = filter_suffix(query);
    if use_fts {
        let mut sql = String::from(
            "SELECT a.id, bm25(assets_fts) FROM assets a INNER JOIN assets_fts ON assets_fts.rowid = a.id WHERE assets_fts MATCH ?",
        );
        sql.push_str(&filt_sql);
        let mut params: Vec<Value> = vec![Value::Text(trimmed.to_string())];
        params.extend(filt_params);
        let mut stmt = conn.prepare(&sql).map_err(SoundScoutError::Database)?;
        let mut rows = stmt
            .query(rusqlite::params_from_iter(params))
            .map_err(SoundScoutError::Database)?;
        while let Some(row) = rows.next().map_err(SoundScoutError::Database)? {
            let id: i64 = row.get(0).map_err(SoundScoutError::Database)?;
            let bm: f64 = row.get(1).map_err(SoundScoutError::Database)?;
            m.insert(id, bm as f32);
        }
    } else if use_like {
        let mut sql = String::from("SELECT a.id FROM assets a WHERE a.filename LIKE ?");
        sql.push_str(&filt_sql);
        let mut params: Vec<Value> = vec![Value::Text(format!("%{trimmed}%"))];
        params.extend(filt_params);
        let mut stmt = conn.prepare(&sql).map_err(SoundScoutError::Database)?;
        let mut rows = stmt
            .query(rusqlite::params_from_iter(params))
            .map_err(SoundScoutError::Database)?;
        while let Some(row) = rows.next().map_err(SoundScoutError::Database)? {
            let id: i64 = row.get(0).map_err(SoundScoutError::Database)?;
            m.insert(id, 0.0f32);
        }
    }
    Ok(m)
}

fn fts_quality_for_id(
    id: i64,
    fts_bm: &HashMap<i64, f32>,
    min_bm: f32,
    max_bm: f32,
    use_like_hits: bool,
) -> f32 {
    let Some(&bm) = fts_bm.get(&id) else {
        return 0.0;
    };
    if use_like_hits {
        return 1.0;
    }
    if (max_bm - min_bm).abs() < 1e-8 {
        return 1.0;
    }
    // bm25 lower is better → higher quality when closer to min_bm
    (max_bm - bm) / (max_bm - min_bm + 1e-8)
}

fn sort_scored(scored: &mut [(Asset, f32)], query: &SearchQuery) {
    scored.sort_by(|(a, sa), (b, sb)| {
        let sim = sb.partial_cmp(sa).unwrap_or(Ordering::Equal);
        if sim != Ordering::Equal {
            return sim;
        }
        match query.sort_by {
            SortField::Name => {
                let ord = a.filename.cmp(&b.filename);
                if query.sort_dir == SortDirection::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            }
            SortField::Duration => {
                let da = a.duration_ms.unwrap_or(i64::MIN);
                let db = b.duration_ms.unwrap_or(i64::MIN);
                let ord = da.cmp(&db);
                if query.sort_dir == SortDirection::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            }
            _ => Ordering::Equal,
        }
    });
}

/// Cosine similarity search over `embeddings` rows matching `model_id` and asset filters.
pub fn execute_search_vector(conn: &Connection, query: &SearchQuery) -> Result<SearchResults, SoundScoutError> {
    let qtxt = query.text.trim();
    if queries::count_embeddings(conn)? == 0 {
        return Ok(SearchResults {
            assets: vec![],
            total: 0,
            offset: query.offset,
            relevance_scores: None,
        });
    }

    let mut qv = embedding::embed_batch(&[qtxt.to_string()])?
        .pop()
        .ok_or_else(|| SoundScoutError::Embedding("no query vector".into()))?;
    l2_normalize(&mut qv);

    let model = embedding::TEXT_EMBEDDING_MODEL_ID;
    let (filt_sql, filt_params) = filter_suffix(query);
    let mut sql = String::from(
        "SELECT a.id, a.path, a.filename, a.extension, a.folder, a.duration_ms, a.sample_rate, a.channels, a.bit_depth, a.file_size, a.category, a.publisher, a.favorite, a.rating, a.notes, a.play_count, e.vector FROM assets a INNER JOIN embeddings e ON e.asset_id = a.id WHERE e.model_id = ?",
    );
    sql.push_str(&filt_sql);
    let mut params: Vec<Value> = vec![Value::Text(model.to_string())];
    params.extend(filt_params);

    let mut stmt = conn.prepare(&sql).map_err(SoundScoutError::Database)?;
    let mut rows = stmt
        .query(rusqlite::params_from_iter(params))
        .map_err(SoundScoutError::Database)?;

    let mut scored: Vec<(Asset, f32)> = Vec::new();
    while let Some(row) = rows.next().map_err(SoundScoutError::Database)? {
        let asset = row_to_asset(&row).map_err(SoundScoutError::Database)?;
        let blob: Vec<u8> = row.get(16).map_err(SoundScoutError::Database)?;
        let mut v = blob_to_f32(&blob)?;
        l2_normalize(&mut v);
        let sim: f32 = qv.iter().zip(v.iter()).map(|(x, y)| x * y).sum();
        scored.push((asset, sim));
    }

    sort_scored(&mut scored, query);
    let total = scored.len() as u64;
    let start = query.offset as usize;
    let end = start.saturating_add(query.limit as usize).min(scored.len());
    let (assets, relevance_scores) = if start < scored.len() {
        let page = &scored[start..end];
        let assets: Vec<Asset> = page.iter().map(|(a, _)| a.clone()).collect();
        let relevance_scores: Vec<f32> = page.iter().map(|(_, s)| *s).collect();
        (assets, Some(relevance_scores))
    } else {
        (vec![], Some(vec![]))
    };

    Ok(SearchResults {
        assets,
        total,
        offset: query.offset,
        relevance_scores,
    })
}

/// Weighted merge of vector similarity and lexical FTS / LIKE (35% / 65% vector-first).
pub fn execute_search_hybrid(conn: &Connection, query: &SearchQuery) -> Result<SearchResults, SoundScoutError> {
    let sanitized = sanitize_fts_query(&query.text);
    let trimmed = sanitized.trim();
    let use_fts = !trimmed.is_empty() && trimmed.chars().count() >= 3;
    let use_like = !trimmed.is_empty() && trimmed.chars().count() < 3;

    if queries::count_embeddings(conn)? == 0 {
        return Ok(SearchResults {
            assets: vec![],
            total: 0,
            offset: query.offset,
            relevance_scores: None,
        });
    }

    let fts_bm = load_fts_bm25(conn, query, trimmed, use_fts, use_like)?;
    let bm_vals: Vec<f32> = fts_bm.values().copied().collect();
    let min_bm = bm_vals.iter().copied().fold(f32::INFINITY, f32::min);
    let max_bm = bm_vals.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    let mut qv = embedding::embed_batch(&[trimmed.to_string()])?
        .pop()
        .ok_or_else(|| SoundScoutError::Embedding("no query vector".into()))?;
    l2_normalize(&mut qv);

    let model = embedding::TEXT_EMBEDDING_MODEL_ID;
    let (filt_sql, filt_params) = filter_suffix(query);
    let mut sql = String::from(
        "SELECT a.id, a.path, a.filename, a.extension, a.folder, a.duration_ms, a.sample_rate, a.channels, a.bit_depth, a.file_size, a.category, a.publisher, a.favorite, a.rating, a.notes, a.play_count, e.vector FROM assets a INNER JOIN embeddings e ON e.asset_id = a.id WHERE e.model_id = ?",
    );
    sql.push_str(&filt_sql);
    let mut params: Vec<Value> = vec![Value::Text(model.to_string())];
    params.extend(filt_params);

    let mut stmt = conn.prepare(&sql).map_err(SoundScoutError::Database)?;
    let mut rows = stmt
        .query(rusqlite::params_from_iter(params))
        .map_err(SoundScoutError::Database)?;

    const W_VEC: f32 = 0.65;
    const W_FTS: f32 = 0.35;

    let mut scored: Vec<(Asset, f32)> = Vec::new();
    while let Some(row) = rows.next().map_err(SoundScoutError::Database)? {
        let asset = row_to_asset(&row).map_err(SoundScoutError::Database)?;
        let blob: Vec<u8> = row.get(16).map_err(SoundScoutError::Database)?;
        let mut v = blob_to_f32(&blob)?;
        l2_normalize(&mut v);
        let sim: f32 = qv.iter().zip(v.iter()).map(|(x, y)| x * y).sum();
        let vec_n = ((sim + 1.0) * 0.5).clamp(0.0, 1.0);
        let fts_n = fts_quality_for_id(asset.id, &fts_bm, min_bm, max_bm, use_like && !use_fts);
        let combined = W_VEC * vec_n + W_FTS * fts_n;
        scored.push((asset, combined));
    }

    sort_scored(&mut scored, query);
    let total = scored.len() as u64;
    let start = query.offset as usize;
    let end = start.saturating_add(query.limit as usize).min(scored.len());
    let (assets, relevance_scores) = if start < scored.len() {
        let page = &scored[start..end];
        let assets: Vec<Asset> = page.iter().map(|(a, _)| a.clone()).collect();
        let relevance_scores: Vec<f32> = page.iter().map(|(_, s)| *s).collect();
        (assets, Some(relevance_scores))
    } else {
        (vec![], Some(vec![]))
    };

    Ok(SearchResults {
        assets,
        total,
        offset: query.offset,
        relevance_scores,
    })
}
