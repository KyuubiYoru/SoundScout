//! FTS5 / `LIKE` SQL construction for [`SearchQuery`](crate::db::models::SearchQuery).

use rusqlite::types::Value;

use crate::db::models::{SearchQuery, SortDirection, SortField};
use crate::search::constants::FTS_MIN_QUERY_CHARS;

/// Strip characters and tokens unsafe for FTS5 / our simplified query model.
pub fn sanitize_fts_query(input: &str) -> String {
    let mut s = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '(' | ')' | ':' | '^' | '*' | '"' => s.push(' '),
            other => s.push(other),
        }
    }
    let mut out = s.to_lowercase();
    for word in [" and ", " or ", " not "] {
        while let Some(i) = out.find(word) {
            out.replace_range(i..i + word.len(), " ");
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn append_filters(sql: &mut String, params: &mut Vec<Value>, query: &SearchQuery) {
    if let Some(exts) = &query.extensions {
        if !exts.is_empty() {
            let ph = exts.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            sql.push_str(&format!(" AND a.extension IN ({ph})"));
            for e in exts {
                params.push(Value::Text(e.clone()));
            }
        }
    }
    if let Some(min) = query.duration_min {
        sql.push_str(" AND (a.duration_ms IS NOT NULL AND a.duration_ms >= ?)");
        params.push(Value::Integer(min));
    }
    if let Some(max) = query.duration_max {
        sql.push_str(" AND (a.duration_ms IS NOT NULL AND a.duration_ms <= ?)");
        params.push(Value::Integer(max));
    }
    if let Some(rates) = &query.sample_rates {
        if !rates.is_empty() {
            let ph = rates.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            sql.push_str(&format!(" AND a.sample_rate IN ({ph})"));
            for r in rates {
                params.push(Value::Integer(i64::from(*r)));
            }
        }
    }
    if let Some(ch) = query.channels {
        sql.push_str(" AND a.channels = ?");
        params.push(Value::Integer(i64::from(ch)));
    }
    if query.favorites_only {
        sql.push_str(" AND a.favorite = 1");
    }
    if let Some(pub_name) = &query.publisher {
        sql.push_str(" AND a.publisher = ?");
        params.push(Value::Text(pub_name.clone()));
    }
    if let Some(tags) = &query.tags {
        if !tags.is_empty() {
            let placeholders = tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            sql.push_str(&format!(
                " AND a.id IN (SELECT at.asset_id FROM asset_tags at INNER JOIN tags t ON t.id = at.tag_id WHERE t.name IN ({placeholders}))"
            ));
            for t in tags {
                params.push(Value::Text(t.clone()));
            }
        }
    }
    if let Some(root) = &query.folder_root {
        if root.is_empty() {
            sql.push_str(" AND 1=0");
        } else if root != "/" {
            sql.push_str(
                " AND (a.folder = ? OR (LENGTH(a.folder) > LENGTH(?) AND SUBSTR(a.folder, 1, LENGTH(?)) = ? AND SUBSTR(a.folder, LENGTH(?) + 1, 1) = '/'))",
            );
            for _ in 0..5 {
                params.push(Value::Text(root.clone()));
            }
        }
    }
}

/// Build `(sql, params)` for the result page.
pub fn build_search_sql(query: &SearchQuery) -> (String, Vec<Value>) {
    let mut params: Vec<Value> = Vec::new();
    let sanitized = sanitize_fts_query(&query.text);
    let trimmed = sanitized.trim();
    let char_count = trimmed.chars().count();
    let use_fts = !trimmed.is_empty() && char_count >= FTS_MIN_QUERY_CHARS;
    let use_like = !trimmed.is_empty() && char_count < FTS_MIN_QUERY_CHARS;

    let mut sql = String::from(
        "SELECT a.id, a.path, a.filename, a.extension, a.folder, a.duration_ms, a.sample_rate, a.channels, a.bit_depth, a.file_size, a.category, a.publisher, a.favorite, a.rating, a.notes, a.play_count FROM assets a",
    );

    if use_fts {
        sql.push_str(
            " INNER JOIN assets_fts ON assets_fts.rowid = a.id WHERE assets_fts MATCH ?",
        );
        params.push(Value::Text(trimmed.to_string()));
    } else if use_like {
        sql.push_str(" WHERE a.filename LIKE ?");
        params.push(Value::Text(format!("%{trimmed}%")));
    } else {
        sql.push_str(" WHERE 1=1");
    }

    append_filters(&mut sql, &mut params, query);

    match query.sort_by {
        SortField::Relevance if use_fts => {
            sql.push_str(" ORDER BY bm25(assets_fts)");
            if query.sort_dir == SortDirection::Asc {
                sql.push_str(" ASC");
            } else {
                sql.push_str(" DESC");
            }
        }
        SortField::Name => {
            sql.push_str(" ORDER BY a.filename ");
            sql.push_str(if query.sort_dir == SortDirection::Asc {
                "ASC"
            } else {
                "DESC"
            });
        }
        SortField::Duration => {
            sql.push_str(" ORDER BY a.duration_ms ");
            sql.push_str(if query.sort_dir == SortDirection::Asc {
                "ASC NULLS LAST"
            } else {
                "DESC NULLS LAST"
            });
        }
        SortField::Date => {
            sql.push_str(" ORDER BY a.indexed_at ");
            sql.push_str(if query.sort_dir == SortDirection::Asc {
                "ASC"
            } else {
                "DESC"
            });
        }
        SortField::Relevance => {
            sql.push_str(" ORDER BY a.filename ASC");
        }
    }

    sql.push_str(" LIMIT ? OFFSET ?");
    params.push(Value::Integer(i64::from(query.limit)));
    params.push(Value::Integer(i64::from(query.offset)));

    (sql, params)
}

/// Count rows matching filters (no order/limit).
pub fn build_count_sql(query: &SearchQuery) -> (String, Vec<Value>) {
    let mut params: Vec<Value> = Vec::new();
    let sanitized = sanitize_fts_query(&query.text);
    let trimmed = sanitized.trim();
    let char_count = trimmed.chars().count();
    let use_fts = !trimmed.is_empty() && char_count >= FTS_MIN_QUERY_CHARS;
    let use_like = !trimmed.is_empty() && char_count < FTS_MIN_QUERY_CHARS;

    let mut sql = String::from("SELECT COUNT(*) FROM assets a");
    if use_fts {
        sql.push_str(
            " INNER JOIN assets_fts ON assets_fts.rowid = a.id WHERE assets_fts MATCH ?",
        );
        params.push(Value::Text(trimmed.to_string()));
    } else if use_like {
        sql.push_str(" WHERE a.filename LIKE ?");
        params.push(Value::Text(format!("%{trimmed}%")));
    } else {
        sql.push_str(" WHERE 1=1");
    }
    append_filters(&mut sql, &mut params, query);
    (sql, params)
}

/// `AND ...` filter fragment plus params (for composing with `WHERE 1=1` or `WHERE match...`).
pub fn filter_suffix(query: &SearchQuery) -> (String, Vec<Value>) {
    let mut sql = String::new();
    let mut params: Vec<Value> = Vec::new();
    append_filters(&mut sql, &mut params, query);
    (sql, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_removes_fts_operators() {
        let s = sanitize_fts_query("(foo):bar^baz*\"x\"");
        assert!(!s.contains('('));
        assert!(!s.contains(':'));
    }

    #[test]
    fn sanitize_does_not_add_prefix_star() {
        let s = sanitize_fts_query("foot");
        assert!(!s.ends_with('*'));
    }

    #[test]
    fn sanitize_empty_returns_empty() {
        assert_eq!(sanitize_fts_query("   "), "");
    }

    #[test]
    fn sanitize_handles_multiple_spaces() {
        assert_eq!(sanitize_fts_query("a    b"), "a b");
    }

    #[test]
    fn build_sql_text_3plus_chars_uses_fts() {
        let q = SearchQuery {
            text: "abc".into(),
            ..Default::default()
        };
        let (sql, _) = build_search_sql(&q);
        assert!(sql.contains("MATCH"));
    }

    #[test]
    fn build_sql_text_1_2_chars_uses_like_fallback() {
        let q = SearchQuery {
            text: "ab".into(),
            ..Default::default()
        };
        let (sql, _) = build_search_sql(&q);
        assert!(sql.contains("LIKE"));
    }

    #[test]
    fn build_sql_empty_text_skips_fts() {
        let q = SearchQuery::default();
        let (sql, _) = build_search_sql(&q);
        assert!(!sql.contains("MATCH"));
    }

    #[test]
    fn build_sql_with_extension_filter() {
        let q = SearchQuery {
            extensions: Some(vec!["wav".into()]),
            ..Default::default()
        };
        let (sql, p) = build_search_sql(&q);
        assert!(sql.contains("extension IN"));
        assert!(p.len() >= 3);
    }

    #[test]
    fn build_sql_with_duration_range() {
        let q = SearchQuery {
            duration_min: Some(0),
            duration_max: Some(5000),
            ..Default::default()
        };
        let (sql, _) = build_search_sql(&q);
        assert!(sql.contains("duration_ms"));
    }

    #[test]
    fn build_sql_with_favorites() {
        let q = SearchQuery {
            favorites_only: true,
            ..Default::default()
        };
        let (sql, _) = build_search_sql(&q);
        assert!(sql.contains("favorite = 1"));
    }

    #[test]
    fn build_sql_pagination() {
        let q = SearchQuery {
            offset: 10,
            limit: 5,
            ..Default::default()
        };
        let (sql, _) = build_search_sql(&q);
        assert!(sql.contains("LIMIT"));
        assert!(sql.contains("OFFSET"));
    }

    #[test]
    fn build_sql_with_folder_root_subtree() {
        let q = SearchQuery {
            folder_root: Some("/lib/a".into()),
            ..Default::default()
        };
        let (sql, p) = build_search_sql(&q);
        assert!(sql.contains("a.folder = ?"));
        assert!(sql.contains("SUBSTR(a.folder"));
        assert!(p.len() >= 5);
    }
}
