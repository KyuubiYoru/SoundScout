//! Recursive audio file discovery.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use walkdir::WalkDir;

use crate::error::SoundScoutError;

/// One file discovered on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub filename: String,
    pub extension: String,
    pub file_size: u64,
    pub modified_at: i64,
}

const AUDIO_EXTENSIONS: &[&str] = &["wav", "flac", "mp3", "ogg", "aiff", "aif"];

fn is_audio_ext(ext: &str) -> bool {
    let e = ext.to_ascii_lowercase();
    AUDIO_EXTENSIONS.iter().any(|&a| a == e.as_str())
}

/// Walk `root` and return audio files (sorted by path).
pub fn scan_directory(root: &Path) -> Result<Vec<ScannedFile>, SoundScoutError> {
    scan_directory_with_progress(root, true, &|_| {})
}

/// Same as [`scan_directory`] with optional hidden-dir skip and per-file progress (`bytes` scanned).
pub fn scan_directory_with_progress(
    root: &Path,
    skip_hidden: bool,
    progress: &dyn Fn(u64),
) -> Result<Vec<ScannedFile>, SoundScoutError> {
    if !root.exists() {
        return Err(SoundScoutError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "scan root does not exist",
        )));
    }

    let mut out = Vec::new();
    let walker = WalkDir::new(root).into_iter().filter_entry(move |e| {
        if !skip_hidden {
            return true;
        }
        if e.depth() == 0 {
            return true;
        }
        !e.file_name()
            .to_str()
            .is_some_and(|n| n.starts_with('.'))
    });

    for entry in walker {
        let entry = entry.map_err(|e| {
            SoundScoutError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("");
        if !is_audio_ext(ext) {
            continue;
        }
        let meta = entry.metadata().map_err(|e| {
            SoundScoutError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(0))
            .unwrap_or(0);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let extension = ext.to_ascii_lowercase();
        out.push(ScannedFile {
            path: path.to_path_buf(),
            filename: stem,
            extension,
            file_size: meta.len(),
            modified_at,
        });
        progress(meta.len());
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn finds_all_audio_files() {
        let dir = TempDir::new().expect("d");
        fs::write(dir.path().join("a.wav"), b"x").unwrap();
        fs::write(dir.path().join("b.flac"), b"x").unwrap();
        fs::write(dir.path().join("c.mp3"), b"x").unwrap();
        let v = scan_directory(dir.path()).expect("scan");
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn skips_non_audio_extensions() {
        let dir = TempDir::new().expect("d");
        fs::write(dir.path().join("a.txt"), b"x").unwrap();
        fs::write(dir.path().join("b.jpg"), b"x").unwrap();
        assert!(scan_directory(dir.path()).expect("scan").is_empty());
    }

    #[test]
    fn skips_hidden_directories() {
        let dir = TempDir::new().expect("d");
        let hidden = dir.path().join(".hidden");
        fs::create_dir_all(&hidden).unwrap();
        fs::write(hidden.join("s.wav"), b"x").unwrap();
        let v = scan_directory(dir.path()).expect("scan");
        assert!(v.is_empty());
    }

    #[test]
    fn returns_correct_file_size() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("z.wav");
        fs::write(&p, vec![0u8; 123]).unwrap();
        let v = scan_directory(dir.path()).expect("scan");
        assert_eq!(v[0].file_size, 123);
    }

    #[test]
    fn returns_nonzero_modified_at() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("z.wav");
        fs::write(&p, b"x").unwrap();
        let v = scan_directory(dir.path()).expect("scan");
        assert!(v[0].modified_at > 0);
    }

    #[test]
    fn empty_directory_returns_empty_vec() {
        let dir = TempDir::new().expect("d");
        assert!(scan_directory(dir.path()).expect("scan").is_empty());
    }

    #[test]
    fn nonexistent_root_returns_error() {
        assert!(scan_directory(Path::new("/nonexistent-soundscout-xyz")).is_err());
    }

    #[test]
    fn recognizes_all_supported_extensions() {
        let dir = TempDir::new().expect("d");
        for ext in ["wav", "flac", "mp3", "ogg", "aiff", "aif"] {
            fs::write(dir.path().join(format!("x.{ext}")), b"1").unwrap();
        }
        assert_eq!(scan_directory(dir.path()).expect("scan").len(), 6);
    }

    #[test]
    fn extension_matching_is_case_insensitive() {
        let dir = TempDir::new().expect("d");
        fs::write(dir.path().join("A.WAV"), b"x").unwrap();
        fs::write(dir.path().join("B.Flac"), b"x").unwrap();
        assert_eq!(scan_directory(dir.path()).expect("scan").len(), 2);
    }

    #[test]
    fn results_are_sorted_by_path() {
        let dir = TempDir::new().expect("d");
        fs::write(dir.path().join("b.wav"), b"x").unwrap();
        fs::write(dir.path().join("a.wav"), b"x").unwrap();
        let v = scan_directory(dir.path()).expect("scan");
        assert!(v[0].path.to_string_lossy() < v[1].path.to_string_lossy());
    }
}
