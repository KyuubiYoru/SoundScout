import { convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writable, get } from "svelte/store";
import type { Asset, PostProcessConfig } from "$lib/types";
import * as ipc from "$lib/ipc";
import { HybridPlayback } from "$lib/utils/hybridPlayback";
import { computePeaksFromInterleavedPcmF32 } from "$lib/utils/computePeaksFromPcm";
import { toastStore } from "./toastStore";
import { settingsStore } from "./settingsStore";
import {
  CLIP_MIN_SEC,
  clampClipToDuration,
  shouldDeferClipClampForHybrid,
} from "$lib/utils/clipBounds";
import { postProcessStore } from "./postProcessStore";

const hybrid = new HybridPlayback();

/** Re-export for callers that imported from `playerStore`. */
export { CLIP_MIN_SEC } from "$lib/utils/clipBounds";

/** Prefer chunked decode + gapless Web Audio when the asset is large or long. */
const STREAM_PCM_MIN_DURATION_MS = 90_000;
const STREAM_PCM_MIN_BYTES = 12 * 1024 * 1024;

function shouldUseChunkedPcm(asset: Asset): boolean {
  return (
    (asset.durationMs != null && asset.durationMs >= STREAM_PCM_MIN_DURATION_MS) ||
    asset.fileSize >= STREAM_PCM_MIN_BYTES
  );
}

let streamUnlistenChunk: UnlistenFn | undefined;
let streamUnlistenDone: UnlistenFn | undefined;

type PcmCacheEntry = { ab: ArrayBuffer; sr: number; ch: number };
const pcmDecodeCache = new Map<number, PcmCacheEntry>();
const pcmDecodeOrder: number[] = [];

function trimPcmDecodeCache() {
  const cap = Math.max(1, get(settingsStore).playback.buffer_cache_count);
  while (pcmDecodeOrder.length > cap) {
    const id = pcmDecodeOrder.shift();
    if (id != null) pcmDecodeCache.delete(id);
  }
}

/** LRU: refresh slot on hit. */
function pcmDecodeCacheGet(assetId: number): PcmCacheEntry | undefined {
  const cached = pcmDecodeCache.get(assetId);
  if (!cached) return undefined;
  const i = pcmDecodeOrder.indexOf(assetId);
  if (i >= 0) pcmDecodeOrder.splice(i, 1);
  pcmDecodeOrder.push(assetId);
  return cached;
}

function pcmDecodeCachePut(assetId: number, entry: PcmCacheEntry) {
  if (pcmDecodeCache.has(assetId)) {
    const i = pcmDecodeOrder.indexOf(assetId);
    if (i >= 0) pcmDecodeOrder.splice(i, 1);
  }
  pcmDecodeCache.set(assetId, entry);
  pcmDecodeOrder.push(assetId);
  trimPcmDecodeCache();
}

async function loadFullPcmForAsset(asset: Asset): Promise<PcmCacheEntry> {
  const hit = pcmDecodeCacheGet(asset.id);
  if (hit) return hit;
  const pcmMeta = await ipc.getAudioPcmFile(asset.id);
  const res = await fetch(convertFileSrc(pcmMeta.path));
  if (!res.ok) {
    throw new Error(`pcm fetch ${res.status}`);
  }
  const ab = await res.arrayBuffer();
  const sr = pcmMeta.sampleRate ?? asset.sampleRate ?? 44100;
  const ch = pcmMeta.channels ?? asset.channels ?? 1;
  const entry = { ab, sr, ch };
  pcmDecodeCachePut(asset.id, entry);
  return entry;
}

/** Bump to ignore stale preview IPC/fetch results. */
let previewGeneration = 0;

async function teardownPcmStreamPlayback(): Promise<void> {
  await streamUnlistenChunk?.();
  await streamUnlistenDone?.();
  streamUnlistenChunk = undefined;
  streamUnlistenDone = undefined;
  await ipc.cancelPcmStream().catch(() => {});
}

const state = writable<{
  currentAsset: Asset | null;
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  peaks: number[];
  clipRange: { start: number; end: number } | null;
  volume: number;
  previewActive: boolean;
  previewLoading: boolean;
  /** Peaks derived from processed preview PCM so the waveform matches export output. */
  previewPeaksOverride: number[] | null;
}>({
  currentAsset: null,
  isPlaying: false,
  currentTime: 0,
  duration: 0,
  peaks: [],
  clipRange: null,
  volume: 1,
  previewActive: false,
  previewLoading: false,
  previewPeaksOverride: null,
});

function applyHybridVolume(): void {
  hybrid.setVolume(get(state).volume);
}

