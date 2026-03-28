export interface PostProcessConfig {
  trimSilence: boolean;
  trimThresholdDb: number;
  trimMinSilenceMs: number;
  normalizePeak: boolean;
  normalizeTarget: number;
  makeLoopable: boolean;
  crossfadeSec: number | null;
  embedSmplChunk: boolean;
}

export const defaultPostProcessConfig: PostProcessConfig = {
  trimSilence: false,
  trimThresholdDb: -60,
  trimMinSilenceMs: 50,
  normalizePeak: false,
  normalizeTarget: 0.97,
  makeLoopable: false,
  crossfadeSec: null,
  embedSmplChunk: true,
};
