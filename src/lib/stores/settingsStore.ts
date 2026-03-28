import { writable, get } from "svelte/store";
import type { AppConfig } from "$lib/types";
import * as ipc from "$lib/ipc";

const emptyConfig = (): AppConfig => ({
  general: { scan_roots: [] },
  indexing: {
    parallel_workers: 0,
    peak_resolution: 800,
    skip_hidden_dirs: true,
    watch_scan_roots: false,
  },
  playback: {
    buffer_cache_count: 10,
    auto_play_on_select: true,
    loop_playback: false,
    clip_notch_ms: 100,
  },
  search: { default_sort: "relevance", results_per_page: 50, default_search_mode: "lexical" },
});

const state = writable<AppConfig>(emptyConfig());

export const settingsStore = {
  subscribe: state.subscribe,
  async load() {
    const config = await ipc.getConfig();
    const base = emptyConfig();
    state.set({
      ...base,
      ...config,
      playback: {
        ...base.playback,
        ...config.playback,
        loop_playback: config.playback.loop_playback ?? base.playback.loop_playback,
        clip_notch_ms: config.playback.clip_notch_ms ?? base.playback.clip_notch_ms,
      },
    });
  },
  async save() {
    await ipc.updateConfig(get(state));
  },
  addScanRoot(path: string) {
    state.update((cfg) => ({
      ...cfg,
      general: { scan_roots: [...cfg.general.scan_roots, path] },
    }));
  },
  removeScanRoot(path: string) {
    state.update((cfg) => ({
      ...cfg,
      general: { scan_roots: cfg.general.scan_roots.filter((p) => p !== path) },
    }));
  },
  setPeakResolution(n: number) {
    state.update((cfg) => ({
      ...cfg,
      indexing: { ...cfg.indexing, peak_resolution: n },
    }));
  },
  setAutoPlayOnSelect(v: boolean) {
    state.update((cfg) => ({
      ...cfg,
      playback: { ...cfg.playback, auto_play_on_select: v },
    }));
  },
  setWatchScanRoots(v: boolean) {
    state.update((cfg) => ({
      ...cfg,
      indexing: { ...cfg.indexing, watch_scan_roots: v },
    }));
  },
  setBufferCacheCount(n: number) {
    const clamped = Math.min(50, Math.max(1, Math.floor(n)));
    state.update((cfg) => ({
      ...cfg,
      playback: { ...cfg.playback, buffer_cache_count: clamped },
    }));
  },
  setLoopPlayback(v: boolean) {
    state.update((cfg) => ({
      ...cfg,
      playback: { ...cfg.playback, loop_playback: v },
    }));
  },
  setClipNotchMs(n: number) {
    const clamped = Math.min(10_000, Math.max(10, Math.floor(n)));
    state.update((cfg) => ({
      ...cfg,
      playback: { ...cfg.playback, clip_notch_ms: clamped },
    }));
  },
};