function clipNotchStepSec(): number {
  const ms = get(settingsStore).playback.clip_notch_ms ?? 100;
  return Math.max(10, Math.min(10_000, ms)) / 1000;
}

let raf: number | null = null;
/** True while `seekInternal` is running from clip enforcement (avoids re-entry; keeps enforcing while hybrid is briefly paused). */
let clipBoundsSeekInFlight = false;

function enforceClipBounds() {
  const snapshot = get(state);
  if (snapshot.previewActive) return;
  const clip = snapshot.clipRange;
  if (!clip) return;
  if (clipBoundsSeekInFlight) return;
  if (!hybrid.isPlaying) return;
  const loopOn = get(settingsStore).playback.loop_playback;
  const playheadTime = hybrid.currentTime;
  if (playheadTime < clip.start) {
    clipBoundsSeekInFlight = true;
    void seekInternal(clip.start).finally(() => {
      clipBoundsSeekInFlight = false;
    });
    return;
  }
  if (playheadTime >= clip.end - 0.0015) {
    if (loopOn) {
      clipBoundsSeekInFlight = true;
      void seekInternal(clip.start).finally(() => {
        clipBoundsSeekInFlight = false;
      });
    } else {
      hybrid.pause();
      if (raf) cancelAnimationFrame(raf);
      raf = null;
      state.update((prev) => ({ ...prev, isPlaying: false }));
      const endPos = Math.min(clip.end, hybrid.duration || clip.end);
      clipBoundsSeekInFlight = true;
      void seekInternal(endPos).finally(() => {
        clipBoundsSeekInFlight = false;
      });
    }
  }
}

function tick() {
  if (hybrid.isPlaying) {
    enforceClipBounds();
  }
  const snapshot = get(state);
  const dur =
    hybrid.duration > 0
      ? hybrid.duration
      : snapshot.currentAsset?.durationMs != null
        ? snapshot.currentAsset.durationMs / 1000
        : snapshot.duration;
  let nextClip: (typeof snapshot)["clipRange"] = snapshot.clipRange;
  if (nextClip && dur > 0 && !snapshot.previewActive) {
    const assetFullDur =
      snapshot.currentAsset?.durationMs != null && snapshot.currentAsset.durationMs > 0
        ? snapshot.currentAsset.durationMs / 1000
        : 0;
    const deferClamp =
      assetFullDur > 0 &&
      shouldDeferClipClampForHybrid(nextClip, dur, assetFullDur);
    if (!deferClamp) {
      const clamped = clampClipToDuration(nextClip, dur);
      if (clamped == null) {
        nextClip = null;
        syncHybridLoopFromSettings();
      } else if (clamped.start !== nextClip.start || clamped.end !== nextClip.end) {
        nextClip = clamped;
      }
    }
  }
  state.update((prev) => ({
    ...prev,
    clipRange: nextClip,
    currentTime: hybrid.currentTime,
    duration: dur,
    isPlaying: hybrid.isPlaying,
  }));
  if (hybrid.isPlaying) {
    raf = requestAnimationFrame(tick);
  } else {
    raf = null;
  }
}

function queuePeaks(assetId: number) {
  void ipc
    .getPeaks(assetId)
    .then((peaks) => {
      state.update((prev) =>
        prev.currentAsset?.id === assetId ? { ...prev, peaks } : prev,
      );
    })
    .catch(() => {});
}

function syncHybridLoopFromSettings(): void {
  hybrid.setLoopPolicy(get(settingsStore).playback.loop_playback, get(state).clipRange != null);
}

async function seekInternal(position: number): Promise<void> {
  const snapshot = get(state);
  const asset = snapshot.currentAsset;
  if (hybrid.mode === "pcm_stream" && asset) {
    const wasPlaying = snapshot.isPlaying;
    await teardownPcmStreamPlayback();
    hybrid.stop();
    try {
      const { ab, sr, ch } = await loadFullPcmForAsset(asset);
      hybrid.loadPcm(ab, sr, ch);
      applyHybridVolume();
      hybrid.seek(position);
      state.update((prev) => ({
        ...prev,
        currentTime: hybrid.currentTime,
        duration: hybrid.duration,
      }));
      syncHybridLoopFromSettings();
      if (wasPlaying) {
        await hybrid.play();
        tick();
        state.update((prev) => ({ ...prev, isPlaying: true }));
      }
    } catch (err) {
      toastStore.show(String(err), "error");
    }
    return;
  }
  hybrid.seek(position);
  state.update((prev) => ({ ...prev, currentTime: hybrid.currentTime }));
}

