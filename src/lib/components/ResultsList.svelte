<script lang="ts">
  import type { Asset, SearchMode } from "$lib/types";
  import { virtualRange } from "$lib/utils/virtualScroll";
  import AssetRow from "./AssetRow.svelte";

  let {
    assets,
    relevanceScores = null,
    searchScoreMode = null,
    selectedId = null,
    multiSelectedIds = [],
    onselect,
    onDataChange,
    onplay,
    infiniteScroll = false,
    loadingMore = false,
    onLoadMore,
  }: {
    assets: Asset[];
    /** Parallel to `assets`; cosine or hybrid score from the indexer. */
    relevanceScores?: number[] | null;
    /** When set with scores, row tooltips explain the metric. */
    searchScoreMode?: SearchMode | null;
    selectedId?: number | null;
    /** Row highlight when batch-selecting with Ctrl/Cmd+click. */
    multiSelectedIds?: number[];
    onselect: (a: Asset, e: MouseEvent) => void;
    onplay?: (a: Asset) => void;
    onDataChange?: () => void | Promise<void>;
    /** When true, scrolling near the bottom loads the next page via `onLoadMore`. */
    infiniteScroll?: boolean;
    loadingMore?: boolean;
    onLoadMore?: () => void;
  } = $props();

  let scrollTop = $state(0);
  let clientHeight = $state(480);
  let scrollEl = $state<HTMLDivElement | null>(null);
  let sentinelEl = $state<HTMLDivElement | null>(null);

  const rowHeight = 56;
  /** Extra scroll extent so the bottom sentinel can intersect (search infinite scroll). */
  const sentinelGap = 32;

  let r = $derived(virtualRange(scrollTop, clientHeight, rowHeight, assets.length));
  let slice = $derived(assets.slice(r.start, r.end));
  let listPixelHeight = $derived(assets.length * rowHeight);
  let innerHeight = $derived(listPixelHeight + (infiniteScroll ? sentinelGap : 0));

  $effect(() => {
    if (!scrollEl) return;
    const el = scrollEl;
    const ro = new ResizeObserver(() => {
      clientHeight = el.clientHeight;
    });
    ro.observe(el);
    clientHeight = el.clientHeight;
    return () => ro.disconnect();
  });

  $effect(() => {
    if (!infiniteScroll || !scrollEl || !sentinelEl || !onLoadMore) return;

    const blocked = loadingMore;
    const obs = new IntersectionObserver(
      (entries) => {
        if (!entries[0]?.isIntersecting || blocked) return;
        onLoadMore();
      },
      { root: scrollEl, rootMargin: "400px", threshold: 0 },
    );
    obs.observe(sentinelEl);
    return () => obs.disconnect();
  });
</script>

<div
  class="list"
  bind:this={scrollEl}
  onscroll={(e) => {
    scrollTop = (e.currentTarget as HTMLDivElement).scrollTop;
  }}
>
  <div class="inner" style:height="{innerHeight}px">
    <div class="slice" style:transform="translateY({r.offsetY}px)">
      {#each slice as asset, i (asset.id)}
        <AssetRow
          {asset}
          relevanceScore={relevanceScores?.[r.start + i] ?? null}
          scoreMode={searchScoreMode === "lexical" ? null : searchScoreMode}
          selected={asset.id === selectedId || multiSelectedIds.includes(asset.id)}
          onclick={(e) => onselect(asset, e)}
          onplay={() => onplay?.(asset)}
          {onDataChange}
        />
      {/each}
    </div>
    {#if infiniteScroll}
      <div
        class="sentinel"
        style:top="{listPixelHeight}px"
        bind:this={sentinelEl}
        aria-hidden="true"
      ></div>
    {/if}
  </div>
</div>

<style>
  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .inner {
    position: relative;
  }
  .slice {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
  }
  .sentinel {
    position: absolute;
    left: 0;
    right: 0;
    height: 32px;
    pointer-events: none;
    visibility: hidden;
  }
</style>
