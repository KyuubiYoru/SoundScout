//! Waveform peak extraction from decoded PCM.

use std::path::Path;

use crate::audio::decode_core;
use crate::error::SoundScoutError;

/// Compute min/max peak pairs (`resolution` buckets) as little-endian `f32` bytes.
pub fn compute_peaks(path: &Path, resolution: usize) -> Result<Vec<u8>, SoundScoutError> {
    if resolution == 0 {
        return Err(SoundScoutError::Validation(
            "peak resolution must be > 0".into(),
        ));
    }
    let (mut samples, _sr, ch) = decode_core::decode_samples(path)?;
    if ch >= 2 {
        let frames = samples.len() / usize::from(ch);
        let mut mono = Vec::with_capacity(frames);
        for i in 0..frames {
            let mut sum = 0.0f32;
            for c in 0..usize::from(ch) {
                sum += samples[i * usize::from(ch) + c];
            }
            mono.push(sum / f32::from(ch));
        }
        samples = mono;
    }

    let n = samples.len();
    let chunk = (n / resolution).max(1);
    let mut out = Vec::with_capacity(resolution * 8);
    for i in 0..resolution {
        let start = i * chunk;
        let end = if i + 1 == resolution {
            n
        } else {
            ((i + 1) * chunk).min(n)
        };
        let (mn, mx) = if start >= n {
            (0.0f32, 0.0f32)
        } else {
            let slice = &samples[start..end];
            let mut mn = slice[0];
            let mut mx = slice[0];
            for &s in slice.iter().skip(1) {
                mn = mn.min(s);
                mx = mx.max(s);
            }
            (mn.clamp(-1.0, 1.0), mx.clamp(-1.0, 1.0))
        };
        out.extend_from_slice(&mn.to_le_bytes());
        out.extend_from_slice(&mx.to_le_bytes());
    }
    Ok(out)
}

/// Decode BLOB bytes to interleaved min,max float sequence.
pub fn peaks_to_floats(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::write_test_wav;
    use tempfile::TempDir;

    #[test]
    fn peak_byte_count_matches_resolution() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("p.wav");
        write_test_wav(&p, 8000, 1, 16, 4000, 440.0).expect("w");
        let b = compute_peaks(&p, 100).expect("pk");
        assert_eq!(b.len(), 100 * 2 * 4);
    }

    #[test]
    fn peaks_to_floats_roundtrip() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("p.wav");
        write_test_wav(&p, 8000, 1, 16, 2000, 200.0).expect("w");
        let b = compute_peaks(&p, 20).expect("pk");
        let f = peaks_to_floats(&b);
        assert_eq!(f.len(), 40);
    }

    #[test]
    fn peaks_are_in_valid_range() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("p.wav");
        write_test_wav(&p, 8000, 1, 16, 1000, 100.0).expect("w");
        let f = peaks_to_floats(&compute_peaks(&p, 10).expect("pk"));
        assert!(f.iter().all(|&x| (-1.0..=1.0).contains(&x)));
    }

    #[test]
    fn silence_produces_near_zero_peaks() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("p.wav");
        write_test_wav(&p, 8000, 1, 16, 5000, 0.0).expect("w");
        let f = peaks_to_floats(&compute_peaks(&p, 50).expect("pk"));
        assert!(f.iter().all(|&x| x.abs() < 0.01));
    }

    #[test]
    fn sine_wave_has_significant_peaks() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("p.wav");
        write_test_wav(&p, 44_100, 1, 16, 44_100, 440.0).expect("w");
        let f = peaks_to_floats(&compute_peaks(&p, 40).expect("pk"));
        let mx = f.iter().copied().fold(0.0f32, f32::max);
        assert!(mx > 0.5);
    }

    #[test]
    fn resolution_of_one_returns_8_bytes() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("p.wav");
        write_test_wav(&p, 8000, 1, 16, 100, 440.0).expect("w");
        assert_eq!(compute_peaks(&p, 1).expect("pk").len(), 8);
    }

    #[test]
    fn corrupt_file_returns_error() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("x.wav");
        std::fs::write(&p, b"xxx").unwrap();
        assert!(compute_peaks(&p, 10).is_err());
    }
}
