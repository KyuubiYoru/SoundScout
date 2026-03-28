<script lang="ts">
  import type { FolderNode, TagWithCount } from "$lib/types";
  import FolderTree from "./FolderTree.svelte";

  let {
    tree,
    tags,
    selectedPath,
    onFolderSelect,
    onAllLibrary,
    onTagClick,
  }: {
    tree: FolderNode[];
    tags: TagWithCount[];
    selectedPath: string | null;
    onFolderSelect: (path: string) => void;
    onAllLibrary: () => void;
    onTagClick: (name: string) => void;
  } = $props();
</script>

<aside class="sidebar">
  <div class="head">
    <h2>Library</h2>
    <button type="button" class="link" onclick={onAllLibrary}>All</button>
  </div>
  <div class="scroll">
    {#if tree.length}
      <FolderTree nodes={tree} {selectedPath} onselect={onFolderSelect} />
    {:else}
      <p class="hint">Add a scan folder in Settings — the library will index automatically.</p>
    {/if}
  </div>
  <div class="tags-head">Tags</div>
  <div class="tags">
    {#each tags as t (t.id)}
      <button type="button" class="tag" onclick={() => onTagClick(t.name)}>
        {t.name}
        <span class="n">{t.count}</span>
      </button>
    {/each}
  </div>
</aside>

<style>
  .sidebar {
    width: var(--sidebar-width);
    flex-shrink: 0;
    border-right: 1px solid var(--border);
    background: var(--bg-surface);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--border);
  }
  h2 {
    font-size: var(--font-size-lg);
    font-weight: 600;
  }
  .link {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: var(--font-size-sm);
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: var(--spacing-sm) 0;
    min-height: 120px;
  }
  .hint {
    padding: var(--spacing-md);
    color: var(--text-muted);
    font-size: var(--font-size-sm);
  }
  .tags-head {
    padding: var(--spacing-sm) var(--spacing-md) 0;
    font-size: var(--font-size-sm);
    color: var(--text-muted);
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: var(--spacing-xs);
    padding: var(--spacing-sm) var(--spacing-md) var(--spacing-md);
  }
  .tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: var(--font-size-sm);
  }
  .tag:hover {
    border-color: var(--accent);
    color: var(--text-primary);
  }
  .n {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
</style>
