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
</script>

<footer class="player">
  {#if $playerStore.currentAsset}
    <div class="info">
      <div class="title">{$playerStore.currentAsset.filename}</div>
      <div class="time">
        {fmt($playerStore.currentTime)} / {fmt($playerStore.duration)}
        {#if $playerStore.previewActive && $playerStore.clipRange}
          <span class="clip-mark"> · clip {fmt($playerStore.clipRange.start)}–{fmt($playerStore.clipRange.end)} (preview)</span>
        {:else if $playerStore.clipRange}
          <span class="clip-mark"> · {fmt($playerStore.clipRange.start)}–{fmt($playerStore.clipRange.end)}</span>
        {/if}
      </div>
    </div>
    <div class="wave">
      <Waveform
        peaks={$playerStore.previewPeaksOverride ?? $playerStore.peaks}
        currentTime={$playerStore.currentTime}
        duration={$playerStore.duration}
        clipRange={$playerStore.previewActive && $playerStore.clipRange ? null : $playerStore.clipRange}
        onSeek={(t) => void playerStore.seek(t)}
        onClipChange={(a, b) => void playerStore.commitClipRange(a, b)}
      />
      <p class="wf-hint">
        {#if $playerStore.previewActive}
          Previewing processed export — waveform matches what you hear · Stop preview to edit clip
        {:else}
          Shift-drag waveform to set clip · Play stays within clip · Loop repeats the clip{#if tauri} · Export / Copy
          (full track or clip; Ctrl/⌘+C){/if}
        {/if}
      </p>
    </div>
    <div class="controls-col">
      <div class="controls-row actions">
        <button type="button" onclick={() => void playerStore.toggle()} aria-label="Play or pause">
          {$playerStore.isPlaying ? "Pause" : "Play"}
        </button>
        <button type="button" onclick={() => playerStore.stop()} aria-label="Stop">Stop</button>
        {#if $playerStore.clipRange}
          <button type="button" class="ghost" onclick={() => playerStore.clearClipRange()} aria-label="Clear clip">
            Clear clip
          </button>
        {/if}
        {#if tauri}
          <button
            type="button"
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
        {/if}
        <label class="loop">
          <input
            type="checkbox"
            checked={$settingsStore.playback.loop_playback}
            onchange={(e) => void playerStore.setLoopPlaybackAndSave(e.currentTarget.checked)}
          />
          Loop
        </label>
        <div class="vol" title="Volume">
          <span class="vol-icon" aria-hidden="true">🔊</span>
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
      {#if tauri}
        <div class="controls-row export-opts">
          <PostProcessPanel />
        </div>
      {/if}
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
  .controls-row.export-opts {
    width: 100%;
    padding-top: 2px;
    border-top: 1px solid var(--border);
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
    padding: var(--spacing-sm) var(--spacing-md);
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