async function clampPlayheadIntoClipIfNeeded(): Promise<void> {
  const snapshot = get(state);
  if (snapshot.previewActive) return;
  const clip = snapshot.clipRange;
  if (!clip) return;
  const playheadTime = hybrid.currentTime;
  if (playheadTime < clip.start || playheadTime >= clip.end) {
    await seekInternal(clip.start);
  }
}

/** Args for export/copy WAV: clip range, or full track using known duration. */
export type AudioExportCopyArgs = { isClip: boolean; startSec: number; endSec: number };

/** Read current player state without subscribing (e.g. debounced preview refresh). */
export function getPlayerState(): {
  currentAsset: Asset | null;
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  peaks: number[];
  clipRange: { start: number; end: number } | null;
  volume: number;
  previewActive: boolean;
  previewLoading: boolean;
  previewPeaksOverride: number[] | null;
} {
  return get(state);
}

export function getAudioExportCopyArgs(): AudioExportCopyArgs | null {
  const snapshot = get(state);
  const asset = snapshot.currentAsset;
  if (!asset) return null;
  if (snapshot.clipRange) {
    return { isClip: true, startSec: snapshot.clipRange.start, endSec: snapshot.clipRange.end };
  }
  const dur =
    hybrid.duration > 0
      ? hybrid.duration
      : Number.isFinite(snapshot.duration) && snapshot.duration > 0
        ? snapshot.duration
        : asset.durationMs != null && asset.durationMs > 0
          ? asset.durationMs / 1000
          : 0;
  if (dur <= 0) return null;
  return { isClip: false, startSec: 0, endSec: dur };
}

