<script lang="ts">
  import { get } from "svelte/store";
  import { settingsStore } from "$lib/stores/settingsStore";
  import { searchStore } from "$lib/stores/searchStore";
  import { playerStore } from "$lib/stores/playerStore";
  import * as ipc from "$lib/ipc";
  import { toastStore } from "$lib/stores/toastStore";
  import { bumpEmbeddingIndexEpoch } from "$lib/stores/embeddingStatusStore";

  let {
    open = $bindable(false),
    onLibraryWiped,
    onStartScanAfterFolderAdded,
    onRescan,
  }: {
    open?: boolean;
    onLibraryWiped?: () => void | Promise<void>;
    /** Save + scan + auto-embed (parent owns progress modal). */
    onStartScanAfterFolderAdded?: () => void | Promise<void>;
    onRescan?: () => void | Promise<void>;
  } = $props();

  let embedStatus = $state<Awaited<ReturnType<typeof ipc.getSemanticSearchStatus>> | null>(null);
  let rebuildBusy = $state(false);
  let wipeBusy = $state(false);

  async function loadEmbedStatus() {
    try {
      embedStatus = await ipc.getSemanticSearchStatus();
    } catch {
      embedStatus = null;
    }
  }

  $effect(() => {
    if (open) void loadEmbedStatus();
  });

  async function doRebuildEmbeddings() {
    rebuildBusy = true;
    try {
      const n = await ipc.rebuildTextEmbeddings();
      toastStore.show(`Text embeddings updated: ${n} assets`, "success", 5000);
      await loadEmbedStatus();
    } catch (e) {
      toastStore.show(String(e), "error");
    } finally {
      rebuildBusy = false;
    }
  }

  async function addRoot() {
    try {
      const p = await ipc.pickDirectory();
      if (!p) return;
      settingsStore.addScanRoot(p);
      await settingsStore.save();
      await onStartScanAfterFolderAdded?.();
    } catch (e) {
      toastStore.show(String(e), "error");
    }
  }

  async function rescanLibrary() {
    const roots = get(settingsStore).general.scan_roots;
    if (!roots.length) {
      toastStore.show("Add a folder first", "info");
      return;
    }
    await onRescan?.();
  }

  async function save() {
    try {
      await settingsStore.save();
      toastStore.show("Settings saved — restart the app if you changed folder watch.", "success", 3200);
      open = false;
    } catch (e) {
      toastStore.show(String(e), "error");
    }
  }

  async function doExport() {
    try {
      const p = await ipc.exportDatabase();
      if (p) toastStore.show(`Exported to ${p}`, "success", 4000);
    } catch (e) {
      toastStore.show(String(e), "error");
    }
  }

  async function doImport() {
    try {
      await ipc.importDatabase();
    } catch (e) {
      toastStore.show(String(e), "error");
    }
  }

  async function doWipeDatabase() {
    wipeBusy = true;
    try {
      const wiped = await ipc.wipeLibraryDatabase();
      if (!wiped) return;
      await playerStore.stop();
      await searchStore.refresh();
      await loadEmbedStatus();
      bumpEmbeddingIndexEpoch();
      await onLibraryWiped?.();
      toastStore.show("Library database cleared.", "success", 5000);
    } catch (e) {
      toastStore.show(String(e), "error");
    } finally {
      wipeBusy = false;
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={() => (open = false)} role="presentation"></div>
  <div class="panel" role="dialog" aria-labelledby="settings-title">
    <header>
      <h2 id="settings-title">Settings</h2>
      <button type="button" class="x" onclick={() => (open = false)} aria-label="Close">×</button>
    </header>
    <div class="body">
      <section>
        <h3>Scan folders</h3>
        <ul class="roots">
          {#each $settingsStore.general.scan_roots as r (r)}
            <li>
              <code>{r}</code>
              <button
                type="button"
                class="rm"
                onclick={() => settingsStore.removeScanRoot(r)}
                aria-label="Remove folder"
              >
                Remove
              </button>
            </li>
          {/each}
        </ul>
        <div class="row-btns">
          <button type="button" class="btn" title="Choose a folder to add to your library" onclick={addRoot}>Add folder…</button>
          <button type="button" class="btn" title="Scan all folders again to pick up new or removed files" onclick={() => void rescanLibrary()}>Rescan library</button>
        </div>
        <p class="hint">Adding a folder saves your settings, scans it for audio files, then builds the search index. A progress window will appear while this runs.</p>
      </section>
      <section>
        <h3>Indexing</h3>
        <label class="field">
          Peak resolution
          <input
            type="number"
            min="50"
            max="8000"
            value={$settingsStore.indexing.peak_resolution}
            onchange={(e) => settingsStore.setPeakResolution(Number(e.currentTarget.value))}
          />
        </label>
        <p class="hint">Controls how detailed the waveform display is. Higher values show more detail but use slightly more disk space. Run Rescan library to apply this change to files already in the library.</p>
        <label class="chk">
          <input
            type="checkbox"
            checked={$settingsStore.indexing.watch_scan_roots}
            onchange={(e) => settingsStore.setWatchScanRoots(e.currentTarget.checked)}
          />
          Watch scan folders for new or removed files (restart the app for this to take effect)
        </label>
      </section>
      <section>
        <h3>Search — text embeddings</h3>
        <p class="hint">
          Vector and hybrid search use an AI model to understand the meaning of your searches. The first time you rebuild, the app may download around 90 MB of model data. Once downloaded, it works offline. You can move the cache folder to other machines to skip the download.
        </p>
        {#if embedStatus}
          <p class="embed-stat">
            Indexed vectors: <strong>{embedStatus.embeddingCount}</strong> / {embedStatus.assetCount} assets
          </p>
        {/if}
        <button type="button" class="btn" disabled={rebuildBusy} onclick={doRebuildEmbeddings}>
          {rebuildBusy ? "Rebuilding…" : "Rebuild text embeddings"}
        </button>
        <p class="hint">
          Run this after adding or editing tags and notes so meaning-based search reflects your changes.
        </p>
      </section>
      <section>
        <h3>Playback</h3>
        <label class="chk">
          <input
            type="checkbox"
            checked={$settingsStore.playback.auto_play_on_select}
            onchange={(e) => settingsStore.setAutoPlayOnSelect(e.currentTarget.checked)}
          />
          Start playing automatically when you click a file
        </label>
        <label class="chk">
          <input
            type="checkbox"
            checked={$settingsStore.playback.loop_playback}
            onchange={(e) => {
              settingsStore.setLoopPlayback(e.currentTarget.checked);
              playerStore.syncLoopFromConfig();
            }}
          />
          Loop playback
        </label>
        <label class="field">
          Audio cache size (number of files)
          <input
            type="number"
            min="1"
            max="50"
            value={$settingsStore.playback.buffer_cache_count}
            onchange={(e) => settingsStore.setBufferCacheCount(Number(e.currentTarget.value))}
          />
        </label>
        <label class="field">
          Clip notch step (ms)
          <input
            type="number"
            min="10"
            max="10000"
            step="10"
            value={$settingsStore.playback.clip_notch_ms ?? 100}
            onchange={(e) => settingsStore.setClipNotchMs(Number(e.currentTarget.value))}
          />
        </label>
        <p class="hint">
          <kbd>i</kbd> / <kbd>o</kbd> nudge the clip <strong>start</strong> left or right; <kbd>Shift</kbd>+<kbd>i</kbd> / <kbd>Shift</kbd>+<kbd>o</kbd> nudge the <strong>end</strong>.
        </p>
      </section>
      <section>
        <h3>Library database</h3>
        <p class="hint">Export saves a backup of your entire library, including tags, ratings, and search data. Import loads a backup and restarts the app.</p>
        <div class="row-btns">
          <button type="button" class="btn" title="Save a backup of your library to a file" onclick={doExport}>Export database…</button>
          <button type="button" class="btn danger" title="Load a library backup from a file. The app will restart." onclick={doImport}>Import database…</button>
        </div>
        <p class="hint">
          Clear removes all files, tags, and ratings from the library. The database file itself stays in place. Export a backup first if you want to keep your data.
        </p>
        <button type="button" class="btn danger" disabled={wipeBusy} onclick={doWipeDatabase}>
          {wipeBusy ? "Please wait…" : "Clear library database…"}
        </button>
      </section>
    </div>
    <footer>
      <button type="button" class="btn primary" onclick={save}>Save</button>
    </footer>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    z-index: 8000;
  }
  .panel {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(520px, 92vw);
    max-height: 85vh;
    overflow: auto;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 10px;
    z-index: 8001;
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-lg);
    border-bottom: 1px solid var(--border);
  }
  h2 {
    font-size: var(--font-size-xl);
  }
  .x {
    background: none;
    border: none;
    color: var(--text-secondary);
    font-size: 22px;
    cursor: pointer;
  }
  .body {
    padding: var(--spacing-lg);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xl);
  }
  section h3 {
    font-size: var(--font-size-base);
    margin-bottom: var(--spacing-md);
    color: var(--text-secondary);
  }
  .roots {
    list-style: none;
    margin-bottom: var(--spacing-md);
  }
  .roots li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-sm);
    padding: var(--spacing-sm) 0;
    border-bottom: 1px solid var(--border);
    font-size: var(--font-size-sm);
  }
  code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rm {
    flex-shrink: 0;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    padding: 2px 8px;
    font-size: var(--font-size-sm);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }
  .field input {
    max-width: 120px;
    padding: var(--spacing-sm);
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-primary);
  }
  .btn {
    padding: var(--spacing-sm) var(--spacing-md);
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-primary);
    cursor: pointer;
  }
  .btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  footer {
    padding: var(--spacing-lg);
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: flex-end;
  }
  .hint {
    font-size: var(--font-size-sm);
    color: var(--text-muted);
    margin: var(--spacing-sm) 0;
    line-height: 1.4;
  }
  .hint kbd {
    font-size: 0.9em;
    padding: 1px 5px;
    border-radius: 3px;
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    font-family: inherit;
  }
  .row-btns {
    display: flex;
    flex-wrap: wrap;
    gap: var(--spacing-sm);
  }
  .btn.danger {
    border-color: rgba(217, 74, 122, 0.5);
    color: #e07090;
  }
  .embed-stat {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    margin: var(--spacing-sm) 0;
  }
  .chk {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    cursor: pointer;
  }
</style>
