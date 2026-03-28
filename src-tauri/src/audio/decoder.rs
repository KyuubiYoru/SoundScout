//! High-level decode to [`PcmData`](crate::db::models::PcmData).

use std::path::Path;

use crate::audio::decode_core;
use crate::db::models::PcmData;
use crate::error::SoundScoutError;

/// Decode entire file to PCM suitable for playback.
pub fn decode_to_pcm(path: &Path) -> Result<PcmData, SoundScoutError> {
    let (samples, sample_rate, channels) = decode_core::decode_samples(path)?;
    Ok(PcmData {
        samples,
        sample_rate,
        channels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::write_test_wav;
    use tempfile::TempDir;

    #[test]
    fn decode_returns_pcm_data() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("a.wav");
        write_test_wav(&p, 44_100, 1, 16, 800, 300.0).expect("w");
        let pcm = decode_to_pcm(&p).expect("pcm");
        assert_eq!(pcm.sample_rate, 44_100);
        assert_eq!(pcm.channels, 1);
        assert!(!pcm.samples.is_empty());
    }

    #[test]
    fn pcm_data_has_correct_duration() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("a.wav");
        let sr = 8000u32;
        let frames = 8000u32;
        write_test_wav(&p, sr, 1, 16, frames, 200.0).expect("w");
        let pcm = decode_to_pcm(&p).expect("pcm");
        let secs = pcm.samples.len() as f64 / f64::from(pcm.sample_rate) / f64::from(pcm.channels);
        assert!((secs - 1.0).abs() < 0.05);
    }
}
