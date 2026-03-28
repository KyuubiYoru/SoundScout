//! Shared decode path (peaks + playback): fast linear WAV, else Symphonia.

use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphErr;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::audio::wav_linear;
use crate::error::SoundScoutError;

/// Decode full file to interleaved f32 samples (stereo: L,R,L,R,…).
pub fn decode_samples(path: &Path) -> Result<(Vec<f32>, u32, u16), SoundScoutError> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
    {
        if let Some(ok) = wav_linear::try_decode_wav_linear_pcm(path) {
            return Ok(ok);
        }
    }
    decode_samples_symphonia(path)
}

fn decode_samples_symphonia(path: &Path) -> Result<(Vec<f32>, u32, u16), SoundScoutError> {
    let file = File::open(path).map_err(SoundScoutError::Io)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| SoundScoutError::AudioDecode(e.to_string()))?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| SoundScoutError::AudioDecode("no default track".into()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| SoundScoutError::AudioDecode(e.to_string()))?;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| SoundScoutError::AudioDecode("unknown sample rate".into()))?;
    let channels = track
        .codec_params
        .channels
        .map(|c| u16::try_from(c.count()).unwrap_or(1))
        .unwrap_or(1);

    let mut samples: Vec<f32> = Vec::new();
    if let Some(n_frames) = track.codec_params.n_frames {
        let ch = usize::from(channels);
        let cap = (n_frames as usize).saturating_mul(ch);
        samples.reserve_exact(cap);
    }

    let mut scratch: Option<SampleBuffer<f32>> = None;
    let mut scratch_spec: Option<symphonia::core::audio::SignalSpec> = None;
    let mut scratch_frame_cap: u64 = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphErr::ResetRequired) => continue,
            Err(SymphErr::IoError(_)) => break,
            Err(e) => return Err(SoundScoutError::AudioDecode(e.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphErr::DecodeError(_)) => continue,
            Err(e) => return Err(SoundScoutError::AudioDecode(e.to_string())),
        };
        append_interleaved_f32(
            &mut samples,
            decoded,
            &mut scratch,
            &mut scratch_spec,
            &mut scratch_frame_cap,
        )?;
    }

    if samples.is_empty() {
        return Err(SoundScoutError::AudioDecode("no audio decoded".into()));
    }

    Ok((samples, sample_rate, channels))
}

fn append_interleaved_f32(
    out: &mut Vec<f32>,
    buf: AudioBufferRef<'_>,
    scratch: &mut Option<SampleBuffer<f32>>,
    scratch_spec: &mut Option<symphonia::core::audio::SignalSpec>,
    scratch_frame_cap: &mut u64,
) -> Result<(), SoundScoutError> {
    let spec = *buf.spec();
    let frames = u64::try_from(buf.capacity()).unwrap_or(0);
    if frames == 0 {
        return Ok(());
    }
    let n_ch = u64::try_from(spec.channels.count()).unwrap_or(0);
    let needed_cap = (frames * n_ch) as usize;

    let need_new = match (&scratch, scratch_spec.as_ref(), *scratch_frame_cap) {
        (Some(b), Some(sp), fc) => sp != &spec || frames > fc || b.capacity() < needed_cap,
        _ => true,
    };
    if need_new {
        *scratch = Some(SampleBuffer::<f32>::new(frames, spec));
        *scratch_spec = Some(spec);
        *scratch_frame_cap = frames;
    }
    let Some(sample_buf) = scratch.as_mut() else {
        return Err(SoundScoutError::AudioDecode("sample scratch missing".into()));
    };
    sample_buf.copy_interleaved_ref(buf);
    out.extend_from_slice(sample_buf.samples());
    Ok(())
}

