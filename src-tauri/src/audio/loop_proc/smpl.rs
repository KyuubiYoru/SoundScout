//! WAV `smpl` chunk writer (one forward loop).

use std::io::Write;

/// Append smpl chunk after caller has written fmt+data (same file, update RIFF size separately in clip_wav).
///
/// `loop_start` / `loop_end` are **sample-frame indices** (inclusive). The WAV `smpl` spec
/// measures positions in *sample frames* (time points), not in interleaved PCM byte offsets, so a
/// stereo file with 1000 frames has valid loop indices 0–999 regardless of channel count.
pub fn write_smpl_chunk<W: Write>(
    w: &mut W,
    sample_rate: u32,
    loop_start: u32,
    loop_end: u32,
) -> Result<(), std::io::Error> {
    let period = 1_000_000_000u32 / sample_rate.max(1);
    w.write_all(b"smpl")?;
    w.write_all(&68u32.to_le_bytes())?;
    w.write_all(&0u32.to_le_bytes())?; // manufacturer
    w.write_all(&0u32.to_le_bytes())?; // product
    w.write_all(&period.to_le_bytes())?;
    w.write_all(&60u32.to_le_bytes())?; // midi unity
    w.write_all(&0u32.to_le_bytes())?; // pitch frac
    w.write_all(&0u32.to_le_bytes())?; // smpte format
    w.write_all(&0u32.to_le_bytes())?; // smpte offset
    w.write_all(&1u32.to_le_bytes())?; // num_loops
    w.write_all(&0u32.to_le_bytes())?; // sampler data len
    // loop entry
    w.write_all(&0u32.to_le_bytes())?; // cue id
    w.write_all(&0u32.to_le_bytes())?; // type forward
    w.write_all(&loop_start.to_le_bytes())?;
    w.write_all(&loop_end.to_le_bytes())?;
    w.write_all(&0u32.to_le_bytes())?; // fraction
    w.write_all(&0u32.to_le_bytes())?; // play count infinite
    Ok(())
}
