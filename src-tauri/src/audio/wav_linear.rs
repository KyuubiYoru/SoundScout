//! Fast decode for common RIFF WAVE linear PCM (and 32-bit float) — avoids Symphonia per-packet work.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const READ_CHUNK: usize = 4 * 1024 * 1024;

/// Parsed linear / float WAV `data` region (file must be positioned at first `data` byte after [`open_wav_pcm_source`]).
#[derive(Debug, Clone)]
pub(super) struct WavPcmLayout {
    pub sample_rate: u32,
    pub channels: u16,
    pub block_align: u16,
    pub audio_format: u16,
    pub bits_per_sample: u16,
    pub data_total_bytes: u64,
}

/// Open file seeked to start of `data` payload, or `None` if not a supported linear/float WAV.
pub(super) fn open_wav_pcm_source(path: &Path) -> Option<(WavPcmLayout, File)> {
    let mut f = File::open(path).ok()?;
    let mut riff = [0u8; 12];
    f.read_exact(&mut riff).ok()?;
    if riff.get(0..4) != Some(b"RIFF") || riff.get(8..12) != Some(b"WAVE") {
        return None;
    }

    let mut audio_format: u16 = 0;
    let mut channels: u16 = 0;
    let mut sample_rate: u32 = 0;
    let mut block_align: u16 = 0;
    let mut bits_per_sample: u16 = 0;
    let mut have_fmt = false;
    let mut data_offset: u64 = 0;
    let mut data_len: u64 = 0;
    let mut have_data = false;

    let mut hdr = [0u8; 8];
    while f.read_exact(&mut hdr).is_ok() {
        let id = hdr.get(0..4)?;
        let size = u32::from_le_bytes(hdr[4..8].try_into().ok()?);

        if id == b"fmt " {
            if size < 16 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            f.read_exact(&mut buf).ok()?;
            audio_format = u16::from_le_bytes(buf.get(0..2)?.try_into().ok()?);
            channels = u16::from_le_bytes(buf.get(2..4)?.try_into().ok()?);
            sample_rate = u32::from_le_bytes(buf.get(4..8)?.try_into().ok()?);
            block_align = u16::from_le_bytes(buf.get(12..14)?.try_into().ok()?);
            bits_per_sample = u16::from_le_bytes(buf.get(14..16)?.try_into().ok()?);

            if audio_format != 1 && !(audio_format == 3 && bits_per_sample == 32) {
                return None;
            }
            if channels == 0 {
                return None;
            }
            let bytes_per_sample = u16::try_from(bits_per_sample.checked_div(8)?).ok()?;
            if channels.checked_mul(bytes_per_sample)? != block_align {
                return None;
            }
            if !(bits_per_sample == 8
                || bits_per_sample == 16
                || bits_per_sample == 24
                || bits_per_sample == 32)
            {
                return None;
            }
            have_fmt = true;
            let pad = i64::from(size & 1);
            if pad != 0 {
                f.seek(SeekFrom::Current(pad)).ok()?;
            }
        } else if id == b"data" {
            data_offset = f.stream_position().ok()?;
            data_len = u64::from(size);
            let skip = i64::try_from(size).ok()?;
            f.seek(SeekFrom::Current(skip)).ok()?;
            have_data = true;
            let pad = i64::from(size & 1);
            if pad != 0 {
                f.seek(SeekFrom::Current(pad)).ok()?;
            }
        } else {
            let pad = i64::from(size & 1);
            f.seek(SeekFrom::Current(i64::from(size) + pad)).ok()?;
        }

        if have_fmt && have_data {
            break;
        }
    }

    if !have_fmt || !have_data {
        return None;
    }

    let baf = usize::from(block_align);
    if baf == 0 {
        return None;
    }

    f.seek(SeekFrom::Start(data_offset)).ok()?;
    let n_frames = data_len / u64::from(block_align);
    let bytes_to_read = n_frames.checked_mul(u64::from(block_align))?;

    Some((
        WavPcmLayout {
            sample_rate,
            channels,
            block_align,
            audio_format,
            bits_per_sample,
            data_total_bytes: bytes_to_read,
        },
        f,
    ))
}

