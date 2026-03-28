/** Minimum clip length (seconds) for Shift-drag selection; aligned with Rust `clip_wav::CLIP_MIN_SEC`. */
export const CLIP_MIN_SEC = 0.05;

/**
 * Clamp a clip (in seconds on the asset timeline) to [0, dur].
 * Returns null if the range collapses below {@link CLIP_MIN_SEC}.
 */
export function clampClipToDuration(
  clip: { start: number; end: number },
  dur: number,
): { start: number; end: number } | null {
  if (dur <= 0) return null;
  const end = Math.min(clip.end, dur);
  const start = Math.min(clip.start, end - CLIP_MIN_SEC);
  if (end - start < CLIP_MIN_SEC) return null;
  return { start: Math.max(0, start), end };
}

/**
 * Skip {@link clampClipToDuration} when the engine still holds a short buffer (export preview PCM,
 * or decode not finished) but `clip` is expressed in full-file seconds. Clamping with preview
 * duration would corrupt the stored clip (e.g. 10–20s → 7.95–8s).
 */
export function shouldDeferClipClampForHybrid(
  clip: { start: number; end: number },
  hybridDurSec: number,
  assetFullDurationSec: number,
): boolean {
  if (assetFullDurationSec <= 0 || hybridDurSec <= 0) return false;
  if (hybridDurSec >= assetFullDurationSec - 0.05) return false;
  return clip.end > hybridDurSec + 1e-3;
}
