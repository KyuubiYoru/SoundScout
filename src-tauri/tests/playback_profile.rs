//! Integration-style checks + optional profiling of a real library file.
//!
//! Default (CI): only checks that the profiling API runs on a temp file.
//!
//! Profile your machine (with the GDC WAV or any path):
//! ```text
//! SOUNDSCOUT_WAV="/path/to/file.wav" cargo test -p tauri-app --test playback_profile profile_real_file_if_env_set -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use tauri_app_lib::audio::profile::decode_path_report;

#[test]
fn profile_api_runs_on_temp_wav() {
    let dir = tempfile::tempdir().expect("tmp");
    let p = dir.path().join("x.wav");
    write_minimal_silence_wav(&p, 8000, 1, 800).expect("write");
    let r = decode_path_report(&p).expect("profile");
    assert!(r.decode_wall_ms < 10_000);
    assert_eq!(r.sample_rate, 8000);
    assert!(r.ipc_u8_bytes > 0);
}

/// Run with: `SOUNDSCOUT_WAV=... cargo test ... -- --ignored --nocapture`
#[test]
#[ignore = "slow / optional: set SOUNDSCOUT_WAV or rely on workspace sample path"]
fn profile_real_file_if_env_set() {
    let path = wav_path_for_manual_profile();
    if !path.exists() {
        eprintln!(
            "skip: file not found: {}\n(set SOUNDSCOUT_WAV or add the WAV under Test Audio Files/...)",
            path.display()
        );
        return;
    }
    let r = decode_path_report(&path).unwrap_or_else(|e| panic!("profile {}: {e}", path.display()));

    eprintln!("{}", serde_json::to_string_pretty(&r).expect("json"));

    assert!(
        r.decode_wall_ms < 120_000,
        "decode_wall_ms={} — if this is huge, PCM fallback explains slow playback; \
         if small but UI is slow, profile WebView/asset URL (not covered here)",
        r.decode_wall_ms
    );
}

fn wav_path_for_manual_profile() -> PathBuf {
    if let Ok(p) = std::env::var("SOUNDSCOUT_WAV") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../Test Audio Files/Sonniss.com - GDC 2019 - Game Audio Bundle Part 1of8/344 Audio - Low Frequency Elements/Alien Spaceship Filtered, Rumble.wav",
    )
}

fn write_minimal_silence_wav(
    path: &std::path::Path,
    sample_rate: u32,
    channels: u16,
    frames: u32,
) -> std::io::Result<()> {
    use std::io::Write;

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
    file.write_all(&1u16.to_le_bytes())?;
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
