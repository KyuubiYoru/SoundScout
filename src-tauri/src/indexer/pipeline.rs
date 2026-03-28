//! Full index pass over one scan root.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rayon::prelude::*;

use crate::config::settings::IndexConfig;
use crate::db::connection::DbPool;
use crate::db::models::{NewAsset, ScanPhase, ScanProgress, ScanStats};
use crate::db::queries;
use crate::error::SoundScoutError;
use crate::indexer::{metadata, path_parser, peaks, scanner};

/// Shareable cancel flag for a running scan.
pub struct CancelHandle(Arc<AtomicBool>);

impl CancelHandle {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Orchestrates scan → metadata → peaks → SQLite.
pub struct IndexPipeline {
    pool: Arc<DbPool>,
    config: IndexConfig,
    cancelled: Arc<AtomicBool>,
}

impl IndexPipeline {
    pub fn new(pool: Arc<DbPool>, config: IndexConfig) -> Self {
        Self {
            pool,
            config,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel_handle(&self) -> CancelHandle {
        CancelHandle(Arc::clone(&self.cancelled))
    }

    /// Run indexing for `root`. Progress events are best-effort (`send` errors ignored).
    pub fn run(
        &self,
        root: &Path,
        progress: Sender<ScanProgress>,
    ) -> Result<ScanStats, SoundScoutError> {
        let started = Instant::now();
        let root_key = root.to_string_lossy().to_string();
        let files = scanner::scan_directory_with_progress(root, self.config.skip_hidden, &|_| {})?;
        let total = files.len() as u64;
        let _ = progress.send(ScanProgress {
            scanned: 0,
            total,
            current_file: String::new(),
            phase: ScanPhase::Enumerating,
        });

        let conn = self.pool.get()?;
        let indexed = queries::get_indexed_paths_with_mtime(&conn)?;

        let on_disk: HashSet<String> = files
            .iter()
            .map(|f| f.path.to_string_lossy().into_owned())
            .collect();

        let mut missing_ids = Vec::new();
        for (path, _) in &indexed {
            if path.starts_with(root_key.as_str()) && !on_disk.contains(path) {
                if let Some(a) = queries::get_asset_by_path(&conn, path)? {
                    missing_ids.push(a.id);
                }
            }
        }

        let mut work: Vec<(scanner::ScannedFile, bool)> = Vec::new();
        let mut skipped = 0u64;
        for f in &files {
            let p = f.path.to_string_lossy().into_owned();
            match indexed.get(&p) {
                None => work.push((f.clone(), true)),
                Some(&mtime) if mtime != f.modified_at => work.push((f.clone(), false)),
                Some(_) => skipped += 1,
            }
        }

        let processed = Arc::new(AtomicU64::new(0));
        let errors = Arc::new(AtomicU64::new(0));
        let peak_res = self.config.peak_resolution.max(1);
        let cancelled = Arc::clone(&self.cancelled);

        let results: Vec<Result<(NewAsset, bool), SoundScoutError>> = work
            .par_iter()
            .map(|(f, is_new)| {
                if cancelled.load(Ordering::Relaxed) {
                    return Err(SoundScoutError::Cancelled);
                }
                let path = &f.path;
                let r = (|| {
                    let meta = metadata::extract_metadata(path)?;
                    let pm = path_parser::parse_path(root, path);
                    let peak_blob = peaks::compute_peaks(path, peak_res).unwrap_or_else(|_| Vec::new());
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| i64::try_from(d.as_secs()).unwrap_or(0))
                        .unwrap_or(0);
                    Ok(NewAsset {
                        path: path.to_string_lossy().into_owned(),
                        filename: pm.filename.clone(),
                        extension: pm.extension.clone(),
                        folder: pm.folder.clone(),
                        duration_ms: meta.duration_ms,
                        sample_rate: meta.sample_rate,
                        channels: meta.channels,
                        bit_depth: meta.bit_depth,
                        file_size: i64::try_from(f.file_size).unwrap_or(i64::MAX),
                        category: pm.category.clone(),
                        publisher: pm.publisher.clone(),
                        modified_at: f.modified_at,
                        indexed_at: now,
                        peaks: Some(peak_blob),
                    })
                })();
                match r {
                    Ok(new) => {
                        let n = processed.fetch_add(1, Ordering::Relaxed) + 1;
                        let _ = progress.send(ScanProgress {
                            scanned: n,
                            total,
                            current_file: new.path.clone(),
                            phase: ScanPhase::Indexing,
                        });
                        Ok((new, *is_new))
                    }
                    Err(e) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                        Err(e)
                    }
                }
            })
            .collect();

        if cancelled.load(Ordering::Relaxed) {
            return Err(SoundScoutError::Cancelled);
        }

        let mut indexed_count = 0u64;
        let mut to_insert: Vec<NewAsset> = Vec::new();
        let mut to_update: Vec<(String, NewAsset)> = Vec::new();

        for r in results {
            match r {
                Ok((a, true)) => to_insert.push(a),
                Ok((a, false)) => to_update.push((a.path.clone(), a)),
                Err(SoundScoutError::Cancelled) => return Err(SoundScoutError::Cancelled),
                Err(_) => {}
            }
        }

        for chunk in to_insert.chunks(500) {
            indexed_count += queries::insert_asset_batch(&conn, chunk)? as u64;
        }
        for (p, a) in to_update {
            queries::update_asset_metadata(&conn, &p, &a)?;
            indexed_count += 1;
        }

        let err_count = errors.load(Ordering::Relaxed);
        let deleted = queries::delete_assets_by_ids(&conn, &missing_ids)? as u64;

        let _ = progress.send(ScanProgress {
            scanned: total,
            total,
            current_file: String::new(),
            phase: ScanPhase::Complete,
        });

        Ok(ScanStats {
            files_indexed: indexed_count,
            files_skipped: skipped,
            files_missing: deleted,
            errors: err_count,
            duration_secs: started.elapsed().as_secs_f64(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use tempfile::TempDir;

    use crate::db::connection::DbPool;
    use crate::test_utils::write_test_wav;

    fn pool() -> Arc<DbPool> {
        Arc::new(DbPool::new_in_memory().expect("pool"))
    }

    fn cfg() -> IndexConfig {
        IndexConfig::default()
    }

    #[test]
    fn full_scan_indexes_all_files() {
        let dir = TempDir::new().expect("d");
        for i in 0..3 {
            let p = dir.path().join(format!("{i}.wav"));
            write_test_wav(&p, 8000, 1, 16, 100, 440.0).expect("w");
        }
        let pool = pool();
        let pipe = IndexPipeline::new(pool, cfg());
        let (tx, _rx) = channel();
        let stats = pipe.run(dir.path(), tx).expect("run");
        assert_eq!(stats.files_indexed, 3);
    }

    #[test]
    fn incremental_scan_skips_unchanged() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("a.wav");
        write_test_wav(&p, 8000, 1, 16, 200, 100.0).expect("w");
        let pool = pool();
        let pipe = IndexPipeline::new(pool.clone(), cfg());
        pipe.run(dir.path(), channel().0).expect("run1");
        let pipe2 = IndexPipeline::new(pool, cfg());
        let s2 = pipe2.run(dir.path(), channel().0).expect("run2");
        assert_eq!(s2.files_indexed, 0);
        assert!(s2.files_skipped >= 1);
    }

    #[test]
    fn scan_detects_new_files_on_rescan() {
        let dir = TempDir::new().expect("d");
        let p1 = dir.path().join("a.wav");
        write_test_wav(&p1, 8000, 1, 16, 50, 0.0).expect("w");
        let pool = pool();
        IndexPipeline::new(pool.clone(), cfg())
            .run(dir.path(), channel().0)
            .expect("r1");
        let p2 = dir.path().join("b.wav");
        write_test_wav(&p2, 8000, 1, 16, 50, 0.0).expect("w");
        let s = IndexPipeline::new(pool, cfg())
            .run(dir.path(), channel().0)
            .expect("r2");
        assert!(s.files_indexed >= 1);
    }

    #[test]
    fn missing_files_are_detected() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("gone.wav");
        write_test_wav(&p, 8000, 1, 16, 50, 0.0).expect("w");
        let pool = pool();
        IndexPipeline::new(pool.clone(), cfg())
            .run(dir.path(), channel().0)
            .expect("r1");
        std::fs::remove_file(&p).expect("rm");
        let s = IndexPipeline::new(pool, cfg())
            .run(dir.path(), channel().0)
            .expect("r2");
        assert!(s.files_missing >= 1);
    }

    #[test]
    fn progress_events_are_emitted() {
        let dir = TempDir::new().expect("d");
        write_test_wav(&dir.path().join("a.wav"), 8000, 1, 16, 30, 0.0).expect("w");
        let pool = pool();
        let (tx, rx) = channel();
        IndexPipeline::new(pool, cfg())
            .run(dir.path(), tx)
            .expect("run");
        let mut n = 0u32;
        while rx.try_recv().is_ok() {
            n += 1;
        }
        assert!(n > 0);
    }
}
