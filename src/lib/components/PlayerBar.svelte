<script lang="ts">
  import { get } from "svelte/store";
  import { isTauri } from "@tauri-apps/api/core";
  import * as ipc from "$lib/ipc";
  import PostProcessPanel from "./PostProcessPanel.svelte";
  import { postProcessStore } from "$lib/stores/postProcessStore";
  import { getAudioExportCopyArgs, playerStore } from "$lib/stores/playerStore";
  import { settingsStore } from "$lib/stores/settingsStore";
  import { toastStore } from "$lib/stores/toastStore";
  import Waveform from "./Waveform.svelte";

  const tauri = isTauri();

  async function exportAudio() {
    const a = get(playerStore).currentAsset;
    const args = getAudioExportCopyArgs();
    if (!a || !args) return;
    try {
      const p = await ipc.exportClipWav(
        a.id,
        args.isClip,
        args.startSec,
        args.endSec,
        get(postProcessStore),
      );
      if (p) toastStore.show(`Exported to ${p}`, "success", 5000);
    } catch (e) {
      toastStore.show(String(e), "error");
    }
  }

  async function copyAudio() {
    const a = get(playerStore).currentAsset;
    const args = getAudioExportCopyArgs();
    if (!a || !args) return;
    try {
      await ipc.copyClipWavToClipboard(
        a.id,
        args.isClip,
        args.startSec,
        args.endSec,
        get(postProcessStore),
      );
      toastStore.show(
        args.isClip
          ? "Clip copied — paste as a file (keep this app open until you paste on Linux)"
          : "Track copied — paste as a file (keep this app open until you paste on Linux)",
        "success",
        6000,
      );
    } catch (e) {
      toastStore.show(String(e), "error");
    }
  }

  function fmt(t: number): string {
    if (!Number.isFinite(t) || t < 0) return "0:00";
    const m = Math.floor(t / 60);
    const s = Math.floor(t % 60);
    return `${m}:${String(s).padStart(2, "0")}`;
  }

  function fmtDur(sec: number): string {
    if (!Number.isFinite(sec) || sec < 0) return "0.0s";
    if (sec >= 60) {
      const m = Math.floor(sec / 60);
      const s = (sec % 60).toFixed(1);
      return `${m}m ${s}s`;
    }
    return `${sec.toFixed(1)}s`;
  }

  async function onClipChange(start: number, end: number) {
    const snap = get(playerStore);
    if (snap.previewActive) {
      await playerStore.commitClipRangeAndRefreshPreview(start, end, get(postProcessStore));
    } else {
      await playerStore.commitClipRange(start, end);
    }
  }

  function onWaveformSeek(t: number) {
    const snap = get(playerStore);
    if (snap.previewActive && snap.clipRange && snap.duration > 0) {
      const span = snap.clipRange.end - snap.clipRange.start;
      if (span > 0) {
        const rel = (t - snap.clipRange.start) / span;
        const pos = Math.max(0, Math.min(snap.duration, rel * snap.duration));
        void playerStore.seek(pos);
        return;
      }
    }
    void playerStore.seek(t);
  }
</script>

