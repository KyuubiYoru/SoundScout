//! Concatenate filename, path, metadata, and tags into one string for text embedding.

use rusqlite::Connection;

use crate::db::models::Asset;
use crate::db::queries;
use crate::error::SoundScoutError;

/// Build a normalized document for `asset_id` (for embedding). Returns `None` if asset missing.
pub fn asset_text_for_embedding(conn: &Connection, asset_id: i64) -> Result<Option<String>, SoundScoutError> {
    let Some(asset) = queries::get_asset_by_id(conn, asset_id)? else {
        return Ok(None);
    };
    Ok(Some(asset_document(&asset, conn, asset_id)?))
}

pub fn asset_document(asset: &Asset, conn: &Connection, asset_id: i64) -> Result<String, SoundScoutError> {
    let tags = queries::get_tags_for_asset(conn, asset_id)?;
    let tag_part = tags
        .into_iter()
        .map(|t| t.name)
        .collect::<Vec<_>>()
        .join(" ");

    let folder_tokens: String = asset
        .folder
        .split(['/', '\\'])
        .flat_map(|seg| seg.split(['_', '-', ' ']))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let name_tokens: String = asset
        .filename
        .split(['_', '-', ' ', '.'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let mut parts: Vec<&str> = Vec::new();
    if !asset.filename.is_empty() {
        parts.push(asset.filename.as_str());
    }
    if !name_tokens.is_empty() && name_tokens != asset.filename {
        parts.push(name_tokens.as_str());
    }
    if !asset.folder.is_empty() {
        parts.push(asset.folder.as_str());
    }
    if !folder_tokens.is_empty() {
        parts.push(folder_tokens.as_str());
    }
    if let Some(c) = asset.category.as_deref() {
        let c = c.trim();
        if !c.is_empty() {
            parts.push(c);
        }
    }
    if let Some(p) = asset.publisher.as_deref() {
        let p = p.trim();
        if !p.is_empty() {
            parts.push(p);
        }
    }
    if let Some(n) = asset.notes.as_deref() {
        let n = n.trim();
        if !n.is_empty() {
            parts.push(n);
        }
    }
    if !tag_part.is_empty() {
        parts.push(tag_part.as_str());
    }

    let doc = parts.join(" ");
    Ok(doc.split_whitespace().collect::<Vec<_>>().join(" "))
}
