//! Derive publisher/category/filename from scan root + file path.

use std::path::{Component, Path};

use crate::db::models::PathMetadata;
use crate::db::queries::normalize_folder_key;

/// Parse [`PathMetadata`] relative to `root`.
pub fn parse_path(root: &Path, file_path: &Path) -> PathMetadata {
    let folder = normalize_folder_key(
        &file_path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );

    let rel = file_path.strip_prefix(root).unwrap_or(file_path);
    let rel_parent = rel.parent().unwrap_or_else(|| Path::new(""));

    let components: Vec<String> = rel_parent
        .components()
        .filter_map(|c| match c {
            Component::Normal(os) => Some(os.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();

    let (publisher, category) = match components.len() {
        0 => (None, None),
        1 => (Some(components[0].clone()), None),
        _ => {
            let pub_name = components[0].clone();
            let cat = components[1..].join(" > ");
            (Some(pub_name), Some(cat))
        }
    };

    let stem = file_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = file_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    PathMetadata {
        filename: stem,
        extension: ext,
        folder,
        publisher,
        category,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn extracts_publisher_from_first_segment() {
        let root = PathBuf::from("/audio");
        let file = PathBuf::from("/audio/Boom/hit.wav");
        let m = parse_path(&root, &file);
        assert_eq!(m.publisher.as_deref(), Some("Boom"));
    }

    #[test]
    fn extracts_category_from_intermediate_segments() {
        let root = PathBuf::from("/audio");
        let file = PathBuf::from("/audio/Boom/Impacts/Metal/hit.wav");
        let m = parse_path(&root, &file);
        assert_eq!(m.category.as_deref(), Some("Impacts > Metal"));
    }

    #[test]
    fn file_directly_in_root_has_no_publisher() {
        let root = PathBuf::from("/audio");
        let file = PathBuf::from("/audio/hit.wav");
        let m = parse_path(&root, &file);
        assert!(m.publisher.is_none());
        assert!(m.category.is_none());
    }

    #[test]
    fn file_one_level_deep_has_publisher_no_category() {
        let root = PathBuf::from("/audio");
        let file = PathBuf::from("/audio/Boom/hit.wav");
        let m = parse_path(&root, &file);
        assert_eq!(m.publisher.as_deref(), Some("Boom"));
        assert!(m.category.is_none());
    }

    #[test]
    fn deeply_nested_path_joins_all_middle_segments() {
        let root = PathBuf::from("/r");
        let file = PathBuf::from("/r/A/B/C/D/x.wav");
        let m = parse_path(&root, &file);
        assert_eq!(m.publisher.as_deref(), Some("A"));
        assert_eq!(m.category.as_deref(), Some("B > C > D"));
    }

    #[test]
    fn filename_strips_extension() {
        let m = parse_path(Path::new("/a"), Path::new("/a/b.wav"));
        assert_eq!(m.filename, "b");
    }

    #[test]
    fn extension_is_lowercase() {
        let m = parse_path(Path::new("/a"), Path::new("/a/b.WAV"));
        assert_eq!(m.extension, "wav");
    }

    #[test]
    fn handles_non_ascii_paths() {
        let root = PathBuf::from("/lib");
        let file = PathBuf::from("/lib/Ström Sounds/Björk.wav");
        let m = parse_path(&root, &file);
        assert_eq!(m.publisher.as_deref(), Some("Ström Sounds"));
        assert_eq!(m.filename, "Björk");
    }

    #[test]
    fn handles_spaces_and_parens() {
        let root = PathBuf::from("/lib");
        let file = PathBuf::from("/lib/Sound (Ideas)/SFX - Vol.1/bang!_01.wav");
        let m = parse_path(&root, &file);
        assert!(m.publisher.is_some());
        assert!(m.filename.contains("bang"));
    }

    #[test]
    fn handles_multiple_dots_in_filename() {
        let m = parse_path(Path::new("/a"), Path::new("/a/ambience.v2.final.wav"));
        assert_eq!(m.extension, "wav");
        assert_eq!(m.filename, "ambience.v2.final");
    }

    #[test]
    fn handles_no_extension() {
        let m = parse_path(Path::new("/a"), Path::new("/a/README"));
        assert_eq!(m.extension, "");
    }
}
