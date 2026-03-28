<script lang="ts">
  import { onMount } from "svelte";
  import type { SortField, SortDirection, SearchMode } from "$lib/types";
  import * as ipc from "$lib/ipc";
  import { searchStore } from "$lib/stores/searchStore";
  import { embeddingIndexEpoch } from "$lib/stores/embeddingStatusStore";

  let embeddingCount = $state(0);

  const modes: { id: SearchMode; label: string }[] = [
    { id: "lexical", label: "Lexical" },
    { id: "vector", label: "Vector" },
    { id: "both", label: "Both" },
  ];

  async function refreshEmbeddingsStatus() {
    try {
      const s = await ipc.getSemanticSearchStatus();
      embeddingCount = Number(s.embeddingCount);
    } catch {
      embeddingCount = 0;
    }
  }

  $effect(() => {
    $embeddingIndexEpoch;
    void refreshEmbeddingsStatus();
  });

  onMount(() => {
    const onVis = () => {
      if (document.visibilityState === "visible") void refreshEmbeddingsStatus();
    };
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  });

  function setMode(id: SearchMode) {
    if ((id === "vector" || id === "both") && embeddingCount === 0) return;
    searchStore.setFilter("searchMode", id);
  }

  let currentMode = $derived(($searchStore.query.searchMode ?? "lexical") as SearchMode);
</script>

<div class="filter-bar">
  <div class="segmented" role="group" aria-label="Search mode">
    {#each modes as m (m.id)}
      {@const disabled = (m.id === "vector" || m.id === "both") && embeddingCount === 0}
      <button
        type="button"
        class:active={currentMode === m.id}
        {disabled}
        title={disabled ? "Go to Settings and run Rebuild text embeddings to enable this mode" : m.label === "Lexical" ? "Search by matching exact words and phrases" : m.label === "Vector" ? "Search by meaning, not just exact words" : "Search by both meaning and exact words"}
        onclick={() => setMode(m.id)}>{m.label}</button>
    {/each}
  </div>
  <label class="chk" title="Show only files you have marked as favorites">
    <input
      type="checkbox"
      checked={$searchStore.query.favoritesOnly}
      onchange={(e) => searchStore.setFilter("favoritesOnly", e.currentTarget.checked)}
    />
    Favorites
  </label>
  <label title="Choose how to order the results">
    Sort
    <select
      value={$searchStore.query.sortBy}
      onchange={(e) => searchStore.setFilter("sortBy", e.currentTarget.value as SortField)}
    >
      <option value="relevance">Relevance</option>
      <option value="name">Name</option>
      <option value="duration">Duration</option>
      <option value="date">Date</option>
    </select>
  </label>
  <label title="Switch between descending and ascending order">
    Dir
    <select
      value={$searchStore.query.sortDir}
      onchange={(e) => searchStore.setFilter("sortDir", e.currentTarget.value as SortDirection)}
    >
      <option value="desc">Desc</option>
      <option value="asc">Asc</option>
    </select>
  </label>
</div>

<style>
  .filter-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--spacing-md);
    padding: var(--spacing-sm) 0;
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }
  .segmented {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
    background: var(--bg-input);
  }
  .segmented button {
    margin: 0;
    padding: 4px 10px;
    border: none;
    border-right: 1px solid var(--border);
    background: transparent;
    color: var(--text-secondary);
    font-size: var(--font-size-sm);
    cursor: pointer;
  }
  .segmented button:last-child {
    border-right: none;
  }
  .segmented button:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-primary);
  }
  .segmented button.active {
    background: var(--accent);
    color: #fff;
  }
  .segmented button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .chk {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    cursor: pointer;
  }
  label :global(select) {
    margin-left: var(--spacing-xs);
  }
</style>
