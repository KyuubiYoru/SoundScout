//! Export-time loop processing (trim, normalize, loop shaping). Source files are never modified.

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::module_name_repetitions)]

mod analysis;
mod crossfade;
mod loop_finder;
mod normalize;
mod smpl;
mod spectral;
mod spectral_blend;
mod split_swap;
mod trim;

pub use smpl::write_smpl_chunk;

use analysis::classify_mono;
use crossfade::{blend_loop_seam, cross_correlation, weld_loop_wrap_endpoints};

/// Serde for Tauri command args.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostProcessConfig {
    pub trim_silence: bool,
    pub trim_threshold_db: f32,
    pub trim_min_silence_ms: f32,
    pub normalize_peak: bool,
    pub normalize_target: f32,
    pub make_loopable: bool,
    pub crossfade_sec: Option<f32>,
    pub embed_smpl_chunk: bool,
}

#[derive(Debug)]
pub enum LoopTechnique {
    Passthrough,
    SplitSwapCrossfade { crossfade_samples: usize },
    PitchSynchronous { periods: u32, crossfade_samples: usize },
    SpectralBlend { fft_size: usize },
}

pub struct LoopResult {
    pub samples: Vec<f32>,
    pub loop_start: usize,
    pub loop_end: usize,
    pub technique: LoopTechnique,
}

pub struct LoopCandidate {
    pub start: usize,
    pub end: usize,
    pub score: f32,
    pub metrics: LoopMetrics,
}

pub struct LoopMetrics {
    pub correlation: f32,
    pub amdf_score: f32,
    pub spectral_similarity: f32,
    pub zero_crossing_quality: f32,
    pub slope_match: f32,
}

fn mono_from_interleaved(buf: &[f32], ch: usize) -> Vec<f32> {
    let frames = buf.len() / ch;
    (0..frames)
        .map(|f| (0..ch).map(|c| buf[f * ch + c]).sum::<f32>() / ch as f32)
        .collect()
}

/// Trim up to `max_shift` frames from the **start** and from the **end** (same frame counts on
/// every channel — required for interleaved PCM).  The best `(a, b)` minimizes **sum over
/// channels** of window mismatch + wrap jump, so stereo/LR are not forced to share a compromise
/// from a mono downmix (which can leave one channel misaligned at the loop).
fn trim_loop_seam_alignment(interleaved: &[f32], ch: usize, max_shift: usize) -> Vec<f32> {
    let frames = interleaved.len() / ch;
    if frames < 8 || max_shift == 0 || ch == 0 {
        return interleaved.to_vec();
    }
    let ms = max_shift.min(frames.saturating_sub(4) / 4).min(96);
    if ms == 0 {
        return interleaved.to_vec();
    }
    let win = (48usize).min(frames / 6).max(8);
    let mut best_a = 0usize;
    let mut best_b = 0usize;
    let mut best_err = f32::INFINITY;
    for a in 0..=ms {
        for b in 0..=ms {
            let inner = frames.saturating_sub(a + b);
            if inner < win.saturating_add(1) {
                continue;
            }
            let t_start = frames.saturating_sub(b).saturating_sub(win);
            if t_start < a {
                continue;
            }
            let mut err = 0f32;
            for c in 0..ch {
                for i in 0..win {
                    let ti = (t_start + i) * ch + c;
                    let hi = (a + i) * ch + c;
                    let d = interleaved[ti] - interleaved[hi];
                    err += d * d;
                }
                let li = (frames - 1 - b) * ch + c;
                let fi = a * ch + c;
                let wrap = interleaved[li] - interleaved[fi];
                err += wrap * wrap * win as f32;
            }
            if err < best_err {
                best_err = err;
                best_a = a;
                best_b = b;
            }
        }
    }
    interleaved[best_a * ch..(frames - best_b) * ch].to_vec()
}

