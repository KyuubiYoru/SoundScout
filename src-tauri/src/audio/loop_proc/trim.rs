//! Leading/trailing silence trim using a simple envelope on mono.

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

/// Convert dB threshold to linear amplitude (e.g. -60 dB → ~0.001).
pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Envelope follower (peak hold with exponential release).
pub fn envelope_follower(mono: &[f32], attack_ms: f32, release_ms: f32, sr: u32) -> Vec<f32> {
    if mono.is_empty() {
        return Vec::new();
    }
    let sr_f = sr as f32;
    let attack = (-1.0 / (attack_ms.max(0.1) * 0.001 * sr_f)).exp();
    let release = (-1.0 / (release_ms.max(1.0) * 0.001 * sr_f)).exp();
    let mut out = Vec::with_capacity(mono.len());
    let mut env = 0.0f32;
    for &x in mono {
        let a = x.abs();
        let coeff = if a > env { attack } else { release };
        env = a + (env - a) * coeff;
        out.push(env);
    }
    out
}

/// Returns `(first_frame, last_exclusive_frame)` into the mono buffer's frame indices
/// (same length as mono — one sample per frame for mono-derived signal).
pub fn detect_silence_bounds(
    mono: &[f32],
    sample_rate: u32,
    threshold_db: f32,
    min_silence_ms: f32,
) -> (usize, usize) {
    if mono.is_empty() {
        return (0, 0);
    }
    let thresh = db_to_linear(threshold_db);
    let min_run = ((min_silence_ms * 0.001) * sample_rate as f32).ceil().max(1.0) as usize;
    let env = envelope_follower(mono, 1.0, 50.0, sample_rate);

    let mut first = 0usize;
    while first < mono.len() && env[first] < thresh {
        first += 1;
    }
    if first >= mono.len() {
        // Entirely silent — return empty range so the caller can decide what to do.
        return (0, 0);
    }
    // Only trim the leading edge when the silence region is long enough to be intentional.
    if first < min_run {
        first = 0;
    }

    let mut last = mono.len();
    while last > first {
        let i = last - 1;
        if env[i] >= thresh {
            break;
        }
        last -= 1;
    }
    // Only trim the trailing edge when the silence region is long enough.
    let tail_silence = mono.len() - last;
    if tail_silence < min_run {
        last = mono.len();
    }

    (first, last)
}

/// Trim interleaved buffer to `[first_frame * ch, last_exclusive_frame * ch)`.
pub fn trim_interleaved(interleaved: &[f32], ch: usize, first_frame: usize, last_exclusive_frame: usize) -> Vec<f32> {
    let start = first_frame * ch;
    let end = last_exclusive_frame * ch;
    if start >= end || end > interleaved.len() {
        return interleaved.to_vec();
    }
    interleaved[start..end].to_vec()
}
