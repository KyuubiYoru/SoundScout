<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { getPlayerState, playerStore } from "$lib/stores/playerStore";
  import { postProcessStore } from "$lib/stores/postProcessStore";

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

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
</script>

<div class="export-settings" aria-label="Export post-processing">
  <span class="label">Export</span>
  <div class="opts">
    <label class="opt">
      <input
        type="checkbox"
        checked={$postProcessStore.trimSilence}
        onchange={(e) =>
          postProcessStore.update((c) => ({ ...c, trimSilence: e.currentTarget.checked }))}
      />
      Trim silence
    </label>
    <label class="opt">
      <input
        type="checkbox"
        checked={$postProcessStore.normalizePeak}
        onchange={(e) =>
          postProcessStore.update((c) => ({ ...c, normalizePeak: e.currentTarget.checked }))}
      />
      Normalize
    </label>
    <label class="opt">
      <input
        type="checkbox"
        checked={$postProcessStore.makeLoopable}
        onchange={(e) =>
          postProcessStore.update((c) => ({ ...c, makeLoopable: e.currentTarget.checked }))}
      />
      Loopable
    </label>
    {#if $postProcessStore.makeLoopable}
      <label class="opt cross">
        <span>Crossfade</span>
        <select
          value={crossfadeSelectValue($postProcessStore)}
          onchange={onCrossfadeChange}
        >
          <option value="auto">auto</option>
          <option value="0.5">0.5 s</option>
          <option value="1">1 s</option>
          <option value="2">2 s</option>
        </select>
      </label>
      <label class="opt">
        <input
          type="checkbox"
          checked={$postProcessStore.embedSmplChunk}
          onchange={(e) =>
            postProcessStore.update((c) => ({ ...c, embedSmplChunk: e.currentTarget.checked }))}
        />
        smpl markers
      </label>
    {/if}
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

<style>
  .export-settings {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--spacing-sm) var(--spacing-md);
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    width: 100%;
    justify-content: flex-end;
  }
  .label {
    color: var(--text-muted);
    font-weight: 600;
    flex-shrink: 0;
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--spacing-xs) var(--spacing-md);
    justify-content: flex-end;
  }
  .opt {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-xs);
    cursor: pointer;
    user-select: none;
    white-space: nowrap;
  }
  .opt input {
    cursor: pointer;
  }
  .opt.cross span {
    margin-right: 2px;
  }
  .opt.cross select {
    max-width: 88px;
    padding: 2px 4px;
    font-size: var(--font-size-sm);
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-primary);
  }
  .preview-btn {
    padding: 2px 10px;
    font-size: var(--font-size-sm);
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    color: var(--text-secondary);
    cursor: pointer;
    margin-left: 4px;
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
