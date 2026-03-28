//! STFT-ish features: magnitude spectrum, MFCC-like vectors, cosine similarity.

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(dead_code)] // `chroma_at` reserved for music-style loop hints

use realfft::RealFftPlanner;

pub fn hanning(n: usize) -> Vec<f32> {
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .map(|i| {
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (n - 1).max(1) as f32).cos())
        })
        .collect()
}

/// Single-sided magnitude spectrum (first n/2+1 bins) at `center` in `samples`.
pub fn magnitude_spectrum(samples: &[f32], center: usize, fft_size: usize) -> Vec<f32> {
    let n = fft_size.next_power_of_two();
    let half = n / 2 + 1;
    let mut planner = RealFftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(n);
    let win = hanning(n);
    let mut buf = vec![0.0f32; n];
    let c = center.min(samples.len().saturating_sub(1));
    let start = c.saturating_sub(n / 2);
    for i in 0..n {
        let si = start + i;
        let s = samples.get(si).copied().unwrap_or(0.0);
        buf[i] = s * win.get(i).copied().unwrap_or(0.0);
    }
    let mut spec = fwd.make_output_vec();
    let mut scratch = fwd.make_scratch_vec();
    fwd.process_with_scratch(&mut buf, &mut spec, &mut scratch)
        .expect("fft forward");
    spec.iter()
        .take(half)
        .map(|c| (c.re * c.re + c.im * c.im).sqrt())
        .collect()
}

/// 13 log-spaced band energies (MFCC-ish) from magnitude spectrum.
pub fn mfcc_like(mag: &[f32], _sr: u32) -> [f32; 13] {
    let mut out = [0.0f32; 13];
    if mag.is_empty() {
        return out;
    }
    let bands = 13usize;
    let m = mag.len();
    for b in 0..bands {
        let i0 = (b * m) / bands;
        let i1 = ((b + 1) * m) / bands;
        let mut e = 0.0f32;
        for i in i0..i1.max(i0 + 1) {
            e += mag.get(i).copied().unwrap_or(0.0);
        }
        let n = (i1 - i0).max(1) as f32;
        out[b] = (e / n + 1e-10).ln();
    }
    out
}

pub fn mfcc_at(samples: &[f32], center: usize, fft_size: usize, sr: u32) -> [f32; 13] {
    let mag = magnitude_spectrum(samples, center, fft_size);
    mfcc_like(&mag, sr)
}

/// Rough chroma from magnitude spectrum (12 bins).
pub fn chroma_at(samples: &[f32], center: usize, fft_size: usize, sr: u32) -> [f32; 12] {
    let mag = magnitude_spectrum(samples, center, fft_size);
    let mut c = [0.0f32; 12];
    let n = mag.len();
    if n < 2 {
        return c;
    }
    for (i, &m) in mag.iter().enumerate().skip(1) {
        let f = i as f32 * sr as f32 / fft_size as f32;
        if f < 20.0 {
            continue;
        }
        let pc = (12.0 * (f.log2() * 12.0 / 12.0 + 1000.0).rem_euclid(12.0)) as usize % 12;
        c[pc] += m;
    }
    c
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let d = (na.sqrt() * nb.sqrt()).max(f32::EPSILON);
    (dot / d).clamp(-1.0, 1.0)
}
