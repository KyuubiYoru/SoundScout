//! Crossfade curves: linear, equal-power, correlation-adaptive.

#![allow(clippy::cast_precision_loss)]
#![allow(dead_code)] // `linear` reserved for alternate loop shaping

/// Normalised cross-correlation between two equal-length slices.
pub fn cross_correlation(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut sum_ab = 0.0f32;
    let mut sum_a2 = 0.0f32;
    let mut sum_b2 = 0.0f32;
    for i in 0..a.len() {
        sum_ab += a[i] * b[i];
        sum_a2 += a[i] * a[i];
        sum_b2 += b[i] * b[i];
    }
    let denom = (sum_a2.sqrt() * sum_b2.sqrt()).max(f32::EPSILON);
    (sum_ab / denom).clamp(-1.0, 1.0)
}

pub fn linear(n: usize) -> (Vec<f32>, Vec<f32>) {
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    if n == 1 {
        return (vec![1.0], vec![0.0]);
    }
    let fade_in: Vec<f32> = (0..n)
        .map(|i| i as f32 / (n - 1) as f32)
        .collect();
    let fade_out: Vec<f32> = fade_in.iter().rev().cloned().collect();
    (fade_in, fade_out)
}

/// Fade the last `n` frames of the tail smoothly toward `frame[0]` (the loop-start), then force
/// an exact sample match.  **Only the tail is touched**; the head (loop_start) is never modified.
///
/// Playback order is … `frames−1` → `0`, so this removes the wrap click without disturbing the
/// crossfaded head that `blend_loop_seam` already set up.  Works per-channel for stereo.
pub fn weld_loop_wrap_endpoints(interleaved: &mut [f32], ch: usize) {
    let frames = interleaved.len() / ch;
    if frames < 2 || ch == 0 {
        return;
    }
    // Short window so we don't audibly alter the tail body.
    let n = (32usize).min(frames.saturating_sub(1) / 2).max(1);
    for c in 0..ch {
        let first = interleaved[c]; // frame 0 — loop-start, never written
        for i in 0..n {
            // weight: 0 at tail window start → 1 at the very last frame
            let weight = (i + 1) as f32 / n as f32;
            let idx = (frames - n + i) * ch + c;
            interleaved[idx] = interleaved[idx] * (1.0 - weight) + first * weight;
        }
        // Guarantee exact continuity at the sample level.
        interleaved[(frames - 1) * ch + c] = first;
    }
}

pub fn equal_power(n: usize) -> (Vec<f32>, Vec<f32>) {
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    if n == 1 {
        return (vec![1.0], vec![0.0]);
    }
    let fade_in: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            (std::f32::consts::FRAC_PI_2 * t).sin()
        })
        .collect();
    let fade_out: Vec<f32> = fade_in.iter().rev().cloned().collect();
    (fade_in, fade_out)
}

/// Niemitalo correlation-adaptive gain.
pub fn correlation_adaptive_gain(t: f32, r: f32) -> f32 {
    let denom = (2.0 * t * (r + t - 1.0 - r * t) + 1.0).max(f32::EPSILON);
    t / denom.sqrt()
}

pub fn correlation_adaptive(n: usize, r: f32) -> (Vec<f32>, Vec<f32>) {
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    if n == 1 {
        return (vec![1.0], vec![0.0]);
    }
    let denom = (n - 1).max(1) as f32;
    let fade_in: Vec<f32> = (0..n)
        .map(|i| correlation_adaptive_gain(i as f32 / denom, r))
        .collect();
    let fade_out: Vec<f32> = fade_in.iter().rev().cloned().collect();
    (fade_in, fade_out)
}

/// Blend loop seam: last `xf_frames` overlap first `xf_frames` in `interleaved` (full loop body).
///
/// The tail is blended into the head using a correlation-adaptive curve.
pub fn blend_loop_seam(interleaved: &mut [f32], ch: usize, xf_frames: usize, r: f32) -> Result<(), String> {
    let frames = interleaved.len() / ch;
    if xf_frames == 0 || frames < xf_frames * 2 + 1 {
        return Err("crossfade too long for buffer".to_string());
    }
    let (fade_in, fade_out) = correlation_adaptive(xf_frames, r);
    for fi in 0..xf_frames {
        let g_in = fade_in[fi];
        let g_out = fade_out[fi];
        for c in 0..ch {
            let tail_i = (frames - xf_frames + fi) * ch + c;
            let head_i = fi * ch + c;
            let t = interleaved[tail_i];
            let h = interleaved[head_i];
            interleaved[head_i] = t * g_out + h * g_in;
        }
    }
    Ok(())
}
