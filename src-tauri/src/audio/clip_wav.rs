//! Slice decoded [`PcmData`](crate::db::models::PcmData) and write IEEE float WAV (format 3).

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::audio::loop_proc::{self, LoopTechnique};
use crate::db::models::PcmData;

/// Minimum clip duration in seconds (aligned with the UI `CLIP_MIN_SEC`).
pub const CLIP_MIN_SEC: f64 = 0.05;

fn sanitize_source_stem(source_filename: &str) -> String {
    let stem = Path::new(source_filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("clip");
    let safe: String = stem
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .take(120)
        .collect();
    let stem = safe.trim();
    if stem.is_empty() || stem == "." {
        "clip".to_string()
    } else {
        stem.trim_end_matches('.').to_string()
    }
}

/// Default save-as name for a full-file export: `{sanitized_stem}.wav`.
pub fn export_full_wav_suggested_filename(source_filename: &str) -> String {
    format!("{}.wav", sanitize_source_stem(source_filename))
}

/// Default save-as name for a clip: `{sanitized_stem}_{start}-{end}s.wav`.
pub fn export_clip_suggested_filename(source_filename: &str, start_sec: f64, end_sec: f64) -> String {
    let stem = sanitize_source_stem(source_filename);
    format!("{stem}_{start_sec:.3}-{end_sec:.3}s.wav")
}

/// Temp path for clipboard “paste as file”: same basename as export; `_2`, `_3`, … if that name already exists in temp.
pub fn clipboard_temp_wav_path(source_filename: &str, is_clip: bool, start_sec: f64, end_sec: f64) -> PathBuf {
    let basename = if is_clip {
        export_clip_suggested_filename(source_filename, start_sec, end_sec)
    } else {
        export_full_wav_suggested_filename(source_filename)
    };
    unique_temp_wav_with_basename(&basename)
}

fn unique_temp_wav_with_basename(basename: &str) -> PathBuf {
    let dir = std::env::temp_dir();
    let first = dir.join(basename);
    if !first.exists() {
        return first;
    }
    let stem = Path::new(basename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("clip");
    for n in 2u32..10_000 {
        let candidate = dir.join(format!("{stem}_{n}.wav"));
        if !candidate.exists() {
            return candidate;
        }
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    dir.join(format!("{stem}_soundscout_{ts}.wav"))
}

/// Write the entire decoded buffer as IEEE float WAV.
pub fn write_full_wav(
    pcm: &PcmData,
    out: &Path,
    post_process: Option<&loop_proc::PostProcessConfig>,
) -> Result<(), String> {
    if pcm.samples.is_empty() || pcm.channels == 0 {
        return Err("no audio samples".to_string());
    }
    write_inner(out, &pcm.samples, pcm.sample_rate, pcm.channels, post_process)
}

/// Inclusive start frame, exclusive end frame, into interleaved `samples`.
pub fn clip_frame_range(pcm: &PcmData, start_sec: f64, end_sec: f64) -> Result<(usize, usize), String> {
    if !start_sec.is_finite() || !end_sec.is_finite() {
        return Err("clip times must be finite".to_string());
    }
    if end_sec <= start_sec {
        return Err("clip end must be after start".to_string());
    }
    if end_sec - start_sec < CLIP_MIN_SEC {
        return Err(format!("clip must be at least {CLIP_MIN_SEC} seconds"));
    }

    let ch = usize::from(pcm.channels);
    if ch == 0 {
        return Err("invalid channel count".to_string());
    }

    let total_frames = pcm.samples.len() / ch;
    if total_frames == 0 {
        return Err("no audio samples".to_string());
    }

    let sr = f64::from(pcm.sample_rate);
    let start_frame = (start_sec * sr).floor() as i64;
    let end_frame = (end_sec * sr).ceil() as i64;

    let max_f = total_frames as i64;
    let sf = start_frame.clamp(0, max_f.saturating_sub(1)) as usize;
    let mut ef = end_frame.clamp(1, max_f) as usize;
    if ef <= sf {
        ef = (sf + 1).min(total_frames);
    }
    if ef > total_frames {
        ef = total_frames;
    }
    if ef <= sf {
        return Err("clip range is empty after clamping".to_string());
    }

    let frames = ef - sf;
    let dur_sec = frames as f64 / sr;
    if dur_sec < CLIP_MIN_SEC - 1e-6 {
        return Err(format!(
            "clip must be at least {CLIP_MIN_SEC} seconds (got {dur_sec:.3}s after clamping to file length)"
        ));
    }

    Ok((sf, ef))
}

/// Write `[start_frame, end_frame)` as IEEE float WAV (interleaved f32 LE).
pub fn write_clip_wav(
    pcm: &PcmData,
    start_sec: f64,
    end_sec: f64,
    out: &Path,
    post_process: Option<&loop_proc::PostProcessConfig>,
) -> Result<(), String> {
    let (sf, ef) = clip_frame_range(pcm, start_sec, end_sec)?;
    let ch = usize::from(pcm.channels);
    let slice = &pcm.samples[sf * ch..ef * ch];
    write_inner(out, slice, pcm.sample_rate, pcm.channels, post_process)
}

fn write_inner(
    path: &Path,
    interleaved: &[f32],
    sample_rate: u32,
    channels: u16,
    post_process: Option<&loop_proc::PostProcessConfig>,
) -> Result<(), String> {
    match post_process {
        None => write_f32_wav_with_smpl(path, interleaved, sample_rate, channels, None),
        Some(cfg) => {
            let result = loop_proc::apply(interleaved, sample_rate, channels, cfg)?;
            let loop_pts =
                if cfg.embed_smpl_chunk && !matches!(result.technique, LoopTechnique::Passthrough) {
                    Some((result.loop_start as u32, result.loop_end as u32))
                } else {
                    None
                };
            write_f32_wav_with_smpl(
                path,
                &result.samples,
                sample_rate,
                channels,
                loop_pts,
            )
        }
    }
}

/// IEEE float WAV; optional `smpl` loop chunk after `data`.
pub fn write_f32_wav_with_smpl(
    path: &Path,
    interleaved_f32: &[f32],
    sample_rate: u32,
    channels: u16,
    loop_pts: Option<(u32, u32)>,
) -> Result<(), String> {
    let ch = usize::from(channels);
    if ch == 0 || interleaved_f32.len() % ch != 0 {
        return Err("invalid sample buffer".to_string());
    }
    let data_bytes = interleaved_f32.len() * 4;
    let block_align = channels.saturating_mul(4);
    let byte_rate = sample_rate.saturating_mul(u32::from(block_align));

    let smpl_extra = if loop_pts.is_some() { 8 + 68 } else { 0 };
    let riff_payload = 4u32 + (8 + 16) + (8 + data_bytes as u32) + smpl_extra;

    let mut f = File::create(path).map_err(|e| e.to_string())?;

    f.write_all(b"RIFF").map_err(|e| e.to_string())?;
    f.write_all(&riff_payload.to_le_bytes()).map_err(|e| e.to_string())?;
    f.write_all(b"WAVE").map_err(|e| e.to_string())?;

    f.write_all(b"fmt ").map_err(|e| e.to_string())?;
    f.write_all(&16u32.to_le_bytes()).map_err(|e| e.to_string())?;
    f.write_all(&3u16.to_le_bytes()).map_err(|e| e.to_string())?;
    f.write_all(&channels.to_le_bytes()).map_err(|e| e.to_string())?;
    f.write_all(&sample_rate.to_le_bytes()).map_err(|e| e.to_string())?;
    f.write_all(&byte_rate.to_le_bytes()).map_err(|e| e.to_string())?;
    f.write_all(&block_align.to_le_bytes()).map_err(|e| e.to_string())?;
    f.write_all(&32u16.to_le_bytes()).map_err(|e| e.to_string())?;

    f.write_all(b"data").map_err(|e| e.to_string())?;
    f.write_all(&(data_bytes as u32).to_le_bytes()).map_err(|e| e.to_string())?;
    let raw = bytemuck::cast_slice(interleaved_f32);
    f.write_all(raw).map_err(|e| e.to_string())?;

    if let Some((ls, le)) = loop_pts {
        loop_proc::write_smpl_chunk(&mut f, sample_rate, ls, le).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::decoder;
    use crate::test_utils::write_test_wav;
    use tempfile::TempDir;

    #[test]
    fn export_suggested_filename_includes_stem_and_times() {
        let n = export_clip_suggested_filename("My Track.flac", 1.0, 2.5);
        assert_eq!(n, "My Track_1.000-2.500s.wav");
        assert_eq!(export_clip_suggested_filename("weird/nested.mp3", 0.0, 0.05), "nested_0.000-0.050s.wav");
        assert_eq!(export_full_wav_suggested_filename("My Track.flac"), "My Track.wav");
        assert_eq!(export_full_wav_suggested_filename("x/nested.mp3"), "nested.wav");
    }

    #[test]
    fn clip_wav_writes_expected_size_and_header() {
        let dir = TempDir::new().expect("tmp");
        let src = dir.path().join("in.wav");
        write_test_wav(&src, 44_100, 1, 16, 4410, 100.0).expect("w");
        let pcm = decoder::decode_to_pcm(&src).expect("dec");

        let out = dir.path().join("clip.wav");
        write_clip_wav(&pcm, 0.0, 0.1, &out, None).expect("write");

        let bytes = std::fs::read(&out).expect("read");
        assert!(bytes.starts_with(b"RIFF"));
        assert_eq!(&bytes[8..12], b"WAVE");
        let data_size = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        assert_eq!(data_size, 17640);
        assert_eq!(bytes.len(), 44 + 17640);
    }

    #[test]
    fn clip_too_short_errors() {
        let dir = TempDir::new().expect("tmp");
        let src = dir.path().join("in.wav");
        write_test_wav(&src, 8000, 1, 16, 800, 100.0).expect("w");
        let pcm = decoder::decode_to_pcm(&src).expect("dec");
        let out = dir.path().join("clip.wav");
        let e = write_clip_wav(&pcm, 0.0, 0.02, &out, None).expect_err("short");
        assert!(e.contains("0.05") || e.contains("at least"));
    }

    #[test]
    fn write_full_wav_matches_clip_spanning_whole_file() {
        let dir = TempDir::new().expect("tmp");
        let src = dir.path().join("in.wav");
        write_test_wav(&src, 44_100, 1, 16, 44_100, 100.0).expect("w");
        let pcm = decoder::decode_to_pcm(&src).expect("dec");
        let full = dir.path().join("full.wav");
        write_full_wav(&pcm, &full, None).expect("full");
        let clip = dir.path().join("clip.wav");
        write_clip_wav(&pcm, 0.0, 1.0, &clip, None).expect("clip");
        assert_eq!(std::fs::read(&full).expect("r1"), std::fs::read(&clip).expect("r2"));
    }
}
