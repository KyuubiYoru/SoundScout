//! Decode / IPC payload profiling for diagnosing slow playback (Symphonia path).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Instant;

use serde::Serialize;

use super::decoder::decode_to_pcm;

/// High-level RIFF/WAVE facts from the file header (no full decode).
#[derive(Debug, Clone, Serialize)]
pub struct WavHeaderInfo {
    pub riff_chunk_size: u32,
    pub audio_format: u16,
    pub num_channels: u16,
    pub sample_rate: u32,
    pub byte_rate: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub data_chunk_size: u64,
}

/// Wall-clock breakdown for the PCM fallback pipeline (matches `get_audio_data` work after cache miss).
#[derive(Debug, Clone, Serialize)]
pub struct DecodeProfileReport {
    pub path_display: String,
    pub file_size_bytes: u64,
    pub wav_header: Option<WavHeaderInfo>,
    pub sniff_error: Option<String>,
    pub decode_wall_ms: u128,
    pub sample_rate: u32,
    pub channels: u16,
    pub pcm_f32_samples: usize,
    pub duration_seconds: f64,
    pub ipc_u8_bytes: usize,
    pub f32_to_bytes_vec_ms: u128,
}

fn read_u16_le(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}

fn read_u32_le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Best-effort `fmt` + `data` chunk read (standard RIFF WAVE; not RF64).
pub fn sniff_wav_header(path: &Path) -> Result<WavHeaderInfo, String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut id = [0u8; 4];
    let mut size_buf = [0u8; 4];
    f.read_exact(&mut id).map_err(|e| e.to_string())?;
    if id != *b"RIFF" {
        return Err("not RIFF".into());
    }
    f.read_exact(&mut size_buf).map_err(|e| e.to_string())?;
    let riff_chunk_size = read_u32_le(&size_buf);
    f.read_exact(&mut id).map_err(|e| e.to_string())?;
    if id != *b"WAVE" {
        return Err("not WAVE".into());
    }

    let mut fmt: Option<WavHeaderInfo> = None;
    let mut data_size: u64 = 0;
    let mut found_data = false;

    loop {
        match f.read_exact(&mut id) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.to_string()),
        }
        f.read_exact(&mut size_buf).map_err(|e| e.to_string())?;
        let chunk_size = u64::from(read_u32_le(&size_buf));
        let pad = chunk_size % 2;

        if id == *b"fmt " {
            if chunk_size < 16 {
                return Err("fmt chunk too small".into());
            }
            let mut buf = vec![0u8; chunk_size as usize];
            f.read_exact(&mut buf).map_err(|e| e.to_string())?;
            let audio_format = read_u16_le(&buf[0..2]);
            let num_channels = read_u16_le(&buf[2..4]);
            let sample_rate = read_u32_le(&buf[4..8]);
            let byte_rate = read_u32_le(&buf[8..12]);
            let block_align = read_u16_le(&buf[12..14]);
            let bits_per_sample = read_u16_le(&buf[14..16]);
            fmt = Some(WavHeaderInfo {
                riff_chunk_size,
                audio_format,
                num_channels,
                sample_rate,
                byte_rate,
                block_align,
                bits_per_sample,
                data_chunk_size: 0,
            });
        } else if id == *b"data" {
            data_size = chunk_size;
            found_data = true;
            f.seek(SeekFrom::Current(chunk_size as i64))
                .map_err(|e| e.to_string())?;
        } else {
            f.seek(SeekFrom::Current(chunk_size as i64))
                .map_err(|e| e.to_string())?;
        }
        if pad == 1 {
            f.seek(SeekFrom::Current(1)).map_err(|e| e.to_string())?;
        }
        if fmt.is_some() && found_data {
            break;
        }
    }

    let mut w = fmt.ok_or_else(|| "no fmt chunk".to_string())?;
    w.data_chunk_size = data_size;
    Ok(w)
}

