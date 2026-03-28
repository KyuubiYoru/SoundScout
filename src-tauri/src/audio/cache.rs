//! LRU cache of decoded [`PcmData`](crate::db::models::PcmData).

use std::path::Path;
use std::sync::{Arc, Mutex};

use lru::LruCache;

use crate::audio::decoder;
use crate::db::models::PcmData;
use crate::error::SoundScoutError;

/// Thread-safe LRU over asset id → PCM.
pub struct AudioCache {
    inner: Mutex<LruCache<i64, Arc<PcmData>>>,
}

impl AudioCache {
    pub fn new(capacity: usize) -> Self {
        let cap = std::num::NonZeroUsize::new(capacity.max(1)).expect("nonzero");
        Self {
            inner: Mutex::new(LruCache::new(cap)),
        }
    }

    pub fn get(&self, asset_id: i64) -> Option<Arc<PcmData>> {
        self.inner.lock().expect("cache lock").get(&asset_id).cloned()
    }

    pub fn insert(&self, asset_id: i64, data: PcmData) -> Arc<PcmData> {
        let arc = Arc::new(data);
        let mut g = self.inner.lock().expect("cache lock");
        g.put(asset_id, Arc::clone(&arc));
        arc
    }

    pub fn get_or_decode(
        &self,
        asset_id: i64,
        path: &Path,
    ) -> Result<Arc<PcmData>, SoundScoutError> {
        if let Some(p) = self.get(asset_id) {
            return Ok(p);
        }
        let pcm = decoder::decode_to_pcm(path)?;
        Ok(self.insert(asset_id, pcm))
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("cache lock").len()
    }

    pub fn clear(&self) {
        self.inner.lock().expect("cache lock").clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::write_test_wav;
    use tempfile::TempDir;

    #[test]
    fn cache_hit_returns_same_arc() {
        let c = AudioCache::new(4);
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("x.wav");
        write_test_wav(&p, 8000, 1, 16, 100, 0.0).expect("w");
        let a = c.get_or_decode(1, &p).expect("d");
        let b = c.get_or_decode(1, &p).expect("d");
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn cache_miss_triggers_decode() {
        let c = AudioCache::new(4);
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("x.wav");
        write_test_wav(&p, 8000, 1, 16, 50, 100.0).expect("w");
        assert_eq!(c.len(), 0);
        c.get_or_decode(7, &p).expect("d");
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn cache_evicts_oldest_when_full() {
        let c = AudioCache::new(2);
        let dir = TempDir::new().expect("d");
        for i in 0..3 {
            let p = dir.path().join(format!("{i}.wav"));
            write_test_wav(&p, 8000, 1, 16, 10, 0.0).expect("w");
            c.get_or_decode(i, &p).expect("d");
        }
        assert!(c.get(0).is_none());
        assert!(c.get(2).is_some());
    }

    #[test]
    fn cache_len_tracks_entries() {
        let c = AudioCache::new(10);
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn cache_clear_empties() {
        let c = AudioCache::new(4);
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("x.wav");
        write_test_wav(&p, 8000, 1, 16, 10, 0.0).expect("w");
        c.get_or_decode(1, &p).expect("d");
        c.clear();
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn get_moves_entry_to_most_recent() {
        let c = AudioCache::new(2);
        let dir = TempDir::new().expect("d");
        for (i, name) in [(0, "a.wav"), (1, "b.wav"), (2, "c.wav")] {
            let p = dir.path().join(name);
            write_test_wav(&p, 8000, 1, 16, 5, 0.0).expect("w");
            if i < 2 {
                c.get_or_decode(i64::from(i), &p).expect("d");
            }
        }
        c.get(0).expect("touch 0");
        let p2 = dir.path().join("c.wav");
        write_test_wav(&p2, 8000, 1, 16, 5, 0.0).expect("w");
        c.get_or_decode(2, &p2).expect("d");
        assert!(c.get(1).is_none());
        assert!(c.get(0).is_some());
    }
}
