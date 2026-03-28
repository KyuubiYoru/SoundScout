<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { getPlayerState, playerStore } from "$lib/stores/playerStore";
  import { postProcessStore } from "$lib/stores/postProcessStore";
  import {
    closestNormalizePeakDbfs,
    defaultPostProcessConfig,
    NORMALIZE_PEAK_DBFS_OPTIONS,
    peakDbfsToLinear,
    trimSilencePresets,
    trimSilencePresetMatch,
    type PostProcessConfig,
    type TrimSilencePresetId,
  } from "$lib/types";

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  let open = $state(false);
  let wrapperEl: HTMLElement | null = $state(null);

  const THRESH_MIN = -96;
  const THRESH_MAX = -12;
  const MIN_SIL_MS_MAX = 500;
  const TRIM_WINDOW_OPTIONS = [2, 3, 4, 5, 6, 8] as const;

  function closestTrimWindowMs(ms: number | undefined): number {
    const m = ms != null && Number.isFinite(ms) ? ms : 4;
    return TRIM_WINDOW_OPTIONS.reduce((best, o) =>
      Math.abs(o - m) < Math.abs(best - m) ? o : best,
    );
  }

  function hasNonDefaultSettings(c: PostProcessConfig): boolean {
    return (
      c.trimSilence !== defaultPostProcessConfig.trimSilence ||
      c.normalizePeak !== defaultPostProcessConfig.normalizePeak ||
      Math.abs(c.normalizeTarget - defaultPostProcessConfig.normalizeTarget) > 1e-5 ||
      c.makeLoopable !== defaultPostProcessConfig.makeLoopable
    );
  }

  function isTriggerActive(preview: boolean, c: PostProcessConfig): boolean {
    return preview || hasNonDefaultSettings(c);
  }

  onMount(() => {
    const unsub = postProcessStore.subscribe(() => {
      if (!getPlayerState().previewActive) return;
      if (debounceTimer) clearTimeout(debounceTimer);
      debounceTimer = setTimeout(() => {
        debounceTimer = null;
        void playerStore.previewProcessed(get(postProcessStore));
      }, 200);
    });
    return () => {
      unsub();
      if (debounceTimer) clearTimeout(debounceTimer);
    };
  });

  function togglePreview() {
    const s = getPlayerState();
    if (s.previewActive) {
      void playerStore.stopPreview();
    } else {
      void playerStore.previewProcessed(get(postProcessStore));
    }
  }

  function onCrossfadeChange(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    postProcessStore.update((c) => ({
      ...c,
      crossfadeSec: v === "auto" ? null : Number(v),
    }));
  }

  function crossfadeSelectValue(c: { crossfadeSec: number | null }): string {
    if (c.crossfadeSec == null) return "auto";
    const x = c.crossfadeSec;
    if (x === 0.5 || x === 1 || x === 2) return String(x);
    return "auto";
  }

  function clampThreshold(v: number): number {
    if (!Number.isFinite(v)) return -60;
    return Math.min(THRESH_MAX, Math.max(THRESH_MIN, Math.round(v)));
  }

  function clampMinSilMs(v: number): number {
    if (!Number.isFinite(v) || v < 0) return 0;
    return Math.min(MIN_SIL_MS_MAX, Math.round(v));
  }

  function clampWindowMs(v: number): number {
    if (!Number.isFinite(v)) return 4;
    return Math.min(16, Math.max(2, Math.round(v * 4) / 4));
  }

  function trimPresetSelectValue(c: PostProcessConfig): string {
    const m = trimSilencePresetMatch(c);
    return m === "custom" ? "__custom__" : m;
  }

  function onTrimPresetChange(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    if (v === "__custom__") return;
    const p = trimSilencePresets[v as TrimSilencePresetId];
    postProcessStore.update((c) => ({
      ...c,
      trimThresholdDb: p.trimThresholdDb,
      trimMinSilenceMs: p.trimMinSilenceMs,
      trimPeakWindowMs: p.trimPeakWindowMs,
    }));
  }

  function onTrimThresholdInput(e: Event) {
    const raw = Number((e.currentTarget as HTMLInputElement).value);
    postProcessStore.update((c) => ({ ...c, trimThresholdDb: clampThreshold(raw) }));
  }

  function onTrimMinMsInput(e: Event) {
    const raw = Number((e.currentTarget as HTMLInputElement).value);
    postProcessStore.update((c) => ({ ...c, trimMinSilenceMs: clampMinSilMs(raw) }));
  }

  function onTrimWindowSelect(e: Event) {
    const raw = Number((e.currentTarget as HTMLSelectElement).value);
    postProcessStore.update((c) => ({ ...c, trimPeakWindowMs: clampWindowMs(raw) }));
  }

  function onNormalizePeakDbfsChange(e: Event) {
    const db = Number((e.currentTarget as HTMLSelectElement).value);
    if (!Number.isFinite(db)) return;
    postProcessStore.update((c) => ({ ...c, normalizeTarget: peakDbfsToLinear(db) }));
  }

  function onDocClick(e: MouseEvent) {
    if (open && wrapperEl && !wrapperEl.contains(e.target as Node)) open = false;
  }