export const playerStore = {
  subscribe: state.subscribe,
  syncLoopFromConfig: syncHybridLoopFromSettings,
  async setLoopPlaybackAndSave(enabled: boolean) {
    settingsStore.setLoopPlayback(enabled);
    syncHybridLoopFromSettings();
    await settingsStore.save();
  },
  async commitClipRange(start: number, end: number) {
    const snapshot = get(state);
    const durationSec =
      hybrid.duration > 0
        ? hybrid.duration
        : snapshot.currentAsset?.durationMs != null
          ? snapshot.currentAsset.durationMs / 1000
          : snapshot.duration;
    if (durationSec <= 0) return;
    const lo = Math.max(0, Math.min(start, end, durationSec));
    const hi = Math.max(lo + CLIP_MIN_SEC, Math.min(Math.max(start, end), durationSec));
    state.update((prev) => ({ ...prev, clipRange: { start: lo, end: hi } }));
    syncHybridLoopFromSettings();
    await seekInternal(lo);
  },
  async commitClipRangeAndRefreshPreview(
    start: number,
    end: number,
    config: PostProcessConfig,
  ): Promise<void> {
    const asset = get(state).currentAsset;
    if (!asset) return;
    const fullDur =
      asset.durationMs != null && asset.durationMs > 0 ? asset.durationMs / 1000 : 0;
    if (fullDur <= 0) return;
    const lo = Math.max(0, Math.min(start, end, fullDur));
    const hi = Math.max(lo + CLIP_MIN_SEC, Math.min(Math.max(start, end), fullDur));
    state.update((prev) => ({ ...prev, clipRange: { start: lo, end: hi } }));
    await playerStore.previewProcessed(config);
  },
  /** Move clip start earlier (`i` — notch left on the start edge). */
  async notchClipStartOut(): Promise<void> {
    const stepSec = clipNotchStepSec();
    const snap = get(state);
    const cr = snap.clipRange;
    if (!cr || !snap.currentAsset) return;
    const nextStart = Math.max(0, cr.start - stepSec);
    if (nextStart >= cr.end - CLIP_MIN_SEC) return;
    if (snap.previewActive) {
      await playerStore.commitClipRangeAndRefreshPreview(
        nextStart,
        cr.end,
        get(postProcessStore),
      );
    } else {
      await playerStore.commitClipRange(nextStart, cr.end);
    }
  },
  /** Move clip start later (`o` — notch right on the start edge). */
  async notchClipStartIn(): Promise<void> {
    const stepSec = clipNotchStepSec();
    const snap = get(state);
    const cr = snap.clipRange;
    if (!cr || !snap.currentAsset) return;
    const nextStart = Math.min(cr.start + stepSec, cr.end - CLIP_MIN_SEC);
    if (nextStart <= cr.start) return;
    if (snap.previewActive) {
      await playerStore.commitClipRangeAndRefreshPreview(
        nextStart,
        cr.end,
        get(postProcessStore),
      );
    } else {
      await playerStore.commitClipRange(nextStart, cr.end);
    }
  },
  /** Move clip end earlier (`Shift+i` — notch left on the end edge). */
  async notchClipEndIn(): Promise<void> {
    const stepSec = clipNotchStepSec();
    const snap = get(state);
    const cr = snap.clipRange;
    if (!cr || !snap.currentAsset) return;
    const nextEnd = Math.max(cr.start + CLIP_MIN_SEC, cr.end - stepSec);
    if (nextEnd >= cr.end) return;
    if (snap.previewActive) {
      await playerStore.commitClipRangeAndRefreshPreview(
        cr.start,
        nextEnd,
        get(postProcessStore),
      );
    } else {
      await playerStore.commitClipRange(cr.start, nextEnd);
    }
  },
  /** Move clip end later (`Shift+o` — notch right on the end edge). */
  async notchClipEndOut(): Promise<void> {
    const stepSec = clipNotchStepSec();
    const snap = get(state);
    const cr = snap.clipRange;
    if (!cr || !snap.currentAsset) return;
    const fullDur =
      snap.currentAsset.durationMs != null && snap.currentAsset.durationMs > 0
        ? snap.currentAsset.durationMs / 1000
        : 0;
    if (fullDur <= 0) return;
    const nextEnd = Math.min(fullDur, cr.end + stepSec);
    if (nextEnd <= cr.start + CLIP_MIN_SEC) return;
    if (snap.previewActive) {
      await playerStore.commitClipRangeAndRefreshPreview(
        cr.start,
        nextEnd,
        get(postProcessStore),
      );
    } else {
      await playerStore.commitClipRange(cr.start, nextEnd);
    }
  },
  clearClipRange() {
    state.update((prev) => ({ ...prev, clipRange: null }));
    syncHybridLoopFromSettings();
  },
  async playAsset(asset: Asset, opts?: { preserveClip?: boolean }) {
    try {
      await teardownPcmStreamPlayback();
      const preserveClip = opts?.preserveClip ?? false;
      state.update((prev) => ({
        ...prev,
        previewActive: false,
        previewLoading: false,
        previewPeaksOverride: null,
        clipRange: preserveClip ? prev.clipRange : null,
      }));
      syncHybridLoopFromSettings();

      if (HybridPlayback.canStream()) {
        try {
          hybrid.prepareStream(asset.path);
          applyHybridVolume();
          const durGuess =
            asset.durationMs != null
              ? asset.durationMs / 1000
              : hybrid.duration > 0
                ? hybrid.duration
                : 0;
          state.update((prev) => ({
            ...prev,
            currentAsset: asset,
            duration: durGuess,
            peaks: [],
            currentTime: 0,
            isPlaying: false,
          }));
          queuePeaks(asset.id);
          await hybrid.play();
          tick();
          return;
        } catch {
          /* fall back to PCM */
        }
      }

      if (shouldUseChunkedPcm(asset)) {
        let activeStreamId: number | null = null;
        streamUnlistenChunk = await listen<ipc.PcmStreamChunkPayload>(
          ipc.EVT_PCM_STREAM_CHUNK,
          (e) => {
            if (e.payload.streamId !== activeStreamId) return;
            hybrid.appendPcmStreamChunk(
              e.payload.path,
              e.payload.sampleRate,
              e.payload.channels,
              e.payload.streamId,
            );
          },
        );
        streamUnlistenDone = await listen<ipc.PcmStreamFinishedPayload>(
          ipc.EVT_PCM_STREAM_FINISHED,
          (e) => {
            if (e.payload.streamId !== activeStreamId) return;
            hybrid.markPcmStreamDecodeFinished();
          },
        );

        const start = await ipc.startPcmStream(asset.id);
        activeStreamId = start.streamId;

        const firstRes = await fetch(convertFileSrc(start.firstChunkPath));
        if (!firstRes.ok) {
          throw new Error(`pcm stream chunk ${firstRes.status}`);
        }
        const firstAb = await firstRes.arrayBuffer();
        hybrid.preparePcmStream(asset, start, firstAb);
        applyHybridVolume();

        const durGuess =
          asset.durationMs != null
            ? asset.durationMs / 1000
            : start.durationSec > 0
              ? start.durationSec
              : hybrid.duration > 0
                ? hybrid.duration
                : 0;
        state.update((prev) => ({
          ...prev,
          currentAsset: asset,
          duration: durGuess,
          peaks: [],
          currentTime: 0,
          isPlaying: false,
        }));
        queuePeaks(asset.id);
        await hybrid.play();
        tick();
        return;
      }

      const { ab, sr, ch } = await loadFullPcmForAsset(asset);
      hybrid.loadPcm(ab, sr, ch);
      applyHybridVolume();
      state.update((prev) => ({
        ...prev,
        currentAsset: asset,
        duration: hybrid.duration,
        peaks: [],
        currentTime: 0,
        isPlaying: false,
      }));
      queuePeaks(asset.id);
      await hybrid.play();
      tick();
    } catch (err) {
      toastStore.show(String(err), "error");
    }
  },

  setVolume(v: number): void {
    const clampedGain = Math.max(0, Math.min(1, v));
    hybrid.setVolume(clampedGain);
    state.update((prev) => ({ ...prev, volume: clampedGain }));
  },

  async previewProcessed(config: PostProcessConfig): Promise<void> {
    const snapshot = get(state);
    const asset = snapshot.currentAsset;
    const args = getAudioExportCopyArgs();
    if (!asset || !args) {
      toastStore.show("Nothing to preview", "error");
      return;
    }
    previewGeneration += 1;
    const gen = previewGeneration;
    state.update((prev) => ({ ...prev, previewLoading: true }));
    try {
      const meta = await ipc.getProcessedPcmFile(
        asset.id,
        args.isClip,
        args.startSec,
        args.endSec,
        config,
      );
      if (gen !== previewGeneration) return;
      const res = await fetch(convertFileSrc(meta.path));
      if (!res.ok) throw new Error(`preview pcm fetch ${res.status}`);
      const ab = await res.arrayBuffer();
      if (gen !== previewGeneration) return;
      await teardownPcmStreamPlayback();
      hybrid.stop();
      const sr = meta.sampleRate ?? asset.sampleRate ?? 44100;
      const ch = meta.channels ?? asset.channels ?? 1;
      hybrid.loadPcm(ab, sr, ch);
      applyHybridVolume();
      hybrid.setLoopPolicy(true, false);
      const peakRes = get(settingsStore).indexing.peak_resolution;
      const previewPeaks = computePeaksFromInterleavedPcmF32(ab, ch, peakRes);
      state.update((prev) => ({
        ...prev,
        duration: hybrid.duration,
        currentTime: 0,
        previewActive: true,
        previewLoading: false,
        previewPeaksOverride: previewPeaks,
      }));
      await hybrid.play();
      tick();
      state.update((prev) => ({ ...prev, isPlaying: true }));
    } catch (err) {
      if (gen === previewGeneration) {
        state.update((prev) => ({
          ...prev,
          previewLoading: false,
          previewActive: false,
          previewPeaksOverride: null,
        }));
        toastStore.show(String(err), "error");
      }
    }
  },

  async stopPreview(): Promise<void> {
    previewGeneration += 1;
    const asset = get(state).currentAsset;
    if (!asset) {
      state.update((prev) => ({
        ...prev,
        previewActive: false,
        previewLoading: false,
        previewPeaksOverride: null,
      }));
      return;
    }
    /** Keep `previewActive` true until `playAsset` reloads hybrid; avoids clamping clip to preview duration. */
    await playerStore.playAsset(asset, { preserveClip: true });
  },
  pause() {
    hybrid.pause();
    state.update((prev) => ({ ...prev, isPlaying: false, currentTime: hybrid.currentTime }));
    if (raf) cancelAnimationFrame(raf);
    raf = null;
  },
  async resume() {
    await clampPlayheadIntoClipIfNeeded();
    await hybrid.play();
    tick();
    state.update((prev) => ({ ...prev, isPlaying: true }));
  },
  async seek(position: number) {
    await seekInternal(position);
  },
  async stop() {
    await teardownPcmStreamPlayback();
    hybrid.stop();
    if (raf) cancelAnimationFrame(raf);
    raf = null;
    state.update((prev) => ({
      ...prev,
      isPlaying: false,
      currentTime: 0,
      currentAsset: null,
      peaks: [],
      clipRange: null,
      previewActive: false,
      previewLoading: false,
      previewPeaksOverride: null,
    }));
  },
  async toggle() {
    const snapshot = get(state);
    if (!snapshot.currentAsset) return;
    if (hybrid.isPlaying) {
      hybrid.pause();
      if (raf) cancelAnimationFrame(raf);
      raf = null;
      state.update((prev) => ({ ...prev, isPlaying: false, currentTime: hybrid.currentTime }));
    } else {
      try {
        await clampPlayheadIntoClipIfNeeded();
        await hybrid.play();
        tick();
        state.update((prev) => ({ ...prev, isPlaying: true }));
      } catch (err) {
        toastStore.show(String(err), "error");
      }
    }
  },
};

hybrid.setOnPcmStreamLoop(() => {
  const asset = get(state).currentAsset;
  if (!asset || !get(settingsStore).playback.loop_playback) return;
  void playerStore.playAsset(asset);
});