/// Largest crossfade length (frames) allowed by split-swap / seam blend geometry.
#[inline]
fn max_crossfade_frames_for_clip(clip_frames: usize) -> usize {
    if clip_frames < 4 {
        0
    } else {
        clip_frames / 2 - 1
    }
}

/// Crossfade length in frames for seam blend (`blend_loop_seam` / spectral).
/// Manual duration is capped to what the clip can support; **auto** uses the largest crossfade
/// the clip geometry allows (`clip_frames / 2 - 1` frames).
fn crossfade_frames_for_clip(
    crossfade_sec: Option<f32>,
    sample_rate: u32,
    clip_frames: usize,
) -> usize {
    let clip_cap = max_crossfade_frames_for_clip(clip_frames);
    if clip_cap == 0 {
        return 0;
    }
    let xf = match crossfade_sec {
        Some(s) => {
            let want = (s * sample_rate as f32).round() as usize;
            want.min(clip_cap)
        }
        None => clip_cap,
    };
    let lower = if clip_cap >= 8 { 8 } else { 2 };
    xf.clamp(lower.min(clip_cap), clip_cap)
}

/// Duration in seconds for [`split_swap::split_swap_crossfade`]. **Auto** uses the longest
/// crossfade the clip allows (same cap as frame-based helpers). Manual picks are clamped to UI
/// range and to the clip maximum.
fn split_swap_crossfade_sec(crossfade_sec: Option<f32>, clip_frames: usize, sample_rate: u32) -> f32 {
    let cap = max_crossfade_frames_for_clip(clip_frames);
    let max_sec = if cap == 0 {
        0.05
    } else {
        (cap as f32 / sample_rate as f32).max(0.05)
    };
    match crossfade_sec {
        None => max_sec,
        Some(s) => s.clamp(0.5, 3.0).min(max_sec).max(0.05),
    }
}

