//! Rank loop candidates using correlation + MFCC similarity.

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

use crate::audio::loop_proc::analysis::positive_zero_crossings;
use crate::audio::loop_proc::crossfade::cross_correlation;
use crate::audio::loop_proc::spectral::{cosine_similarity, mfcc_at};
use crate::audio::loop_proc::{LoopCandidate, LoopMetrics};

const FFT_SIZE: usize = 512;

/// Find a single strong candidate loop region in mono samples (frame indices into mono).
pub fn find_loop_candidate(
    mono: &[f32],
    sample_rate: u32,
    fundamental_hz: f32,
    periodic: bool,
) -> Option<LoopCandidate> {
    if mono.len() < 512 {
        return None;
    }
    let period_samples = (sample_rate as f32 / fundamental_hz.max(20.0)).round() as usize;
    let period_samples = period_samples.clamp(32, mono.len() / 4);
    let loop_frames = if periodic {
        (period_samples * 8).min(mono.len() * 4 / 5)
    } else {
        (sample_rate as usize / 2).min(mono.len() * 4 / 5)
    }
    .max(period_samples * 2);

    let margin = mono.len() / 10;
    let mut start = margin;
    let zc = positive_zero_crossings(mono, margin, mono.len() - margin);
    if let Some(&z) = zc.first() {
        start = z;
    }
    let mut end = (start + loop_frames).min(mono.len() - margin);
    if end <= start + period_samples {
        end = (start + period_samples * 4).min(mono.len());
    }
    if end <= start + 32 {
        return None;
    }

    let xf = period_samples.min(256).max(32);
    let tail_start = end.saturating_sub(xf).max(start);
    let head_end = (start + xf).min(end);
    if tail_start >= end || head_end <= start {
        return None;
    }
    let tail = &mono[tail_start..end];
    let head = &mono[start..head_end];
    let n = tail.len().min(head.len());
    if n < 8 {
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
    let amdf_norm = (diff_mean / mean_abs.max(1e-6)).clamp(0.0, 1.0);

    let m_tail = mfcc_at(mono, (tail_start + end) / 2, FFT_SIZE, sample_rate);
    let m_head = mfcc_at(mono, (start + head_end) / 2, FFT_SIZE, sample_rate);
    let spectral_similarity = cosine_similarity(&m_tail, &m_head).max(0.0);

    let zq = if correlation > 0.5 { 0.8 } else { 0.3 };
    let slope = correlation * 0.9;

    let metrics = LoopMetrics {
        correlation,
        amdf_score: amdf_norm,
        spectral_similarity,
        zero_crossing_quality: zq,
        slope_match: slope,
    };
    let score = 0.35 * correlation
        + 0.25 * (1.0 - amdf_norm)
        + 0.25 * spectral_similarity
        + 0.10 * zq
        + 0.05 * slope;

    Some(LoopCandidate {
        start,
        end,
        score: score.clamp(0.0, 1.0),
        metrics,
    })
}