/// Full Symphonia decode + same `f32` → `Vec<u8>` work as `get_audio_data` (excluding DB).
pub fn decode_path_report(path: &Path) -> Result<DecodeProfileReport, String> {
    let path_display = path.display().to_string();
    let file_size_bytes = std::fs::metadata(path).map_err(|e| e.to_string())?.len();

    let (wav_header, sniff_error) = match sniff_wav_header(path) {
        Ok(h) => (Some(h), None),
        Err(e) => (None, Some(e)),
    };

    let t0 = Instant::now();
    let pcm = decode_to_pcm(path).map_err(|e| e.to_string())?;
    let decode_wall_ms = t0.elapsed().as_millis();

    let sample_rate = pcm.sample_rate;
    let channels = pcm.channels;
    let pcm_f32_samples = pcm.samples.len();
    let ch = usize::from(channels.max(1));
    let frames = pcm_f32_samples.checked_div(ch).unwrap_or(0);
    let duration_seconds = (frames as f64) / f64::from(sample_rate);

    let slice = bytemuck::cast_slice::<f32, u8>(pcm.samples.as_slice());
    let ipc_u8_bytes = slice.len();
    let t1 = Instant::now();
    let _ipc_copy = slice.to_vec();
    let f32_to_bytes_vec_ms = t1.elapsed().as_millis();

    Ok(DecodeProfileReport {
        path_display,
        file_size_bytes,
        wav_header,
        sniff_error,
        decode_wall_ms,
        sample_rate,
        channels,
        pcm_f32_samples,
        duration_seconds,
        ipc_u8_bytes,
        f32_to_bytes_vec_ms,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::Path;

    use tempfile::TempDir;

    use super::{decode_path_report, sniff_wav_header};

    fn write_silence_wav(path: &Path, sample_rate: u32, channels: u16, frames: u32) -> std::io::Result<()> {
        let bits: u16 = 16;
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * u32::from(block_align);
        let data_bytes = usize::from(block_align) * frames as usize;
        let chunk_size = 36 + data_bytes;

        let mut file = std::fs::File::create(path)?;
        file.write_all(b"RIFF")?;
        file.write_all(&(chunk_size as u32).to_le_bytes())?;
        file.write_all(b"WAVEfmt ")?;
        file.write_all(&16u32.to_le_bytes())?;
        file.write_all(&1u16.to_le_bytes())?; // PCM
        file.write_all(&channels.to_le_bytes())?;
        file.write_all(&sample_rate.to_le_bytes())?;
        file.write_all(&byte_rate.to_le_bytes())?;
        file.write_all(&block_align.to_le_bytes())?;
        file.write_all(&bits.to_le_bytes())?;
        file.write_all(b"data")?;
        file.write_all(&(data_bytes as u32).to_le_bytes())?;
        file.write_all(&vec![0u8; data_bytes])?;
        Ok(())
    }

    #[test]
    fn sniff_matches_decode_metadata() {
        let dir = TempDir::new().expect("tmp");
        let p = dir.path().join("t.wav");
        write_silence_wav(&p, 48_000, 2, 100).expect("w");
        let h = sniff_wav_header(&p).expect("sniff");
        assert_eq!(h.sample_rate, 48_000);
        assert_eq!(h.num_channels, 2);
        assert_eq!(h.bits_per_sample, 16);
        let r = decode_path_report(&p).expect("dec");
        assert_eq!(r.sample_rate, 48_000);
        assert_eq!(r.channels, 2);
        assert!((r.duration_seconds - 100.0 / 48_000.0).abs() < 0.001);
    }

    #[test]
    fn decode_profile_short_file_is_fast() {
        let dir = TempDir::new().expect("tmp");
        let p = dir.path().join("s.wav");
        write_silence_wav(&p, 44_100, 1, 44_100).expect("w");
        let r = decode_path_report(&p).expect("dec");
        assert!(
            r.decode_wall_ms < 5_000,
            "1s mono decode took {}ms (unexpectedly slow)",
            r.decode_wall_ms
        );
    }
}
