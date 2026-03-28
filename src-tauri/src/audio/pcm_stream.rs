//! Chunked PCM decode to temp files for low time-to-first-audio.

use std::cell::Cell;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use bytemuck;
use tokio::sync::mpsc::UnboundedSender;

use crate::audio::decode_core;
use crate::audio::wav_linear;

/// Seconds of audio per chunk (WAV / Symphonia streaming).
pub const PCM_CHUNK_SECS: u32 = 2;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PcmStreamChunkMsg {
    pub stream_id: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub chunk_index: u32,
    pub path: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PcmStreamFinished {
    pub stream_id: u64,
}

fn write_chunk_file(stream_id: u64, index: u32, samples: &[f32]) -> Result<String, String> {
    let p = std::env::temp_dir().join(format!("svstream-{stream_id}-{index}.pcm"));
    let slice = bytemuck::cast_slice(samples);
    std::fs::write(&p, slice).map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().to_string())
}

fn send_chunk(
    tx: &UnboundedSender<PcmStreamChunkMsg>,
    stream_id: u64,
    sample_rate: u32,
    channels: u16,
    idx: u32,
    samples: &[f32],
) -> Result<(), String> {
    let path = write_chunk_file(stream_id, idx, samples)?;
    tx.send(PcmStreamChunkMsg {
        stream_id,
        sample_rate,
        channels,
        chunk_index: idx,
        path,
    })
    .map_err(|_| "pcm stream channel closed".to_string())
}

/// Decode `path` in chunks; sends each [`PcmStreamChunkMsg`] on `tx` (blocking thread).
pub fn run_pcm_stream(
    path: PathBuf,
    stream_id: u64,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    tx: UnboundedSender<PcmStreamChunkMsg>,
) -> Result<(), String> {
    if let Some((layout, mut file)) = wav_linear::open_wav_pcm_source(&path) {
        let sr = layout.sample_rate;
        let ch = layout.channels;
        let baf = usize::from(layout.block_align);
        let chunk_frames = u64::from(PCM_CHUNK_SECS)
            .saturating_mul(u64::from(sr))
            .max(1);
        let chunk_bytes_u64 = chunk_frames
            .checked_mul(u64::from(layout.block_align))
            .ok_or("chunk size overflow")?;
        let chunk_bytes = usize::try_from(chunk_bytes_u64).map_err(|_| "chunk too large")?;
        let mut remaining = usize::try_from(layout.data_total_bytes).map_err(|_| "wav too large")?;
        let mut read_buf = vec![0u8; chunk_bytes.max(baf).min(32 * 1024 * 1024)];
        let mut idx: u32 = 0;

        while remaining > 0 {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            let n_raw = chunk_bytes.min(remaining);
            let n = (n_raw / baf) * baf;
            if n == 0 {
                return Err("wav data not frame-aligned".into());
            }
            file
                .read_exact(&mut read_buf[..n])
                .map_err(|e| e.to_string())?;
            let mut samples = Vec::new();
            wav_linear::decode_wav_bytes_to_f32(
                layout.audio_format,
                layout.bits_per_sample,
                baf,
                &read_buf[..n],
                &mut samples,
            )
            .map_err(|_| "wav decode".to_string())?;
            send_chunk(&tx, stream_id, sr, ch, idx, &samples)?;
            idx += 1;
            remaining -= n;
        }
        return Ok(());
    }

    let path_ref: &Path = path.as_ref();
    let meta_sr = Cell::new(0u32);
    let meta_ch = Cell::new(0u16);
    decode_core::stream_symphonia_pcm(
        path_ref,
        PCM_CHUNK_SECS,
        cancel.as_ref(),
        |sr, ch| {
            meta_sr.set(sr);
            meta_ch.set(ch);
        },
        |i, data| {
            send_chunk(
                &tx,
                stream_id,
                meta_sr.get(),
                meta_ch.get(),
                i as u32,
                &data,
            )
        },
    )
    .map_err(|e| e.to_string())
}