/// Full post-process pipeline on an already-sliced interleaved buffer.
pub fn apply(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    config: &PostProcessConfig,
) -> Result<LoopResult, String> {
    if samples.is_empty() {
        return Err("no audio samples".to_string());
    }
    let ch = usize::from(channels);
    if ch == 0 || samples.len() % ch != 0 {
        return Err("invalid sample buffer".to_string());
    }

    let mut buf = samples.to_vec();

    if config.trim_silence {
        let mono = mono_from_interleaved(&buf, ch);
        let (f0, f1) = trim::detect_silence_bounds(
            &mono,
            sample_rate,
            config.trim_threshold_db,
            config.trim_min_silence_ms,
        );
        buf = trim::trim_interleaved(&buf, ch, f0, f1);
    }

    if buf.is_empty() {
        return Err("no audio after trim".to_string());
    }

    if config.normalize_peak {
        normalize::peak_normalize(&mut buf, config.normalize_target);
    }

    let frames = buf.len() / ch;

    if !config.make_loopable {
        return Ok(LoopResult {
            samples: buf,
            loop_start: 0,
            loop_end: frames.saturating_sub(1),
            technique: LoopTechnique::Passthrough,
        });
    }

    let mono = mono_from_interleaved(&buf, ch);
    let (_peak, hz, kind) = classify_mono(&mono, sample_rate);
    let period_samples = (sample_rate as f32 / hz.max(20.0)).round() as usize;
    let is_periodic = kind == 2;
    let is_quasi = kind == 1;

    let loop_result = if kind == 0 {
        let xf_sec = split_swap_crossfade_sec(config.crossfade_sec, frames, sample_rate);
        split_swap::split_swap_crossfade(&buf, sample_rate, channels, xf_sec)
    } else if let Some(cand) = loop_finder::find_loop_candidate(&mono, sample_rate, hz, is_periodic) {
        let start = cand.start;
        let end = cand.end;
        if end * ch <= buf.len() && start < end {
            let mut slice = buf[start * ch..end * ch].to_vec();
            let sub_frames = slice.len() / ch;
            let xf = crossfade_frames_for_clip(config.crossfade_sec, sample_rate, sub_frames);
            // `find_loop_candidate` returns a *window* (often ~0.5 s for quasi-periodic, or
            // `period * 8` for periodic).  With crossfade "auto", `xf` is small, so
            // `xf * 2 < sub_frames` is true and we used to export **only that window** — a tiny
            // WAV.  A fixed crossfade (e.g. 0.5 s) makes `xf` large, the condition fails, and
            // `split_swap` runs on the **full** clip instead.  Only blend inside the candidate
            // when it covers almost the entire export buffer; otherwise reshape the full clip.
            let candidate_covers_export =
                sub_frames.saturating_mul(10) >= frames.saturating_mul(9);
            if xf > 0 && xf * 2 < sub_frames && candidate_covers_export {
                let tail_s = end.saturating_sub(xf).max(start);
                let head_e = (start + xf).min(end);
                let tail_m = &mono[tail_s..end];
                let head_m = &mono[start..head_e];
                let n = tail_m.len().min(head_m.len());
                let r = if n > 8 {
                    cross_correlation(&tail_m[tail_m.len() - n..], &head_m[..n])
                } else {
                    0.0
                };

                if is_quasi && r < 0.75 {
                    spectral_blend::blend_seam_equal_power_long(&mut slice, ch, xf).map(|()| {
                        LoopResult {
                            samples: slice,
                            loop_start: 0,
                            loop_end: sub_frames.saturating_sub(1),
                            technique: LoopTechnique::SpectralBlend { fft_size: 512 },
                        }
                    })
                } else {
                    blend_loop_seam(&mut slice, ch, xf, r).map(|()| {
                        let periods = (sub_frames / period_samples.max(1)) as u32;
                        LoopResult {
                            samples: slice,
                            loop_start: 0,
                            loop_end: sub_frames.saturating_sub(1),
                            technique: LoopTechnique::PitchSynchronous {
                                periods: periods.max(1),
                                crossfade_samples: xf * ch,
                            },
                        }
                    })
                }
            } else {
                let xf_sec = split_swap_crossfade_sec(config.crossfade_sec, frames, sample_rate);
                split_swap::split_swap_crossfade(&buf, sample_rate, channels, xf_sec)
            }
        } else {
            let xf_sec = split_swap_crossfade_sec(config.crossfade_sec, frames, sample_rate);
            split_swap::split_swap_crossfade(&buf, sample_rate, channels, xf_sec)
        }
    } else {
        let xf_sec = split_swap_crossfade_sec(config.crossfade_sec, frames, sample_rate);
        split_swap::split_swap_crossfade(&buf, sample_rate, channels, xf_sec)
    };

    let mut result = loop_result.or_else(|_| -> Result<LoopResult, String> {
        let frames = buf.len() / ch;
        Ok(LoopResult {
            samples: buf,
            loop_start: 0,
            loop_end: frames.saturating_sub(1),
            technique: LoopTechnique::Passthrough,
        })
    })?;

    if !matches!(result.technique, LoopTechnique::Passthrough) {
        let out_frames = result.samples.len() / ch;
        let ms = 64.min(out_frames.saturating_sub(4) / 4);
        if ms > 0 {
            result.samples = trim_loop_seam_alignment(&result.samples, ch, ms);
            let nf = result.samples.len() / ch;
            result.loop_end = nf.saturating_sub(1);
        }
        // File loop is … last frame → first frame.  Align seam per channel (trim search + weld).
        weld_loop_wrap_endpoints(&mut result.samples, ch);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn sine_samples(freq_hz: f32, sr: u32, duration_sec: f32) -> Vec<f32> {
        let n = ((sr as f32 * duration_sec).round() as usize).max(100);
        let two_pi = 2.0 * std::f32::consts::PI;
        (0..n)
            .map(|i| (two_pi * freq_hz * i as f32 / sr as f32).sin())
            .collect()
    }

    fn white_noise_samples(sr: u32, duration_sec: f32) -> Vec<f32> {
        let n = ((sr as f32 * duration_sec).round() as usize).max(8000);
        let mut x = 0xC0FFEE_BABEu64;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                ((x >> 32) as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect()
    }

    #[test]
    fn sine_440_is_periodic() {
        let sine = sine_samples(440.0, 44100, 2.0);
        let (_p, _hz, kind) = classify_mono(&sine, 44100);
        assert_eq!(kind, 2, "expected periodic classification");
    }

    #[test]
    fn noise_is_aperiodic() {
        let noise = white_noise_samples(44100, 4.0);
        let (_p, _hz, kind) = classify_mono(&noise, 44100);
        assert_ne!(kind, 2, "noise should not classify as strongly periodic");
    }

    #[test]
    fn split_swap_loop_points() {
        let noise = white_noise_samples(44100, 2.0);
        let interleaved: Vec<f32> = noise.iter().copied().collect();
        let result = split_swap::split_swap_crossfade(&interleaved, 44100, 1, 1.0).unwrap();
        assert_eq!(result.loop_start, 0);
        assert_eq!(result.loop_end, result.samples.len() - 1);
        assert!(matches!(
            result.technique,
            LoopTechnique::SplitSwapCrossfade { .. }
        ));
    }

    /// Read a 32-bit IEEE-float WAV (the only format we test with) and return
    /// (interleaved f32 samples, sample_rate, channels).
    fn read_f32_wav(path: &str) -> (Vec<f32>, u32, u16) {
        let mut file = std::fs::File::open(path).expect("test WAV not found");
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        // RIFF header
        assert_eq!(&buf[0..4], b"RIFF");
        assert_eq!(&buf[8..12], b"WAVE");
        let mut pos = 12usize;
        let mut sample_rate = 0u32;
        let mut channels = 0u16;
        let mut pcm: Vec<f32> = Vec::new();
        while pos + 8 <= buf.len() {
            let tag = &buf[pos..pos + 4];
            let sz = u32::from_le_bytes(buf[pos + 4..pos + 8].try_into().unwrap()) as usize;
            pos += 8;
            match tag {
                b"fmt " => {
                    // audio format = 3 (IEEE float)
                    channels = u16::from_le_bytes(buf[pos + 2..pos + 4].try_into().unwrap());
                    sample_rate = u32::from_le_bytes(buf[pos + 4..pos + 8].try_into().unwrap());
                }
                b"data" => {
                    let n_samples = sz / 4;
                    pcm.reserve(n_samples);
                    for i in 0..n_samples {
                        let b4: [u8; 4] = buf[pos + i * 4..pos + i * 4 + 4].try_into().unwrap();
                        pcm.push(f32::from_le_bytes(b4));
                    }
                }
                _ => {}
            }
            pos += sz + (sz & 1); // chunks are word-aligned
        }
        assert!(sample_rate > 0 && channels > 0 && !pcm.is_empty(), "WAV parse failed");
        (pcm, sample_rate, channels)
    }

    /// Loop wrap must be sample-continuous per channel after weld.
    fn assert_no_loop_click(result: &LoopResult, ch: usize) {
        let frames = result.samples.len() / ch;
        assert!(frames > 0, "empty output");
        for c in 0..ch {
            let first = result.samples[c];
            let last = result.samples[(frames - 1) * ch + c];
            let jump = (first - last).abs();
            assert!(
                jump < 1e-3,
                "channel {c}: loop wrap jump |first−last|={jump} (expected ~0 after weld)"
            );
        }
    }

    /// Regression: crossfade "auto" used to use a small `xf`, satisfy `xf * 2 < sub_frames`, and
    /// then export **only** the loop-finder window (often ~0.5 s or `period * 8` frames) — a tiny
    /// WAV.  Fixed by requiring the candidate to cover ≥90% of the clip before that path runs.
    #[test]
    fn make_loopable_auto_crossfade_export_not_tiny_slice() {
        let sr = 44100u32;
        let sine = sine_samples(220.0, sr, 5.0);
        let in_frames = sine.len();
        let cfg = PostProcessConfig {
            trim_silence: false,
            trim_threshold_db: -60.0,
            trim_min_silence_ms: 50.0,
            normalize_peak: false,
            normalize_target: 0.97,
            make_loopable: true,
            crossfade_sec: None,
            embed_smpl_chunk: false,
        };
        let result = apply(&sine, sr, 1, &cfg).expect("apply");
        let out_frames = result.samples.len();
        assert!(
            out_frames * 10 >= in_frames * 5,
            "expected at least half the input length (split-swap full clip), got in={in_frames} out={out_frames}"
        );
    }

    #[test]
    fn blend_loop_seam_click_free_sine() {
        // 220 Hz sine for 3 s — classified as periodic → PitchSynchronous path.
        let sr = 44100u32;
        let sine = sine_samples(220.0, sr, 3.0);
        let cfg = PostProcessConfig {
            trim_silence: false,
            trim_threshold_db: -60.0,
            trim_min_silence_ms: 50.0,
            normalize_peak: false,
            normalize_target: 0.97,
            make_loopable: true,
            crossfade_sec: Some(0.2),
            embed_smpl_chunk: false,
        };
        let result = apply(&sine, sr, 1, &cfg).expect("apply failed");
        assert_no_loop_click(&result, 1);
    }

    #[test]
    fn blend_loop_seam_click_free_real_file() {
        let wav_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../docs/Alien Spaceship Filtered, Rumble_58.344-61.179s.wav"
        );
        if !std::path::Path::new(wav_path).exists() {
            eprintln!("skipping real-file click test: WAV not found at {wav_path}");
            return;
        }
        let (pcm, sr, ch) = read_f32_wav(wav_path);
        let cfg = PostProcessConfig {
            trim_silence: false,
            trim_threshold_db: -60.0,
            trim_min_silence_ms: 50.0,
            normalize_peak: true,
            normalize_target: 0.97,
            make_loopable: true,
            crossfade_sec: Some(0.5),
            embed_smpl_chunk: false,
        };
        let result = apply(&pcm, sr, ch, &cfg).expect("apply failed on real file");
        assert_no_loop_click(&result, ch as usize);
    }

    #[test]
    fn smpl_chunk_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.wav");
        let mut f = std::fs::File::create(&path).unwrap();
        let data_bytes = 4410 * 4u32;
        let riff_payload = 4u32 + (8 + 16) + (8 + data_bytes) + 8 + 68;
        f.write_all(b"RIFF").unwrap();
        f.write_all(&riff_payload.to_le_bytes()).unwrap();
        f.write_all(b"WAVE").unwrap();
        f.write_all(b"fmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&3u16.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&44100u32.to_le_bytes()).unwrap();
        f.write_all(&(176_400u32).to_le_bytes()).unwrap();
        f.write_all(&4u16.to_le_bytes()).unwrap();
        f.write_all(&32u16.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data_bytes.to_le_bytes()).unwrap();
        f.write_all(&vec![0u8; data_bytes as usize]).unwrap();
        write_smpl_chunk(&mut f, 44100, 100, 4309).unwrap();
        drop(f);

        let bytes = std::fs::read(&path).unwrap();
        let smpl_pos = bytes
            .windows(4)
            .position(|w| w == b"smpl")
            .expect("smpl chunk missing");
        let loop_start =
            u32::from_le_bytes(bytes[smpl_pos + 52..smpl_pos + 56].try_into().unwrap());
        let loop_end =
            u32::from_le_bytes(bytes[smpl_pos + 56..smpl_pos + 60].try_into().unwrap());
        assert_eq!(loop_start, 100);
        assert_eq!(loop_end, 4309);
    }

}
