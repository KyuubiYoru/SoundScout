//! Peak normalization on interleaved f32.

#![allow(clippy::cast_precision_loss)]
#![allow(dead_code)] // `rms` reserved for future loudness matching

/// Scale interleaved samples so peak absolute value hits `target` (e.g. 0.97).
/// If peak is 0, does nothing.
pub fn peak_normalize(interleaved: &mut [f32], target: f32) {
    if target <= 0.0 || !target.is_finite() {
        return;
    }
    let mut peak = 0.0f32;
    for &s in interleaved.iter() {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
    }
    if peak <= f32::EPSILON {
        return;
    }
    let g = target / peak;
    for s in interleaved.iter_mut() {
        *s *= g;
    }
}

/// RMS of a slice.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}
