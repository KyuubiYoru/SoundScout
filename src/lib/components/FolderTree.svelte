<script lang="ts">
  import type { FolderNode } from "$lib/types";
  import FolderTree from "./FolderTree.svelte";

  let {
    nodes,
    selectedPath,
    collapsedPaths,
    onToggleCollapsed,
    onselect,
  }: {
    nodes: FolderNode[];
    selectedPath: string | null;
    collapsedPaths: Set<string>;
    onToggleCollapsed: (path: string) => void;
    onselect: (path: string) => void;
  } = $props();
</script>

<ul class="tree">
  {#each nodes as n (n.path)}
    <li>
      <div class="row">
        {#if n.children.length}
          <button
            type="button"
            class="disclosure"
            aria-expanded={!collapsedPaths.has(n.path)}
            aria-label={`${collapsedPaths.has(n.path) ? "Expand" : "Collapse"} folder ${n.name}`}
            onclick={(e) => {
              e.stopPropagation();
              onToggleCollapsed(n.path);
            }}
          >
            <span class="chev" class:closed={collapsedPaths.has(n.path)}>▸</span>
          </button>
        {:else}
          <span class="disclosure-spacer" aria-hidden="true"></span>
        {/if}
        <button
          type="button"
          class="node"
          class:active={selectedPath === n.path}
          onclick={() => onselect(n.path)}
        >
          <span class="name">{n.name}</span>
          <span class="cnt">{n.count}</span>
        </button>
      </div>
      {#if n.children.length && !collapsedPaths.has(n.path)}
        <FolderTree
          nodes={n.children}
          {selectedPath}
          {collapsedPaths}
          {onToggleCollapsed}
          {onselect}
        />
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
  .row {
    display: flex;
    align-items: stretch;
    gap: 0;
    width: 100%;
  }
  .disclosure {
    flex: 0 0 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    flex-shrink: 0;
  }
  .disclosure:hover {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }
  .disclosure-spacer {
    flex: 0 0 22px;
    flex-shrink: 0;
  }
  .chev {
    display: inline-block;
    font-size: 10px;
    line-height: 1;
    transition: transform 0.12s ease;
  }
  .chev.closed {
    transform: rotate(-90deg);
  }
  .node {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex: 1;
    min-width: 0;
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
