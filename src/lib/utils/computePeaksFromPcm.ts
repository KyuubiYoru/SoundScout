/**
 * Min/max peak pairs per bucket, matching `src-tauri/src/indexer/peaks.rs` (`compute_peaks`).
 * Interleaved f32 LE PCM → mono → `resolution` buckets.
 */
export function computePeaksFromInterleavedPcmF32(
  ab: ArrayBuffer,
  channels: number,
  resolution: number,
): number[] {
  if (resolution <= 0) return [];
  const ch = Math.max(1, channels | 0);
  const samples = new Float32Array(ab);
  const totalFrames = Math.floor(samples.length / ch);
  if (totalFrames === 0) return [];

  let mono: Float32Array;
  if (ch === 1) {
    mono = samples.subarray(0, totalFrames);
  } else {
    mono = new Float32Array(totalFrames);
    for (let i = 0; i < totalFrames; i++) {
      let sum = 0;
      for (let c = 0; c < ch; c++) sum += samples[i * ch + c];
      mono[i] = sum / ch;
    }
  }

  const n = mono.length;
  const chunk = Math.max(1, Math.floor(n / resolution));
  const out: number[] = [];
  for (let i = 0; i < resolution; i++) {
    const start = i * chunk;
    const end = i + 1 === resolution ? n : Math.min((i + 1) * chunk, n);
    let mn = 0;
    let mx = 0;
    if (start >= n) {
      mn = 0;
      mx = 0;
    } else {
      mn = mono[start];
      mx = mono[start];
      for (let j = start + 1; j < end; j++) {
        const s = mono[j];
        if (s < mn) mn = s;
        if (s > mx) mx = s;
      }
      mn = Math.max(-1, Math.min(1, mn));
      mx = Math.max(-1, Math.min(1, mx));
    }
    out.push(mn, mx);
  }
  return out;
}
