//! Split-swap-crossfade (ambient / bulletproof loop).

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

use crate::audio::loop_proc::crossfade;
use crate::audio::loop_proc::{LoopResult, LoopTechnique};

/// `samples` is mono (one channel interleaved length = frames for ch=1).
pub fn split_swap_crossfade(
    interleaved: &[f32],
    sample_rate: u32,
    channels: u16,
    crossfade_sec: f32,
) -> Result<LoopResult, String> {
    let ch = usize::from(channels);
    if ch == 0 || interleaved.len() % ch != 0 {
        return Err("invalid buffer".to_string());
    }
    let frames = interleaved.len() / ch;
    if frames < 4 {
        return Err("audio too short for split-swap".to_string());
    }
    let xf = ((crossfade_sec.max(0.05) * sample_rate as f32).round() as usize).clamp(2, frames / 2 - 1);
    let mid = frames / 2;

    let (fade_in, fade_out) = crossfade::equal_power(xf);
    // a = first half frames [0..mid), b = [mid..frames)
    let mut a = Vec::with_capacity(mid * ch);
    let mut b = Vec::with_capacity((frames - mid) * ch);
    for f in 0..mid {
        for c in 0..ch {
            a.push(interleaved[f * ch + c]);
        }
    }
    for f in mid..frames {
        for c in 0..ch {
            b.push(interleaved[f * ch + c]);
        }
    }
    let b_frames = b.len() / ch;
    let a_frames = a.len() / ch;
    if b_frames < xf || a_frames < xf {
        return Err("segments too short for crossfade".to_string());
    }

    // blended = b[last xf] fade_out + a[first xf] fade_in
    let mut blended = Vec::with_capacity(xf * ch);
    for fi in 0..xf {
        for c in 0..ch {
            let bi = (b_frames - xf + fi) * ch + c;
            let ai = fi * ch + c;
            let v = b[bi] * fade_out[fi] + a[ai] * fade_in[fi];
            blended.push(v);
        }
    }

    // result = b[0 .. b_frames-xf] + blended + a[xf .. a_frames]
    let mut out = Vec::with_capacity((b_frames - xf + xf + a_frames - xf) * ch);
    for f in 0..b_frames - xf {
        for c in 0..ch {
            out.push(b[f * ch + c]);
        }
    }
    out.extend_from_slice(&blended);
    for f in xf..a_frames {
        for c in 0..ch {
            out.push(a[f * ch + c]);
        }
    }

    let out_frames = out.len() / ch;
    let loop_end = out_frames.saturating_sub(1);
    Ok(LoopResult {
        samples: out,
        loop_start: 0,
        loop_end,
        technique: LoopTechnique::SplitSwapCrossfade {
            crossfade_samples: xf * ch,
        },
    })
}
