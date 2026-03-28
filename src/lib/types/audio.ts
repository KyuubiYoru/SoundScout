export interface PostProcessConfig {
  trimSilence: boolean;
  /** dBFS vs short-window peak; below this counts as silence. */
  trimThresholdDb: number;
  /** Minimum leading/trailing silence (ms) before that edge is trimmed. */
  trimMinSilenceMs: number;
  /** Sliding max window width (ms) for measuring level; larger = smoother, smaller = sharper. */
  trimPeakWindowMs: number;
  normalizePeak: boolean;
  /** Linear peak amplitude after peak normalize (0–1); presets use `peakDbfsToLinear` (e.g. −6 dBFS ≈ 0.501). */
  normalizeTarget: number;
  makeLoopable: boolean;
  crossfadeSec: number | null;
  embedSmplChunk: boolean;
}

/** Named starting points for trim silence; users can still tweak numbers. */
export const trimSilencePresets = {
  balanced: { trimThresholdDb: -60, trimMinSilenceMs: 15, trimPeakWindowMs: 4 },
  tight: { trimThresholdDb: -48, trimMinSilenceMs: 5, trimPeakWindowMs: 3 },
  loose: { trimThresholdDb: -72, trimMinSilenceMs: 40, trimPeakWindowMs: 6 },
} as const;

export type TrimSilencePresetId = keyof typeof trimSilencePresets;

export function trimSilencePresetMatch(c: PostProcessConfig): TrimSilencePresetId | "custom" {
  for (const id of Object.keys(trimSilencePresets) as TrimSilencePresetId[]) {
    const v = trimSilencePresets[id];
    if (
      Math.abs(c.trimThresholdDb - v.trimThresholdDb) < 0.01 &&
      Math.abs(c.trimMinSilenceMs - v.trimMinSilenceMs) < 0.01 &&
      Math.abs(c.trimPeakWindowMs - v.trimPeakWindowMs) < 0.01
    ) {
      return id;
    }
  }
  return "custom";
}

/** Linear peak for a given dBFS below full scale (0 dBFS = 1.0 linear). */
export function peakDbfsToLinear(dbfs: number): number {
  return Math.pow(10, dbfs / 20);
}

/** dBFS of a linear peak in (0, 1], for labels. */
export function linearPeakToDbfs(linear: number): number {
  if (linear <= 0 || !Number.isFinite(linear)) return -96;
  return 20 * Math.log10(linear);
}

/** Preset peak levels (dBFS). Default export target is −6 dBFS (headroom for layered SFX). */
export const NORMALIZE_PEAK_DBFS_OPTIONS = [-12, -6, -3, -1, 0] as const;

/** Nearest preset dBFS for a stored linear peak (for select display). */
export function closestNormalizePeakDbfs(
  linear: number,
): (typeof NORMALIZE_PEAK_DBFS_OPTIONS)[number] {
  let best = NORMALIZE_PEAK_DBFS_OPTIONS[0];
  let bestDist = Infinity;
  for (const db of NORMALIZE_PEAK_DBFS_OPTIONS) {
    const d = Math.abs(linear - peakDbfsToLinear(db));
    if (d < bestDist) {
      bestDist = d;
      best = db;
    }
  }
  return best;
}

export const defaultPostProcessConfig: PostProcessConfig = {
  trimSilence: false,
  trimThresholdDb: -60,
  trimMinSilenceMs: 15,
  trimPeakWindowMs: 4,
  normalizePeak: false,
  normalizeTarget: peakDbfsToLinear(-6),
  makeLoopable: false,
  crossfadeSec: null,
  embedSmplChunk: true,
};
