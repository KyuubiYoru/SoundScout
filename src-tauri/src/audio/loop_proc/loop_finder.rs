//! Rank loop candidates using correlation + MFCC similarity.

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

use crate::audio::loop_proc::analysis::positive_zero_crossings;
use crate::audio::loop_proc::constants::MFCC_FFT_SIZE;
use crate::audio::loop_proc::crossfade::cross_correlation;
use crate::audio::loop_proc::spectral::{cosine_similarity, mfcc_at};
use crate::audio::loop_proc::{LoopCandidate, LoopMetrics};

// --- Geometry (frames / Hz) ---
/// Ignore pitch estimates below this (Hz) when converting to period length.
const FUNDAMENTAL_HZ_FLOOR: f32 = 20.0;
const MIN_PERIOD_SAMPLES: usize = 32;
/// At least this many mono samples required to search for a loop (matches MFCC FFT size).
const MIN_MONO_SAMPLES: usize = MFCC_FFT_SIZE;
/// Periodic loops: window length in fundamental periods.
const PERIODIC_LENGTH_PERIODS: usize = 8;
/// Cap loop window to this fraction of buffer length (numerator/denominator).
const MAX_REGION_FRAC_NUM: usize = 4;
const MAX_REGION_FRAC_DEN: usize = 5;
const MARGIN_DIVISOR: usize = 10;
/// If `end` is too close to `start` after first pass, extend by `period_samples *` this.
const END_EXTEND_PERIOD_MULT: usize = 4;
const MIN_START_END_GAP: usize = 32;
const CROSS_REGION_MIN: usize = 32;
const CROSS_REGION_MAX: usize = 256;
const MIN_OVERLAP_FRAMES: usize = 8;
const MEAN_ABS_EPS: f32 = 1e-6;

// --- Correlation gates (zero-crossing quality) ---
const CORR_STRONG: f32 = 0.5;
const ZQ_IF_STRONG: f32 = 0.8;
const ZQ_IF_WEAK: f32 = 0.3;
const SLOPE_MATCH_SCALE: f32 = 0.9;

// --- Score weights (sum = 1.0) ---
const SCORE_W_CORRELATION: f32 = 0.35;
const SCORE_W_AMDF_INV: f32 = 0.25;
const SCORE_W_SPECTRAL: f32 = 0.25;
const SCORE_W_ZERO_CROSS: f32 = 0.10;
const SCORE_W_SLOPE: f32 = 0.05;

/// Find a single strong candidate loop region in mono samples (frame indices into mono).
pub fn find_loop_candidate(
    mono: &[f32],
    sample_rate: u32,
    fundamental_hz: f32,
    periodic: bool,
) -> Option<LoopCandidate> {
    if mono.len() < MIN_MONO_SAMPLES {
        return None;
    }
    let period_samples = (sample_rate as f32 / fundamental_hz.max(FUNDAMENTAL_HZ_FLOOR)).round() as usize;
    let period_samples = period_samples.clamp(MIN_PERIOD_SAMPLES, mono.len() / 4);
    let max_region = mono.len().saturating_mul(MAX_REGION_FRAC_NUM) / MAX_REGION_FRAC_DEN;
    let loop_frames = if periodic {
        (period_samples * PERIODIC_LENGTH_PERIODS).min(max_region)
    } else {
        // Half a second of frames at `sample_rate` (matches legacy `sample_rate / 2`).
        (sample_rate as usize / 2).min(max_region)
    }
    .max(period_samples * 2);

    let margin = mono.len() / MARGIN_DIVISOR;
    let mut start = margin;
    let zc = positive_zero_crossings(mono, margin, mono.len() - margin);
    if let Some(&z) = zc.first() {
        start = z;
    }
    let mut end = (start + loop_frames).min(mono.len() - margin);
    if end <= start + period_samples {
        end = (start + period_samples * END_EXTEND_PERIOD_MULT).min(mono.len());
    }
    if end <= start + MIN_START_END_GAP {
        return None;
    }

    let xf = period_samples.min(CROSS_REGION_MAX).max(CROSS_REGION_MIN);
    let tail_start = end.saturating_sub(xf).max(start);
    let head_end = (start + xf).min(end);
    if tail_start >= end || head_end <= start {
        return None;
    }
    let tail = &mono[tail_start..end];
    let head = &mono[start..head_end];
    let n = tail.len().min(head.len());
    if n < MIN_OVERLAP_FRAMES {
        return None;
    }
    let tail = &tail[tail.len() - n..];
    let head = &head[..n];
    let correlation = cross_correlation(tail, head);
    let mean_abs: f32 = tail.iter().map(|x| x.abs()).sum::<f32>() / n as f32;
    let diff_mean: f32 = tail
        .iter()
        .zip(head.iter())
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / n as f32;
    let amdf_norm = (diff_mean / mean_abs.max(MEAN_ABS_EPS)).clamp(0.0, 1.0);

    let m_tail = mfcc_at(mono, (tail_start + end) / 2, MFCC_FFT_SIZE, sample_rate);
    let m_head = mfcc_at(mono, (start + head_end) / 2, MFCC_FFT_SIZE, sample_rate);
    let spectral_similarity = cosine_similarity(&m_tail, &m_head).max(0.0);

    let zq = if correlation > CORR_STRONG {
        ZQ_IF_STRONG
    } else {
        ZQ_IF_WEAK
    };
    let slope = correlation * SLOPE_MATCH_SCALE;

    let metrics = LoopMetrics {
        correlation,
        amdf_score: amdf_norm,
        spectral_similarity,
        zero_crossing_quality: zq,
        slope_match: slope,
    };
    let score = SCORE_W_CORRELATION * correlation
        + SCORE_W_AMDF_INV * (1.0 - amdf_norm)
        + SCORE_W_SPECTRAL * spectral_similarity
        + SCORE_W_ZERO_CROSS * zq
        + SCORE_W_SLOPE * slope;

    Some(LoopCandidate {
        start,
        end,
        score: score.clamp(0.0, 1.0),
        metrics,
    })
}
