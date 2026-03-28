//! Seam blend using longer equal-power crossfade (quasi-periodic fallback).

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

use crate::audio::loop_proc::crossfade;

/// Apply a longer equal-power seam blend (approximates smoother spectral transition for quasi-periodic material).
pub fn blend_seam_equal_power_long(
    interleaved: &mut [f32],
    ch: usize,
    xf_frames: usize,
) -> Result<(), String> {
    let frames = interleaved.len() / ch;
    if xf_frames == 0 || frames < xf_frames * 2 + 1 {
        return Err("crossfade too long for buffer".to_string());
    }
    let (fade_in, fade_out) = crossfade::equal_power(xf_frames);
    for fi in 0..xf_frames {
        let g_in = fade_in[fi];
        let g_out = fade_out[fi];
        for c in 0..ch {
            let tail_i = (frames - xf_frames + fi) * ch + c;
            let head_i = fi * ch + c;
            let t = interleaved[tail_i];
            let h = interleaved[head_i];
            interleaved[head_i] = t * g_out + h * g_in;
        }
    }
    Ok(())
}