<footer class="player">
  {#if $playerStore.currentAsset}
    <div class="info">
      <div class="title">{$playerStore.currentAsset.filename}</div>
      <div class="time">
        {fmt($playerStore.currentTime)} / {fmt($playerStore.duration)}
        {#if $playerStore.previewActive && $playerStore.clipRange}
          <span class="clip-mark"> · clip {fmt($playerStore.clipRange.start)}–{fmt($playerStore.clipRange.end)} ({fmtDur($playerStore.clipRange.end - $playerStore.clipRange.start)}, preview)</span>
        {:else if $playerStore.clipRange}
          <span class="clip-mark"> · {fmt($playerStore.clipRange.start)}–{fmt($playerStore.clipRange.end)} ({fmtDur($playerStore.clipRange.end - $playerStore.clipRange.start)})</span>
        {/if}
      </div>
    </div>
    <div class="wave">
      <Waveform
        peaks={$playerStore.peaks}
        currentTime={$playerStore.previewActive &&
        $playerStore.clipRange &&
        $playerStore.duration > 0
          ? $playerStore.clipRange.start +
            ($playerStore.currentTime / $playerStore.duration) *
              ($playerStore.clipRange.end - $playerStore.clipRange.start)
          : $playerStore.currentTime}
        duration={$playerStore.currentAsset?.durationMs != null
          ? $playerStore.currentAsset.durationMs / 1000
          : $playerStore.duration}
        clipRange={$playerStore.clipRange}
        zoomToClipPreview={$playerStore.previewActive && $playerStore.clipRange != null}
        onSeek={onWaveformSeek}
        onClipChange={onClipChange}
      />
      <p class="wf-hint">
        {#if $playerStore.previewActive}
          {#if $playerStore.clipRange}
            Clip: {fmt($playerStore.clipRange.start)}–{fmt($playerStore.clipRange.end)} ({fmtDur($playerStore.clipRange.end - $playerStore.clipRange.start)}) · Waveform zoomed to clip. Drag handles or notch keys to adjust; zoom follows the clip.
          {:else}
            Drag edge handles to adjust clip. Changes restart the preview.
          {/if}
        {:else}
          Shift-drag or drag edge handles to set clip.{#if tauri} Ctrl/⌘+C to copy.{/if}
          i / o: notch clip start left / right · Shift+i / Shift+o: notch end left / right ({($settingsStore.playback.clip_notch_ms ?? 100)} ms — Settings).
        {/if}
      </p>
    </div>
    <div class="controls-col">
      <div class="controls-row actions">
        <button type="button" title={$playerStore.isPlaying ? "Pause playback" : "Start playback"} onclick={() => void playerStore.toggle()} aria-label="Play or pause">
          {$playerStore.isPlaying ? "Pause" : "Play"}
        </button>
        <button type="button" title="Stop playback and return to the start" onclick={() => playerStore.stop()} aria-label="Stop">Stop</button>
        {#if $playerStore.clipRange}
          <button type="button" class="ghost" title="Remove the selected clip region and use the full track" onclick={() => playerStore.clearClipRange()} aria-label="Clear clip">
            Clear clip
          </button>
        {/if}
        {#if tauri}
          <button
            type="button"
            title="Save the current track or clip as a WAV file"
            onclick={() => void exportAudio()}
            disabled={!(
              $playerStore.clipRange != null ||
              (Number.isFinite($playerStore.duration) && $playerStore.duration > 0) ||
              (($playerStore.currentAsset.durationMs ?? 0) > 0)
            )}
            aria-label="Export as WAV"
          >
            Export…
          </button>
          <button
            type="button"
            title="Copy the current track or clip to your clipboard as a file"
            onclick={() => void copyAudio()}
            disabled={!(
              $playerStore.clipRange != null ||
              (Number.isFinite($playerStore.duration) && $playerStore.duration > 0) ||
              (($playerStore.currentAsset.durationMs ?? 0) > 0)
            )}
            aria-label="Copy to clipboard as file"
          >
            Copy
          </button>
          <PostProcessPanel />
        {/if}
        <label class="loop" title="Repeat the track or clip continuously">
          <input
            type="checkbox"
            checked={$settingsStore.playback.loop_playback}
            onchange={(e) => void playerStore.setLoopPlaybackAndSave(e.currentTarget.checked)}
          />
          Loop
        </label>
        <div class="vol" title="Adjust playback volume">
          <span class="vol-icon" aria-hidden="true">vol</span>
          <input
            type="range"
            class="vol-slider"
            min="0"
            max="1"
            step="0.01"
            value={$playerStore.volume}
            aria-label="Playback volume"
            oninput={(e) => playerStore.setVolume(e.currentTarget.valueAsNumber)}
          />
        </div>
      </div>
    </div>
  {:else}
    <div class="empty">Select a file to play</div>
  {/if}
</footer>

<style>
  .player {
    min-height: var(--player-height);
    border-top: 1px solid var(--border);
    background: var(--bg-surface);
    display: flex;
    align-items: stretch;
    gap: var(--spacing-lg);
    padding: var(--spacing-sm) var(--spacing-lg);
    flex-shrink: 0;
  }
  .info {
    min-width: 140px;
    max-width: 220px;
    align-self: center;
  }
  .title {
    font-size: var(--font-size-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .time {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }
  .clip-mark {
    color: var(--text-muted);
  }
  .wave {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 0;
    justify-content: center;
  }
  .wf-hint {
    margin: 0;
    font-size: 11px;
    color: var(--text-muted);
    line-height: 1.2;
  }
  .controls-col {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    justify-content: center;
    gap: var(--spacing-sm);
    flex-shrink: 0;
    min-width: min(100%, 320px);
  }
  .controls-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--spacing-sm);
  }
  .loop {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    cursor: pointer;
    user-select: none;
  }
  .loop input {
    cursor: pointer;
  }
  .vol-icon {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .vol {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .vol-slider {
    width: 72px;
    height: 4px;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .controls-col button {
    padding: 5px 14px;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-primary);
    cursor: pointer;
  }
  .controls-col button:hover {
    border-color: var(--accent);
  }
  .controls-col button.ghost {
    background: transparent;
    color: var(--text-secondary);
  }
  .empty {
    color: var(--text-muted);
    font-size: var(--font-size-sm);
  }
</style>
