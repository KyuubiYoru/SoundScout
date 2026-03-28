//! Leading/trailing silence trim using short-window peak levels on mono (symmetric at both edges).

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

/// Clamp analysis window to a sensible range (ms).
fn clamp_peak_window_ms(ms: f32) -> f32 {
    ms.clamp(1.0, 32.0)
}

/// Convert dB threshold to linear amplitude (e.g. -60 dB → ~0.001).
pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Per-frame maximum of `|mono[j]|` over `j ∈ [i - half, i + half]` (inclusive).
fn window_peak_max(abs_mono: &[f32], half_window: usize) -> Vec<f32> {
    let n = abs_mono.len();
    if n == 0 {
        return Vec::new();
    }
    let hw = half_window.max(1);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let lo = i.saturating_sub(hw);
        let hi = (i + hw).min(n - 1);
        let mut m = 0.0f32;
        for j in lo..=hi {
            m = m.max(abs_mono[j]);
        }
        out.push(m);
    }
    out
}

/// Returns `(first_frame, last_exclusive_frame)` into the mono buffer's frame indices
/// (same length as mono — one sample per frame for mono-derived signal).
///
/// Uses a short sliding max of `|x|` so trailing silence is detected immediately after the last peak
/// (unlike a slow-release envelope). Leading/trailing trims are applied only when the silent run at
/// that edge is at least `min_silence_ms` long (converted to frames).
///
/// `peak_window_ms` is the total width of the sliding max window (typically 2–8 ms).
pub fn detect_silence_bounds(
    mono: &[f32],
    sample_rate: u32,
    threshold_db: f32,
    min_silence_ms: f32,
    peak_window_ms: f32,
) -> (usize, usize) {
    if mono.is_empty() {
        return (0, 0);
    }
    let sr = sample_rate.max(1) as f32;
    let win = clamp_peak_window_ms(peak_window_ms);
    let half_window = ((win * 0.5 * 0.001) * sr).ceil().max(1.0) as usize;
    let thresh = db_to_linear(threshold_db);
    let min_run = ((min_silence_ms.max(0.0) * 0.001) * sr).ceil().max(0.0) as usize;

    let abs_mono: Vec<f32> = mono.iter().map(|x| x.abs()).collect();
    let peak = window_peak_max(&abs_mono, half_window);

    let mut first_non_silent: Option<usize> = None;
    for i in 0..mono.len() {
        if peak[i] >= thresh {
            first_non_silent = Some(i);
            break;
        }
    }
    let Some(fs) = first_non_silent else {
        return (0, 0);
    };

    let leading_silent_frames = fs;
    let first = if leading_silent_frames >= min_run {
        fs
    } else {
        0
    };

    let mut last_audible: Option<usize> = None;
    for i in (0..mono.len()).rev() {
        if peak[i] >= thresh {
            last_audible = Some(i);
            break;
        }
    }
    let Some(la) = last_audible else {
        return (0, 0);
    };

    let last_exclusive_default = la + 1;
    let trailing_silent_frames = mono.len().saturating_sub(last_exclusive_default);
    let last = if trailing_silent_frames >= min_run {
        last_exclusive_default
    } else {
        mono.len()
    };

    (first, last)
}

/// Trim interleaved buffer to `[first_frame * ch, last_exclusive_frame * ch)`.
pub fn trim_interleaved(
    interleaved: &[f32],
    ch: usize,
    first_frame: usize,
    last_exclusive_frame: usize,
) -> Vec<f32> {
    let start = first_frame * ch;
    let end = last_exclusive_frame * ch;
    if start >= end || end > interleaved.len() {
        return interleaved.to_vec();
    }
    interleaved[start..end].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_leading_silence_only() {
        let sr = 48_000u32;
        let silence_frames = sr as usize / 10; // 100 ms
        let tone_frames = sr as usize;
        let mut mono = vec![0.0f32; silence_frames];
        mono.extend(vec![0.99f32; tone_frames]);
        let thresh = -60.0f32;
        let min_ms = 10.0f32;
        let (f0, f1) = detect_silence_bounds(&mono, sr, thresh, min_ms, 4.0);
        assert!(f0 > 0, "expected leading trim");
        assert_eq!(f1, mono.len());
        let trimmed = trim_interleaved(&mono, 1, f0, f1);
        assert!(trimmed.len() < mono.len());
        assert!(f0 <= silence_frames + 500, "trim start should be near tone onset");
    }

    #[test]
    fn trims_trailing_silence_only() {
        let sr = 48_000u32;
        let tone_frames = sr as usize / 2;
        let silence_frames = sr as usize / 10;
        let mut mono = vec![0.99f32; tone_frames];
        mono.extend(vec![0.0f32; silence_frames]);
        let thresh = -60.0f32;
        let min_ms = 10.0f32;
        let (f0, f1) = detect_silence_bounds(&mono, sr, thresh, min_ms, 4.0);
        assert_eq!(f0, 0);
        assert!(f1 < mono.len(), "expected trailing trim");
        let trimmed = trim_interleaved(&mono, 1, f0, f1);
        assert!(trimmed.len() < mono.len());
    }

    #[test]
    fn trims_both_ends() {
        let sr = 48_000u32;
        let pad = sr as usize / 10;
        let tone_frames = sr as usize / 4;
        let mut mono = vec![0.0f32; pad];
        mono.extend(vec![0.99f32; tone_frames]);
        mono.extend(vec![0.0f32; pad]);
        let thresh = -60.0f32;
        let min_ms = 10.0f32;
        let (f0, f1) = detect_silence_bounds(&mono, sr, thresh, min_ms, 4.0);
        assert!(f0 > 0 && f1 < mono.len());
        let trimmed = trim_interleaved(&mono, 1, f0, f1);
        assert!(trimmed.len() < mono.len());
    }

    #[test]
    fn short_leading_silence_not_trimmed_when_below_min_run() {
        let sr = 48_000u32;
        let silence_frames = sr as usize / 100; // 10 ms
        let mut mono = vec![0.0f32; silence_frames];
        mono.extend(vec![0.99f32; 10_000]);
        let thresh = -60.0f32;
        let min_ms = 20.0f32; // require 20 ms — 10 ms lead should not trim
        let (f0, f1) = detect_silence_bounds(&mono, sr, thresh, min_ms, 4.0);
        assert_eq!(f0, 0);
        assert_eq!(f1, mono.len());
    }

    #[test]
    fn stereo_interleaved_respects_frame_bounds() {
        let sr = 8_000u32;
        let lead = 400usize;
        let tone = 4_000usize;
        let mut mono = vec![0.0f32; lead];
        mono.extend(vec![0.99f32; tone]);
        let ch = 2usize;
        let mut interleaved = Vec::with_capacity(mono.len() * ch);
        for &m in &mono {
            interleaved.push(m);
            interleaved.push(m);
        }
        let (f0, f1) = detect_silence_bounds(&mono, sr, -60.0, 5.0, 4.0);
        let out = trim_interleaved(&interleaved, ch, f0, f1);
        assert_eq!(out.len(), (f1 - f0) * ch);
        assert_eq!(out.len() % ch, 0);
    }

    #[test]
    fn entirely_silent_returns_empty_range() {
        let mono = vec![0.0f32; 10_000];
        let (f0, f1) = detect_silence_bounds(&mono, 48_000, -60.0, 1.0, 4.0);
        assert_eq!((f0, f1), (0, 0));
    }
}