/// Linear PCM (`format` 1), 8/16/24/32-bit, or IEEE float (`format` 3) 32-bit. Returns `None` to fall back to Symphonia.
pub(super) fn try_decode_wav_linear_pcm(path: &Path) -> Option<(Vec<f32>, u32, u16)> {
    let (layout, mut f) = open_wav_pcm_source(path)?;
    let baf = usize::from(layout.block_align);
    let total_samples = usize::try_from(
        (layout.data_total_bytes / u64::from(layout.block_align)).checked_mul(u64::from(layout.channels))?,
    )
    .ok()?;

    let mut samples = Vec::new();
    samples.try_reserve_exact(total_samples).ok()?;
    let mut remaining = usize::try_from(layout.data_total_bytes).ok()?;
    let mut read_buf = vec![0u8; READ_CHUNK.min(remaining.max(baf))];

    decode_wav_body(
        layout.audio_format,
        layout.bits_per_sample,
        baf,
        &mut f,
        &mut read_buf,
        &mut remaining,
        &mut samples,
    )
    .ok()?;

    if samples.len() != total_samples {
        return None;
    }

    Some((samples, layout.sample_rate, layout.channels))
}

/// Decode one aligned `data` slice (full frames only) into interleaved `f32`.
pub(super) fn decode_wav_bytes_to_f32(
    audio_format: u16,
    bits_per_sample: u16,
    baf: usize,
    bytes: &[u8],
    out: &mut Vec<f32>,
) -> Result<(), ()> {
    if baf == 0 || bytes.len() % baf != 0 {
        return Err(());
    }
    match (audio_format, bits_per_sample) {
        (1, 8) => {
            for chunk in bytes.chunks_exact(baf) {
                for &b in chunk {
                    out.push((f32::from(b) - 128.0) / 128.0);
                }
            }
        }
        (1, 16) => {
            for chunk in bytes.chunks_exact(baf) {
                for w in chunk.chunks_exact(2) {
                    let v = i16::from_le_bytes([w[0], w[1]]);
                    out.push(f32::from(v) * (1.0 / 32768.0));
                }
            }
        }
        (1, 24) => {
            for chunk in bytes.chunks_exact(baf) {
                for w in chunk.chunks_exact(3) {
                    let lo = u32::from(w[0]);
                    let mid = u32::from(w[1]);
                    let hi = u32::from(w[2]);
                    let x = lo | (mid << 8) | (hi << 16);
                    let v = (x << 8) as i32 >> 8;
                    out.push(v as f32 * (1.0 / 8_388_608.0));
                }
            }
        }
        (1, 32) => {
            for chunk in bytes.chunks_exact(baf) {
                for w in chunk.chunks_exact(4) {
                    let v = i32::from_le_bytes([w[0], w[1], w[2], w[3]]);
                    out.push(v as f32 * (1.0 / 2_147_483_648.0));
                }
            }
        }
        (3, 32) => {
            for chunk in bytes.chunks_exact(baf) {
                for w in chunk.chunks_exact(4) {
                    out.push(f32::from_le_bytes([w[0], w[1], w[2], w[3]]));
                }
            }
        }
        _ => return Err(()),
    }
    Ok(())
}

/// Decode from current file position; `remaining` is bytes left in `data`.
pub(super) fn decode_wav_body(
    audio_format: u16,
    bits_per_sample: u16,
    baf: usize,
    f: &mut File,
    read_buf: &mut [u8],
    remaining: &mut usize,
    samples: &mut Vec<f32>,
) -> std::io::Result<()> {
    while *remaining > 0 {
        let n_raw = (*remaining).min(read_buf.len());
        let n = (n_raw / baf) * baf;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "wav chunk align",
            ));
        }
        f.read_exact(&mut read_buf[..n])?;
        decode_wav_bytes_to_f32(audio_format, bits_per_sample, baf, &read_buf[..n], samples).map_err(
            |_| std::io::Error::new(std::io::ErrorKind::InvalidData, "wav decode"),
        )?;
        *remaining -= n;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::write_test_wav;
    use tempfile::TempDir;

    #[test]
    fn open_source_matches_full_decode() {
        let dir = TempDir::new().expect("d");
        let p = dir.path().join("x.wav");
        write_test_wav(&p, 44_100, 2, 16, 5000, 200.0).expect("w");
        let full = try_decode_wav_linear_pcm(&p).expect("full");
        let (layout, mut f) = open_wav_pcm_source(&p).expect("open");
        let mut buf = vec![0u8; 4096];
        let mut rem = usize::try_from(layout.data_total_bytes).unwrap();
        let mut part = Vec::new();
        decode_wav_body(
            layout.audio_format,
            layout.bits_per_sample,
            usize::from(layout.block_align),
            &mut f,
            &mut buf,
            &mut rem,
            &mut part,
        )
        .expect("body");
        assert_eq!(part, full.0);
    }
}