</script>

<svelte:document onclick={onDocClick} />

<div class="pp-wrapper" bind:this={wrapperEl}>
  <button
    type="button"
    class="pp-trigger"
    class:open
    onclick={() => (open = !open)}
    aria-expanded={open}
    aria-haspopup="dialog"
  >
    Export options{#if isTriggerActive($playerStore.previewActive, $postProcessStore)}<span class="pp-dot" aria-hidden="true">·</span>{/if}
  </button>
  {#if open}
    <div
      id="export-options-popover"
      class="pp-popover"
      role="dialog"
      aria-label="Export options"
    >
      <section class="pp-section">
        <h3 class="pp-heading">Trim</h3>
        <label class="pp-check" title="Remove quiet gaps at the start and end of the file before exporting">
          <input
            type="checkbox"
            checked={$postProcessStore.trimSilence}
            onchange={(e) =>
              postProcessStore.update((c) => ({ ...c, trimSilence: e.currentTarget.checked }))}
          />
          Trim silence
        </label>
        <div class="pp-field">
          <span class="pp-label" id="trim-preset-label">Preset</span>
          <select
            class="pp-select pp-select-wide"
            value={trimPresetSelectValue($postProcessStore)}
            onchange={onTrimPresetChange}
            disabled={!$postProcessStore.trimSilence}
            title="Starting values; you can still adjust fields below"
            aria-labelledby="trim-preset-label"
          >
            <option value="balanced">Balanced</option>
            <option value="tight">Tight (more trim)</option>
            <option value="loose">Loose (less trim)</option>
            <option value="__custom__" disabled>Custom (adjust sliders)</option>
          </select>
        </div>
        <div class="pp-field">
          <span class="pp-label" id="trim-thresh-label">Silence threshold</span>
          <div class="pp-slider-row">
            <input
              type="range"
              class="pp-range"
              min={THRESH_MIN}
              max={THRESH_MAX}
              step="1"
              value={clampThreshold($postProcessStore.trimThresholdDb)}
              oninput={onTrimThresholdInput}
              disabled={!$postProcessStore.trimSilence}
              aria-labelledby="trim-thresh-label"
              aria-describedby="trim-thresh-hint"
            />
            <input
              type="number"
              class="pp-num"
              min={THRESH_MIN}
              max={THRESH_MAX}
              step="1"
              value={clampThreshold($postProcessStore.trimThresholdDb)}
              oninput={onTrimThresholdInput}
              disabled={!$postProcessStore.trimSilence}
              aria-labelledby="trim-thresh-label"
            />
            <span class="pp-unit">dBFS</span>
          </div>
          <p id="trim-thresh-hint" class="pp-hint">
            Peaks below this level count as silence (more negative = quieter material is still treated
            as sound).
          </p>
        </div>
        <div class="pp-field">
          <span class="pp-label" id="trim-min-label">Minimum silence to cut</span>
          <div class="pp-slider-row">
            <input
              type="range"
              class="pp-range"
              min="0"
              max={MIN_SIL_MS_MAX}
              step="1"
              value={clampMinSilMs($postProcessStore.trimMinSilenceMs)}
              oninput={onTrimMinMsInput}
              disabled={!$postProcessStore.trimSilence}
              aria-labelledby="trim-min-label"
              aria-describedby="trim-min-hint"
            />
            <input
              type="number"
              class="pp-num pp-num-wide"
              min="0"
              max={MIN_SIL_MS_MAX}
              step="1"
              value={clampMinSilMs($postProcessStore.trimMinSilenceMs)}
              oninput={onTrimMinMsInput}
              disabled={!$postProcessStore.trimSilence}
              aria-labelledby="trim-min-label"
            />
            <span class="pp-unit">ms</span>
          </div>
          <p id="trim-min-hint" class="pp-hint">Only trims an edge if the quiet run there is at least this long.</p>
        </div>
        <div class="pp-field">
          <span class="pp-label" id="trim-win-label">Analysis window</span>
          <select
            class="pp-select"
            value={String(closestTrimWindowMs($postProcessStore.trimPeakWindowMs))}
            onchange={onTrimWindowSelect}
            disabled={!$postProcessStore.trimSilence}
            title="Width of the level meter at each point: shorter reacts faster at boundaries"
            aria-labelledby="trim-win-label"
            aria-describedby="trim-win-hint"
          >
            <option value="2">2 ms</option>
            <option value="3">3 ms</option>
            <option value="4">4 ms</option>
            <option value="5">5 ms</option>
            <option value="6">6 ms</option>
            <option value="8">8 ms</option>
          </select>
          <p id="trim-win-hint" class="pp-hint">Shorter = snappier edge detection; longer = slightly smoother.</p>
        </div>
      </section>
      <hr class="pp-sep" />
      <section class="pp-section">
        <h3 class="pp-heading">Output</h3>
        <label
          class="pp-check"
          title="Scale so the loudest sample hits the peak target (below 0 dBFS for headroom)"
        >
          <input
            type="checkbox"
            checked={$postProcessStore.normalizePeak}
            onchange={(e) =>
              postProcessStore.update((c) => ({ ...c, normalizePeak: e.currentTarget.checked }))}
          />
          Normalize
        </label>
        {#if $postProcessStore.normalizePeak}
          <div class="pp-field">
            <span class="pp-label" id="norm-peak-label">Peak target</span>
            <select
              class="pp-select pp-select-wide"
              value={String(closestNormalizePeakDbfs($postProcessStore.normalizeTarget))}
              onchange={onNormalizePeakDbfsChange}
              aria-labelledby="norm-peak-label"
              aria-describedby="norm-peak-hint"
            >
              {#each NORMALIZE_PEAK_DBFS_OPTIONS as db}
                <option value={String(db)}>
                  {db} dBFS{db === -6 ? " — default" : ""}
                </option>
              {/each}
            </select>
            <p id="norm-peak-hint" class="pp-hint">
              Loudest sample scaled to this level below digital full scale (0 dBFS).
            </p>
          </div>
        {/if}
        <label class="pp-check" title="Blend the end of the file into its beginning so it repeats seamlessly">
          <input
            type="checkbox"
            checked={$postProcessStore.makeLoopable}
            onchange={(e) =>
              postProcessStore.update((c) => ({ ...c, makeLoopable: e.currentTarget.checked }))}
          />
          Loopable
        </label>
        {#if $postProcessStore.makeLoopable}
          <div class="pp-field pp-field-inline">
            <span class="pp-label">Crossfade</span>
            <select
              class="pp-select pp-crossfade"
              value={crossfadeSelectValue($postProcessStore)}
              onchange={onCrossfadeChange}
              title="How long the blend between the end and start should last"
            >
              <option value="auto">auto</option>
              <option value="0.5">0.5 s</option>
              <option value="1">1 s</option>
              <option value="2">2 s</option>
            </select>
          </div>
          <label
            class="pp-check"
            title="Write loop point markers into the WAV file so samplers and DAWs can read them"
          >
            <input
              type="checkbox"
              checked={$postProcessStore.embedSmplChunk}
              onchange={(e) =>
                postProcessStore.update((c) => ({ ...c, embedSmplChunk: e.currentTarget.checked }))}
            />
            smpl markers
          </label>
        {/if}
      </section>
      <hr class="pp-sep" />
      <div class="pp-footer">
        <button
          type="button"
          class="preview-btn"
          class:active={$playerStore.previewActive}
          disabled={!$playerStore.currentAsset || $playerStore.previewLoading}
          onclick={() => togglePreview()}
          aria-pressed={$playerStore.previewActive}
          aria-label={$playerStore.previewActive ? "Stop export preview" : "Preview export processing"}
        >
          {#if $playerStore.previewLoading}
            <span class="preview-spin" aria-hidden="true">…</span> Loading
          {:else if $playerStore.previewActive}
            Stop preview
          {:else}
            Preview
          {/if}
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .pp-wrapper {
    position: relative;
    display: inline-block;
  }

  .pp-trigger {
    padding: 5px 14px;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-primary);
    cursor: pointer;
    font-size: var(--font-size-sm);
    font-family: var(--font-mono);
  }
  .pp-trigger:hover {
    border-color: var(--accent);
  }
  .pp-trigger.open {
    border-color: var(--accent);
    color: var(--text-primary);
  }

  .pp-dot {
    color: var(--accent);
    margin-left: 4px;
    font-size: 1.2em;
    line-height: 1;
  }

  .pp-popover {
    position: absolute;
    bottom: calc(100% + 8px);
    right: 0;
    z-index: 200;
    min-width: 300px;
    max-width: 340px;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: var(--spacing-md);
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.5);
    box-sizing: border-box;
  }

  .pp-section {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
  }

  .pp-heading {
    margin: 0;
    font-size: var(--font-size-sm);
    font-weight: 600;
    color: var(--text-secondary);
  }

  .pp-check {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-xs);
    cursor: pointer;
    user-select: none;
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }
  .pp-check input {
    cursor: pointer;
  }

  .pp-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .pp-field-inline {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .pp-label {
    font-size: var(--font-size-sm);
    font-weight: 600;
    color: var(--text-secondary);
  }

  .pp-slider-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--spacing-sm);
    min-height: 28px;
  }

  .pp-range {
    flex: 1 1 120px;
    min-width: 100px;
    max-width: 100%;
    height: 6px;
    cursor: pointer;
    accent-color: var(--accent, #6b9fff);
  }
  .pp-range:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .pp-num {
    width: 3.25rem;
    padding: 4px 6px;
    font-size: var(--font-size-sm);
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-primary);
  }
  .pp-num:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .pp-num-wide {
    width: 3.75rem;
  }

  .pp-unit {
    flex-shrink: 0;
    color: var(--text-muted);
    font-size: var(--font-size-sm);
  }

  .pp-select {
    max-width: 100%;
    padding: 4px 8px;
    font-size: var(--font-size-sm);
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-primary);
    cursor: pointer;
  }
  .pp-select:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .pp-select-wide {
    max-width: 100%;
  }
  .pp-crossfade {
    max-width: 88px;
  }

  .pp-hint {
    margin: 0;
    font-size: 11px;
    line-height: 1.4;
    color: var(--text-muted);
    margin-top: 2px;
  }

  .pp-sep {
    border: none;
    border-top: 1px solid var(--border);
    margin: var(--spacing-sm) 0;
  }

  .pp-footer {
    margin-top: 0;
  }

  .preview-btn {
    width: 100%;
    padding: 5px 0;
    font-size: var(--font-size-sm);
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-secondary);
    cursor: pointer;
    font-family: var(--font-mono);
  }
  .preview-btn:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--text-primary);
  }
  .preview-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .preview-btn.active {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
  .preview-spin {
    display: inline-block;
    animation: pulse 0.8s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
