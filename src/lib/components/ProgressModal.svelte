<script lang="ts">
  import type { ScanProgress, ScanStats, EmbedRebuildProgress, EmbedRebuildComplete } from "$lib/types";
  import * as ipc from "$lib/ipc";

  let {
    open = $bindable(false),
    progress = null,
    doneStats = null,
    embedProgress = null,
    embedDoneStats = null,
  }: {
    open?: boolean;
    progress?: ScanProgress | null;
    doneStats?: ScanStats | null;
    embedProgress?: EmbedRebuildProgress | null;
    embedDoneStats?: EmbedRebuildComplete | null;
  } = $props();

  async function cancel() {
    try {
      await ipc.cancelScan();
    } catch {
      /* ignore */
    }
    open = false;
  }

  const scanPct = $derived(
    progress && progress.total > 0 ? Math.min(100, (100 * progress.scanned) / progress.total) : 0,
  );

  const embedPct = $derived(
    embedProgress && embedProgress.total > 0
      ? Math.min(100, (100 * embedProgress.processed) / embedProgress.total)
      : embedProgress
        ? 0
        : 0,
  );

  const showCancel = $derived(
    open && !doneStats && !embedDoneStats && embedProgress === null,
  );
</script>

{#if open}
  <div class="backdrop" role="presentation"></div>
  <div class="modal" role="alertdialog" aria-busy={!doneStats && !embedDoneStats} aria-live="polite">
    {#if embedDoneStats}
      <h2>Embeddings complete</h2>
      <p>
        Wrote vectors for {embedDoneStats.written} assets ({embedDoneStats.durationSecs.toFixed(1)}s)
      </p>
    {:else if embedProgress}
      <h2>Building embeddings…</h2>
      <div class="bar"><div class="fill" style:width="{embedPct}%"></div></div>
      {#if embedProgress.detail}
        <p class="file">{embedProgress.detail}</p>
      {/if}
      <p class="meta">
        {embedProgress.processed} / {embedProgress.total} assets examined
      </p>
    {:else if doneStats}
      <h2>Scan complete</h2>
      <p>
        Indexed {doneStats.filesIndexed}, skipped {doneStats.filesSkipped}, errors {doneStats.errors}
        ({doneStats.durationSecs.toFixed(1)}s)
      </p>
    {:else if progress}
      <h2>Scanning…</h2>
      <div class="bar"><div class="fill" style:width="{scanPct}%"></div></div>
      <p class="file">{progress.currentFile || "—"}</p>
      <p class="meta">{progress.scanned} / {progress.total} · {progress.phase}</p>
      {#if showCancel}
        <button type="button" class="btn" onclick={cancel}>Cancel</button>
      {/if}
    {:else}
      <h2>Starting…</h2>
      {#if showCancel}
        <button type="button" class="btn" onclick={cancel}>Cancel</button>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 8500;
  }
  .modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(440px, 90vw);
    padding: var(--spacing-xl);
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 10px;
    z-index: 8501;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.4);
  }
  h2 {
    margin-bottom: var(--spacing-md);
    font-size: var(--font-size-lg);
  }
  .bar {
    height: 8px;
    background: var(--bg-input);
    border-radius: 4px;
    overflow: hidden;
    margin-bottom: var(--spacing-md);
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.15s ease-out;
  }
  .file {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    margin-top: var(--spacing-sm);
    font-size: var(--font-size-sm);
    color: var(--text-muted);
  }
  .btn {
    margin-top: var(--spacing-lg);
    padding: var(--spacing-sm) var(--spacing-md);
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-primary);
    cursor: pointer;
  }
</style>
