<script lang="ts">
  import type { FolderNode } from "$lib/types";
  import FolderTree from "./FolderTree.svelte";

  let {
    nodes,
    selectedPath,
    onselect,
  }: {
    nodes: FolderNode[];
    selectedPath: string | null;
    onselect: (path: string) => void;
  } = $props();
</script>

<ul class="tree">
  {#each nodes as n (n.path)}
    <li>
      <button
        type="button"
        class="node"
        class:active={selectedPath === n.path}
        onclick={() => onselect(n.path)}
      >
        <span class="name">{n.name}</span>
        <span class="cnt">{n.count}</span>
      </button>
      {#if n.children.length}
        <FolderTree nodes={n.children} {selectedPath} {onselect} />
      {/if}
    </li>
  {/each}
</ul>

<style>
  .tree {
    list-style: none;
    padding-left: var(--spacing-md);
  }
  .tree > li {
    margin: 2px 0;
  }
  .node {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    gap: var(--spacing-sm);
    padding: 4px 8px;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: var(--font-size-sm);
    text-align: left;
  }
  .node:hover {
    background: var(--bg-elevated);
    color: var(--text-primary);
  }
  .node.active {
    background: rgba(74, 144, 217, 0.15);
    color: var(--accent);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cnt {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }
</style>
