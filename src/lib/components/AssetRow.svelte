<script lang="ts">
  import type { Asset } from "$lib/types";
  import { formatDuration, formatFileSize, formatSampleRate } from "$lib/utils/format";
  import * as ipc from "$lib/ipc";
  import { searchStore } from "$lib/stores/searchStore";

  let {
    asset,
    relevanceScore = null,
    scoreMode = null,
    selected = false,
    onclick,
    onplay,
    onDataChange,
  }: {
    asset: Asset;
    relevanceScore?: number | null;
    scoreMode?: "vector" | "both" | null;
    selected?: boolean;
    onclick?: (e: MouseEvent) => void;
    onplay?: () => void;
    /** After favorite/rating change (e.g. reload browse view). */
    onDataChange?: () => void | Promise<void>;
  } = $props();

  let scoreTitle = $derived(
    scoreMode === "vector"
      ? "Cosine similarity (−1…1, higher = closer)"
      : scoreMode === "both"
        ? "Hybrid score, vector + lexical (0…1, higher = better)"
        : "",
  );

  function onDragStart(e: DragEvent) {
    e.dataTransfer?.setData("text/plain", asset.path);
    e.dataTransfer?.setData("application/x-soundscout-path", asset.path);
  }

  async function refreshAfterEdit() {
    if (onDataChange) await onDataChange();
    else await searchStore.refresh();
  }

  function extClass(ext: string): string {
    const e = ext.toLowerCase().replace(".", "");
    if (e === "wav") return "wav";
    if (e === "flac") return "flac";
    if (e === "mp3") return "mp3";
    if (e === "ogg") return "ogg";
    if (e === "aiff" || e === "aif") return "aiff";
    return "";
  }

  async function toggleFav(e: MouseEvent) {
    e.stopPropagation();
    try {
      await ipc.toggleFavorite(asset.id);
      await refreshAfterEdit();
    } catch (err) {
      console.error(err);
    }
  }

  async function setRating(r: number, e: MouseEvent) {
    e.stopPropagation();
    try {
      await ipc.setRating(asset.id, r);
      await refreshAfterEdit();
    } catch (err) {
      console.error(err);
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="row"
  class:selected
  draggable="true"
  ondragstart={onDragStart}
  onclick={onclick}
  ondblclick={(e) => {
    e.stopPropagation();
    onplay?.();
  }}
  role="button"
  tabindex="0"
  title="Drag to a DAW or explorer to copy file path"
>
  <button type="button" class="star" class:on={asset.favorite} title="Favorite" onclick={toggleFav}>
    ★
  </button>
  <span class="badge {extClass(asset.extension)}">{asset.extension}</span>
  <div class="meta">
    <div class="name">{asset.filename}</div>
    <div class="sub">
      {asset.folder}
      {#if asset.sampleRate}
        · {formatSampleRate(asset.sampleRate)}
      {/if}
      {#if asset.channels}
        · {asset.channels}ch
      {/if}
      · {formatFileSize(asset.fileSize)}
    </div>
  </div>
  {#if relevanceScore != null && scoreMode}
    <span class="score-pill" title={scoreTitle}>{relevanceScore.toFixed(3)}</span>
  {/if}
  <div class="dur">{formatDuration(asset.durationMs)}</div>
  <div class="rate" onclick={(e) => e.stopPropagation()}>
    {#each [1, 2, 3, 4, 5] as r}
      <button type="button" class:active={asset.rating >= r} onclick={(e) => setRating(r, e)}>{r}</button>
    {/each}
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    height: var(--row-height);
    padding: 0 var(--spacing-md);
    border-bottom: 1px solid var(--border);
    cursor: pointer;
  }
  .row:hover {
    background: var(--bg-elevated);
  }
  .row.selected {
    background: rgba(74, 144, 217, 0.12);
    border-left: 3px solid var(--accent);
  }
  .star {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 16px;
    padding: 4px;
  }
  .star.on {
    color: var(--favorite);
  }
  .badge {
    font-size: 10px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
    text-transform: uppercase;
    min-width: 36px;
    text-align: center;
    background: var(--bg-input);
    color: var(--text-secondary);
  }
  .badge.wav {
    background: rgba(74, 144, 217, 0.2);
    color: var(--badge-wav);
  }
  .badge.flac {
    background: rgba(74, 217, 122, 0.15);
    color: var(--badge-flac);
  }
  .badge.mp3 {
    background: rgba(217, 145, 74, 0.15);
    color: var(--badge-mp3);
  }
  .badge.ogg {
    background: rgba(154, 74, 217, 0.15);
    color: var(--badge-ogg);
  }
  .badge.aiff {
    background: rgba(217, 74, 122, 0.15);
    color: var(--badge-aiff);
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-primary);
  }
  .sub {
    font-size: var(--font-size-sm);
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .score-pill {
    flex-shrink: 0;
    font-size: var(--font-size-base);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
    padding: 5px 10px;
    border-radius: 8px;
    text-align: center;
    min-width: 4.25rem;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 18%, var(--bg-input));
    border: 1px solid color-mix(in srgb, var(--accent) 42%, var(--border));
    box-shadow: 0 1px 0 color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .row.selected .score-pill {
    background: color-mix(in srgb, var(--accent) 28%, var(--bg-elevated));
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  }
  .dur {
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    min-width: 52px;
    text-align: right;
  }
  .rate {
    display: flex;
    gap: 2px;
  }
  .rate button {
    background: var(--bg-input);
    border: 1px solid var(--border);
    color: var(--text-muted);
    border-radius: 3px;
    width: 22px;
    height: 22px;
    font-size: 10px;
    cursor: pointer;
    padding: 0;
  }
  .rate button.active {
    color: var(--accent);
    border-color: var(--accent);
  }
</style>
