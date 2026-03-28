//! Autocorrelation, AMDF, classification, zero-crossings.

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(dead_code)] // `amdf` reserved for extended loop scoring

use realfft::RealFftPlanner;

/// FFT autocorrelation r[τ] normalised by r[0], length = input.len() (real part truncated).
pub fn fft_autocorrelation(samples: &[f32]) -> Result<Vec<f32>, String> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    let n = (samples.len() * 2).next_power_of_two();
    let mut planner = RealFftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(n);
    let inv = planner.plan_fft_inverse(n);
    let mut input = samples.to_vec();
    input.resize(n, 0.0);
    let mut spectrum = fwd.make_output_vec();
    let mut scratch_fwd = fwd.make_scratch_vec();
    fwd.process_with_scratch(&mut input, &mut spectrum, &mut scratch_fwd)
        .map_err(|e| e.to_string())?;
    for bin in &mut spectrum {
        let re = bin.re;
        let im = bin.im;
        bin.re = re * re + im * im;
        bin.im = 0.0;
    }
    let mut acf = inv.make_output_vec();
    let mut scratch_inv = inv.make_scratch_vec();
    inv.process_with_scratch(&mut spectrum, &mut acf, &mut scratch_inv)
        .map_err(|e| e.to_string())?;
    let take = samples.len().min(acf.len());
    let mut out: Vec<f32> = acf.into_iter().take(take).collect();
    let norm = out.first().copied().unwrap_or(1.0).abs().max(f32::EPSILON);
    for v in &mut out {
        *v /= norm;
    }
    Ok(out)
}

pub fn amdf(samples: &[f32], max_lag: usize) -> Vec<f32> {
    let n = samples.len();
    let max_lag = max_lag.min(n.saturating_sub(1));
    let mut out = Vec::with_capacity(max_lag + 1);
    out.push(0.0);
    for lag in 1..=max_lag {
        let mut sum = 0.0f32;
        let count = n - lag;
        if count == 0 {
            out.push(0.0);
            continue;
        }
        for i in 0..count {
            sum += (samples[i] - samples[i + lag]).abs();
        }
        out.push(sum / count as f32);
    }
    out
}

/// Classify using autocorrelation on `mono` (one sample per frame).
/// Returns `(peak_correlation, estimated_hz, kind)` where `kind`: 0 = aperiodic, 1 = quasi, 2 = periodic.
pub fn classify_mono(mono: &[f32], sample_rate: u32) -> (f32, f32, u8) {
    if mono.len() < 256 {
        return (0.0, 0.0, 0);
    }
    let start = mono.len() / 4;
    let end = mono.len() * 3 / 4;
    let region = &mono[start..end];
    let acf = fft_autocorrelation(region).unwrap_or_default();
    let min_lag = (sample_rate as usize / 2000).max(2);
    let max_lag = (acf.len().min(region.len()) - 1).max(min_lag + 1);
    if max_lag <= min_lag {
        return (0.0, 0.0, 0);
    }
    let mut best_lag = min_lag;
    let mut best_val = f32::NEG_INFINITY;
    for lag in min_lag..max_lag {
        if let Some(&v) = acf.get(lag) {
            if v > best_val {
                best_val = v;
                best_lag = lag;
            }
        }
    }
    let hz = sample_rate as f32 / best_lag as f32;
    let kind = if best_val > 0.7 {
        2u8
    } else if best_val > 0.4 {
        1u8
    } else {
        0u8
    };
    (best_val, hz, kind)
}

/// Positive-going zero crossings in `[start, end)`.
pub fn positive_zero_crossings(samples: &[f32], start: usize, end: usize) -> Vec<usize> {
    let end = end.min(samples.len());
    let start = start.min(end);
    let mut out = Vec::new();
    for i in start + 1..end {
        let prev = samples[i - 1];
        let cur = samples[i];
        if prev < 0.0 && cur >= 0.0 {
            out.push(i);
        }
    }
    out
}
