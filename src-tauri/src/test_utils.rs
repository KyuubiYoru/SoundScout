//! Shared test helpers. Only compiled under `#[cfg(test)]`.

use std::io::Write;
use std::path::Path;

fn write_u32_le(w: &mut impl Write, v: u32) -> std::io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_u16_le(w: &mut impl Write, v: u16) -> std::io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

/// Writes a minimal valid WAV file for testing (RIFF PCM).
/// For stereo, duplicates mono signal to both channels.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn write_test_wav(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    num_samples: u32,
    frequency_hz: f32,
) -> std::io::Result<()> {
    assert!(channels == 1 || channels == 2, "only mono/stereo");
    assert!(bits_per_sample == 16 || bits_per_sample == 24, "only 16/24 bit");

    let block_align = (channels * bits_per_sample) / 8;
    let byte_rate = sample_rate * u32::from(block_align);
    let frames = num_samples;
    let data_bytes = usize::from(block_align) * frames as usize;

    let mut data = Vec::with_capacity(data_bytes);
    let two_pi = std::f32::consts::TAU;
    for i in 0..frames {
        let t = f64::from(i) / f64::from(sample_rate);
        let sample = if frequency_hz <= 0.0 {
            0.0f32
        } else {
            (f64::from(frequency_hz) * t * two_pi as f64).sin() as f32
        };
        if bits_per_sample == 16 {
            let s = (sample * i16::MAX as f32) as i16;
            for _ in 0..channels {
                data.extend_from_slice(&s.to_le_bytes());
            }
        } else {
            let s = (sample * 8_388_607.0) as i32; // 24-bit in i32
            let b0 = (s & 0xFF) as u8;
            let b1 = ((s >> 8) & 0xFF) as u8;
            let b2 = ((s >> 16) & 0xFF) as u8;
            for _ in 0..channels {
                data.extend_from_slice(&[b0, b1, b2]);
            }
        }
    }

    let chunk_size = 36 + data.len();
    let mut file = std::fs::File::create(path)?;
    file.write_all(b"RIFF")?;
    write_u32_le(&mut file, chunk_size as u32)?;
    file.write_all(b"WAVEfmt ")?;
    write_u32_le(&mut file, 16)?; // PCM fmt chunk size
    write_u16_le(&mut file, 1)?; // audio format PCM
    write_u16_le(&mut file, channels)?;
    write_u32_le(&mut file, sample_rate)?;
    write_u32_le(&mut file, byte_rate)?;
    write_u16_le(&mut file, block_align)?;
    write_u16_le(&mut file, bits_per_sample)?;
    file.write_all(b"data")?;
    write_u32_le(&mut file, data.len() as u32)?;
    file.write_all(&data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn generates_valid_wav_file() {
        let dir = TempDir::new().expect("tempdir");
        let p = dir.path().join("t.wav");
        write_test_wav(&p, 44_100, 1, 16, 100, 440.0).expect("write");
        let meta = fs::metadata(&p).expect("meta");
        assert!(meta.len() > 44);
    }

    #[test]
    fn generates_silence_when_frequency_zero() {
        let dir = TempDir::new().expect("tempdir");
        let p = dir.path().join("s.wav");
        write_test_wav(&p, 8_000, 1, 16, 10, 0.0).expect("write");
        let bytes = fs::read(&p).expect("read");
        // data region should be mostly zeros after header
        assert!(bytes.iter().skip(44).all(|&b| b == 0));
    }

    #[test]
    fn generates_stereo_file() {
        let dir = TempDir::new().expect("tempdir");
        let p = dir.path().join("st.wav");
        write_test_wav(&p, 44_100, 2, 16, 50, 1000.0).expect("write");
        assert!(fs::metadata(&p).expect("meta").len() > 44);
    }
}