/// Stream interleaved `f32` PCM in time chunks via Symphonia. `on_meta` runs once before any chunk.
pub(crate) fn stream_symphonia_pcm(
    path: &Path,
    chunk_duration_secs: u32,
    cancel: &AtomicBool,
    mut on_meta: impl FnMut(u32, u16),
    mut on_chunk: impl FnMut(usize, Vec<f32>) -> Result<(), String>,
) -> Result<(), SoundScoutError> {
    let file = File::open(path).map_err(SoundScoutError::Io)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| SoundScoutError::AudioDecode(e.to_string()))?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| SoundScoutError::AudioDecode("no default track".into()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| SoundScoutError::AudioDecode(e.to_string()))?;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| SoundScoutError::AudioDecode("unknown sample rate".into()))?;
    let channels = track
        .codec_params
        .channels
        .map(|c| u16::try_from(c.count()).unwrap_or(1))
        .unwrap_or(1);

    on_meta(sample_rate, channels);

    let ch = usize::from(channels);
    let chunk_frames = u64::from(chunk_duration_secs)
        .saturating_mul(u64::from(sample_rate))
        .max(1);
    let target = usize::try_from(chunk_frames)
        .unwrap_or(usize::MAX)
        .saturating_mul(ch)
        .max(ch);

    let mut batch: Vec<f32> = Vec::with_capacity(target.min(262_144));
    let mut scratch: Option<SampleBuffer<f32>> = None;
    let mut scratch_spec: Option<symphonia::core::audio::SignalSpec> = None;
    let mut scratch_frame_cap: u64 = 0;
    let mut idx = 0usize;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphErr::ResetRequired) => continue,
            Err(SymphErr::IoError(_)) => break,
            Err(e) => return Err(SoundScoutError::AudioDecode(e.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphErr::DecodeError(_)) => continue,
            Err(e) => return Err(SoundScoutError::AudioDecode(e.to_string())),
        };
        append_interleaved_f32(
            &mut batch,
            decoded,
            &mut scratch,
            &mut scratch_spec,
            &mut scratch_frame_cap,
        )?;
        while batch.len() >= target {
            let tail: Vec<f32> = batch.drain(..target).collect();
            on_chunk(idx, tail).map_err(SoundScoutError::AudioDecode)?;
            idx += 1;
        }
    }

    while batch.len() >= target {
        let tail: Vec<f32> = batch.drain(..target).collect();
        on_chunk(idx, tail).map_err(SoundScoutError::AudioDecode)?;
        idx += 1;
    }
    if !batch.is_empty() {
        on_chunk(idx, std::mem::take(&mut batch)).map_err(SoundScoutError::AudioDecode)?;
    } else if idx == 0 {
        return Err(SoundScoutError::AudioDecode("no audio decoded".into()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::write_test_wav;
    use tempfile::TempDir;

    #[test]
    fn decodes_wav_mono() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("m.wav");
        write_test_wav(&p, 44_100, 1, 16, 1000, 440.0).expect("w");
        let (s, sr, ch) = decode_samples(&p).expect("dec");
        assert_eq!(sr, 44_100);
        assert_eq!(ch, 1);
        assert!(!s.is_empty());
    }

    #[test]
    fn decodes_wav_stereo() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("s.wav");
        write_test_wav(&p, 48_000, 2, 16, 500, 100.0).expect("w");
        let (samples, sr, ch) = decode_samples(&p).expect("dec");
        assert_eq!(ch, 2);
        assert_eq!(samples.len() % usize::from(ch), 0);
        assert_eq!(sr, 48_000);
    }

    #[test]
    fn samples_in_valid_range() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("m.wav");
        write_test_wav(&p, 8000, 1, 16, 200, 440.0).expect("w");
        let (s, _, _) = decode_samples(&p).expect("dec");
        assert!(s.iter().all(|&x| (-1.01..=1.01).contains(&x)));
    }

    #[test]
    fn corrupt_file_returns_error() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("bad.wav");
        std::fs::write(&p, b"not a wav").unwrap();
        assert!(decode_samples(&p).is_err());
    }

    #[test]
    fn empty_file_returns_error() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("e.wav");
        std::fs::write(&p, []).unwrap();
        assert!(decode_samples(&p).is_err());
    }
}
