//! Audio metadata via Symphonia (partial decode / probe).

use std::fs::File;
use std::path::Path;

use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use crate::db::models::AudioMetadata;
use crate::error::SoundScoutError;

/// Read duration / codec parameters without full decode when possible.
pub fn extract_metadata(path: &Path) -> Result<AudioMetadata, SoundScoutError> {
    let file = File::open(path).map_err(SoundScoutError::Io)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| SoundScoutError::AudioDecode(e.to_string()))?;

    let format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| SoundScoutError::AudioDecode("no default track".into()))?;

    let sample_rate = track.codec_params.sample_rate.map(|v| v as i32);
    let channels = track
        .codec_params
        .channels
        .map(|c| i32::try_from(c.count()).unwrap_or(1));
    let bit_depth = track.codec_params.bits_per_sample.map(|v| v as i32);

    let n_frames = track.codec_params.n_frames;
    let duration_ms = match (n_frames, sample_rate) {
        (Some(frames), Some(sr)) if sr > 0 => {
            let sr_u = u64::try_from(sr).unwrap_or(1);
            Some(i64::try_from(frames * 1000 / sr_u).unwrap_or(0))
        }
        _ => None,
    };

    Ok(AudioMetadata {
        duration_ms,
        sample_rate,
        channels,
        bit_depth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::write_test_wav;
    use tempfile::TempDir;

    #[test]
    fn reads_wav_44100_mono_16bit() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("t.wav");
        write_test_wav(&p, 44_100, 1, 16, 44_100, 440.0).expect("w");
        let m = extract_metadata(&p).expect("meta");
        assert_eq!(m.sample_rate, Some(44_100));
        assert_eq!(m.channels, Some(1));
        assert_eq!(m.bit_depth, Some(16));
        assert!(m.duration_ms.is_some());
    }

    #[test]
    fn reads_wav_48000_stereo() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("t.wav");
        write_test_wav(&p, 48_000, 2, 16, 1000, 100.0).expect("w");
        let m = extract_metadata(&p).expect("meta");
        assert_eq!(m.channels, Some(2));
    }

    #[test]
    fn reads_wav_24bit() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("t.wav");
        write_test_wav(&p, 44_100, 1, 24, 5000, 200.0).expect("w");
        let m = extract_metadata(&p).expect("meta");
        assert_eq!(m.bit_depth, Some(24));
    }

    #[test]
    fn duration_is_approximately_correct() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("t.wav");
        write_test_wav(&p, 1000, 1, 16, 1000, 440.0).expect("w");
        let m = extract_metadata(&p).expect("meta");
        let d = m.duration_ms.expect("dur");
        assert!((d - 1000).abs() <= 50);
    }

    #[test]
    fn corrupt_file_returns_error_not_panic() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("x.wav");
        std::fs::write(&p, [0u8; 64]).unwrap();
        assert!(extract_metadata(&p).is_err());
    }

    #[test]
    fn empty_file_returns_error() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("e.wav");
        std::fs::write(&p, []).unwrap();
        assert!(extract_metadata(&p).is_err());
    }

    #[test]
    fn truncated_header_no_panic() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("t.wav");
        std::fs::write(&p, b"RIFF\x00\x00\x00\x00WAVE").unwrap();
        let _ = extract_metadata(&p);
    }
}
