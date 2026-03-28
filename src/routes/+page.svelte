<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { get } from "svelte/store";
  import { listen } from "@tauri-apps/api/event";
  import { isTauri } from "@tauri-apps/api/core";
  import type { Asset, FolderNode, ScanProgress, ScanStats, TagWithCount, EmbedRebuildProgress, EmbedRebuildComplete } from "$lib/types";
  import * as ipc from "$lib/ipc";
  import { searchStore } from "$lib/stores/searchStore";
  import { settingsStore } from "$lib/stores/settingsStore";
  import { getAudioExportCopyArgs, playerStore } from "$lib/stores/playerStore";
  import { postProcessStore } from "$lib/stores/postProcessStore";
  import { toastStore } from "$lib/stores/toastStore";
  import { autoEmbedAfterNextScanComplete } from "$lib/stores/scanFlowStore";
  import { bumpEmbeddingIndexEpoch } from "$lib/stores/embeddingStatusStore";
  import SearchBar from "$lib/components/SearchBar.svelte";
  import FilterBar from "$lib/components/FilterBar.svelte";
  import ResultsList from "$lib/components/ResultsList.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import PlayerBar from "$lib/components/PlayerBar.svelte";
  import ToastContainer from "$lib/components/ToastContainer.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import ProgressModal from "$lib/components/ProgressModal.svelte";

  let settingsOpen = $state(false);
  let scanModalOpen = $state(false);
  let scanProgress = $state<ScanProgress | null>(null);
  let scanDoneStats = $state<ScanStats | null>(null);
  let embedProgress = $state<EmbedRebuildProgress | null>(null);
  let embedDoneStats = $state<EmbedRebuildComplete | null>(null);

  let tree = $state<FolderNode[]>([]);
  let tags = $state<TagWithCount[]>([]);

  let browseAssets = $state<Asset[]>([]);
  /** Folder listing (browse API) when a tree folder is selected and the search field is empty; otherwise show search results. */
  let useFolderBrowse = $derived(selectedPath != null && $searchStore.query.text.trim() === "");
  /** Total assets under the selected folder (recursive); null before first successful count. */
  let folderTotal = $state<number | null>(null);
  let folderLoadingMore = $state(false);
  let selectedPath = $state<string | null>(null);
  let selectedId = $state<number | null>(null);
  let selectedIds = $state<number[]>([]);

  let batchTag = $state("");

  let displayed = $derived(useFolderBrowse ? browseAssets : $searchStore.results);
  let displayedTotal = $derived(
    useFolderBrowse ? (folderTotal ?? browseAssets.length) : $searchStore.total,
  );
  let browseHasMore = $derived(
    useFolderBrowse
      ? folderTotal != null && browseAssets.length < folderTotal
      : displayed.length < $searchStore.total,
  );

  const unsubs: Array<() => void> = [];

  function batchTargetIds(): number[] {
    if (selectedIds.length) return selectedIds;
    if (selectedId != null) return [selectedId];
    return [];
  }

  async function refreshSidebar() {
    try {
      tree = await ipc.getFolderTree();
      tags = await ipc.getAllTags();
    } catch (e) {
      tree = [];
      tags = [];
      toastStore.show(String(e), "error");
    }
  }

  async function reloadBrowse() {
    try {
      if (selectedPath != null && get(searchStore).query.text.trim() === "") {
        const [assets, total] = await Promise.all([
          ipc.browseFolder(selectedPath, 500, 0),
          ipc.browseFolderCount(selectedPath),
        ]);
        browseAssets = assets;
        folderTotal = total;
      }
    } catch (e) {
      toastStore.show(String(e), "error");
    }
  }

  async function afterRowEdit() {
    if (selectedPath != null && get(searchStore).query.text.trim() === "") await reloadBrowse();
    else await searchStore.refresh();
    await refreshSidebar();
  }

  async function selectFolder(path: string) {
    searchStore.setFolderScope(path);
    selectedPath = path;
    selectedId = null;
    selectedIds = [];
    folderTotal = null;
    const hasSearchText = get(searchStore).query.text.trim() !== "";
    try {
      const [assets, total] = await Promise.all([
        ipc.browseFolder(path, 500, 0),
        ipc.browseFolderCount(path),
      ]);
      browseAssets = assets;
      folderTotal = total;
      if (hasSearchText) {
        await searchStore.refresh();
      }
    } catch (e) {
      browseAssets = [];
      folderTotal = null;
      toastStore.show(String(e), "error");
    }
  }

  function allLibrary() {
    selectedPath = null;
    browseAssets = [];
    folderTotal = null;
    selectedId = null;
    selectedIds = [];
    searchStore.setFolderScope(null);
    void searchStore.refresh();
  }

  function onTagClick(name: string) {
    allLibrary();
    const cur = get(searchStore).query.tags ?? [];
    if (!cur.includes(name)) {
      searchStore.setFilter("tags", [...cur, name]);
    }
  }

  async function selectAsset(a: Asset, e: MouseEvent) {
    if (e.ctrlKey || e.metaKey) {
      selectedIds = selectedIds.includes(a.id)
        ? selectedIds.filter((x) => x !== a.id)
        : [...selectedIds, a.id];
      selectedId = a.id;
      return;
    }
    selectedIds = [];
    selectedId = a.id;
    if (get(settingsStore).playback.auto_play_on_select) {
      await playerStore.playAsset(a);
    }
  }

  async function applyBatchTag() {
    const ids = batchTargetIds();
    const tag = batchTag.trim();
    if (!ids.length || !tag) {
      toastStore.show("Select rows (Ctrl+click) or one row, then enter a tag", "info");
      return;
    }
    try {
      await ipc.bulkAddTag(ids, tag);
      batchTag = "";
      await afterRowEdit();
    } catch (e) {
      toastStore.show(String(e), "error");
    }
  }

  async function applyBatchFavorite(fav: boolean) {
    const ids = batchTargetIds();
    if (!ids.length) return;
    try {
      await ipc.bulkSetFavorite(ids, fav);
      await afterRowEdit();
    } catch (e) {
      toastStore.show(String(e), "error");
    }
  }

  async function applyBatchRating(r: number) {
    const ids = batchTargetIds();
    if (!ids.length) return;
    try {
      await ipc.bulkSetRating(ids, r);
      await afterRowEdit();
    } catch (e) {
      toastStore.show(String(e), "error");
    }
  }

  async function rescan() {
    const roots = get(settingsStore).general.scan_roots;
    if (!roots.length) {
      toastStore.show("Add a folder in Settings first", "info");
      settingsOpen = true;
      return;
    }
    try {
      await settingsStore.save();
    } catch (e) {
      toastStore.show(String(e), "error");
      return;
    }
    autoEmbedAfterNextScanComplete.set(false);
    scanDoneStats = null;
    scanProgress = null;
    embedProgress = null;
    embedDoneStats = null;
    scanModalOpen = true;
    try {
      await ipc.startScan();
    } catch (e) {
      scanModalOpen = false;
      toastStore.show(String(e), "error");
    }
  }

  /** After saving a new scan root from Settings: scan then auto-rebuild embeddings. */
  async function startScanAfterFolderAdded() {
    autoEmbedAfterNextScanComplete.set(true);
    scanDoneStats = null;
    scanProgress = null;
    embedProgress = null;
    embedDoneStats = null;
    scanModalOpen = true;
    try {
      await ipc.startScan();
    } catch (e) {
      autoEmbedAfterNextScanComplete.set(false);
      scanModalOpen = false;
      toastStore.show(String(e), "error");
    }
  }

  async function loadMore() {
    if (!useFolderBrowse) {
      searchStore.nextPage();
      return;
    }
    if (selectedPath == null || folderTotal == null) return;
    if (browseAssets.length >= folderTotal || folderLoadingMore) return;
    folderLoadingMore = true;
    try {
      const more = await ipc.browseFolder(selectedPath, 500, browseAssets.length);
      browseAssets = [...browseAssets, ...more];
    } catch (e) {
      toastStore.show(String(e), "error");
    } finally {
      folderLoadingMore = false;
    }
  }

  function focusMainSearch() {
    document.getElementById("main-search")?.focus();
  }

  function onKeyDown(e: KeyboardEvent) {
    const el = e.target as HTMLElement | null;
    if (el?.closest("input, textarea, [contenteditable=true]")) {
      if (e.key === "Escape") settingsOpen = false;
      return;
    }
    if (
      e.key === "F3" ||
      (e.key === "/" && !e.ctrlKey && !e.metaKey && !e.altKey) ||
      ((e.ctrlKey || e.metaKey) && !e.altKey && (e.key === "k" || e.key === "K"))
    ) {
      e.preventDefault();
      focusMainSearch();
      return;
    }
    if (e.key === "?" || e.key === "F1") {
      e.preventDefault();
      toastStore.show(
        "Shortcuts: Ctrl/⌘+K · F3 · / focus search · F1 · ? help · Space play/pause · ←/→ seek · i/o notch clip start left/right · Shift+i Shift+o notch end left/right (step in Settings) · Enter play · Ctrl/⌘+click multi-select · drag row for file path · Shift-drag waveform for clip · Ctrl/⌘+C copy (Tauri)",
        "info",
        9000,
      );
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key === "c" && isTauri()) {
      const args = getAudioExportCopyArgs();
      const a = get(playerStore).currentAsset;
      if (args && a) {
        e.preventDefault();
        void ipc
          .copyClipWavToClipboard(
            a.id,
            args.isClip,
            args.startSec,
            args.endSec,
            get(postProcessStore),
          )
          .then(() =>
            toastStore.show(
              args.isClip
                ? "Clip copied — paste as a file (keep this app open until you paste on Linux)"
                : "Track copied — paste as a file (keep this app open until you paste on Linux)",
              "success",
              6000,
            ),
          )
          .catch((err) => toastStore.show(String(err), "error"));
        return;
      }
    }
    if (e.key === " " || e.code === "Space") {
      e.preventDefault();
      void playerStore.toggle();
      return;
    }
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      const t = get(playerStore).currentTime - 5;
      void playerStore.seek(Math.max(0, t));
      return;
    }
    if (e.key === "ArrowRight") {
      e.preventDefault();
      const t = get(playerStore).currentTime + 5;
      const d = get(playerStore).duration;
      void playerStore.seek(Math.min(d, t));
      return;
    }
    if (!e.ctrlKey && !e.metaKey && !e.altKey) {
      const k = e.key;
      if (k === "i" || k === "I") {
        e.preventDefault();
        if (e.shiftKey) void playerStore.notchClipEndIn();
        else void playerStore.notchClipStartOut();
        return;
      }
      if (k === "o" || k === "O") {
        e.preventDefault();
        if (e.shiftKey) void playerStore.notchClipEndOut();
        else void playerStore.notchClipStartIn();
        return;
      }
    }
    if (e.key === "Enter" && selectedId != null) {
      const a = displayed.find((x) => x.id === selectedId);
      if (a) void playerStore.playAsset(a);
    }
    if (e.key === "Escape") settingsOpen = false;
  }

  onMount(() => {
    void (async () => {
      try {
        await settingsStore.load();
        playerStore.syncLoopFromConfig();
        await searchStore.applyDefaultSearchModeFromConfig(get(settingsStore));
        if (get(settingsStore).general.scan_roots.length === 0) settingsOpen = true;
        await refreshSidebar();
        await searchStore.refresh();
      } catch (e) {
        toastStore.show(String(e), "error");
      }

      try {
        unsubs.push(
          await listen<ScanProgress>("scan:progress", (ev) => {
            scanProgress = ev.payload;
            scanModalOpen = true;
          }),
        );
        unsubs.push(
          await listen<ScanStats>("scan:complete", (ev) => {
            scanProgress = null;
            if (get(autoEmbedAfterNextScanComplete)) {
              autoEmbedAfterNextScanComplete.set(false);
              scanDoneStats = null;
              embedDoneStats = null;
              embedProgress = { processed: 0, total: 1, detail: "" };
              scanModalOpen = true;
              void searchStore.refresh();
              void refreshSidebar();
              void ipc.rebuildTextEmbeddings().catch((err) => {
                toastStore.show(String(err), "error");
                scanModalOpen = false;
                embedProgress = null;
              });
              return;
            }
            scanDoneStats = ev.payload;
            setTimeout(() => {
              scanModalOpen = false;
              scanDoneStats = null;
              void searchStore.refresh();
              void refreshSidebar();
            }, 1400);
          }),
        );
        unsubs.push(
          await listen<EmbedRebuildProgress>(ipc.EVT_EMBED_PROGRESS, (ev) => {
            embedProgress = ev.payload;
            embedDoneStats = null;
            scanModalOpen = true;
            scanProgress = null;
            scanDoneStats = null;
          }),
        );
        unsubs.push(
          await listen<EmbedRebuildComplete>(ipc.EVT_EMBED_COMPLETE, (ev) => {
            embedDoneStats = ev.payload;
            embedProgress = null;
            bumpEmbeddingIndexEpoch();
            setTimeout(() => {
              scanModalOpen = false;
              embedDoneStats = null;
              void searchStore.refresh();
              void refreshSidebar();
            }, 1200);
          }),
        );
        unsubs.push(
          await listen<Record<string, unknown>>(ipc.EVT_LIBRARY_FILES_CHANGED, () => {
            toastStore.show("Files changed under a watched folder. Rescan when you are ready.", "info", 6000);
          }),
        );
      } catch {
        /* not running inside Tauri */
      }
    })();
  });

  onDestroy(() => {
    unsubs.forEach((u) => u());
  });
</script>

<svelte:window onkeydown={onKeyDown} />

<div class="app">
  <header class="top">
    <div class="search-wrap">
      <SearchBar />
    </div>
    <button type="button" class="gear" title="Open settings" onclick={() => (settingsOpen = true)} aria-label="Settings">⚙</button>
  </header>

  <div class="body">
    <Sidebar {tree} {tags} {selectedPath} onFolderSelect={selectFolder} onAllLibrary={allLibrary} onTagClick={onTagClick} />
    <main class="content">
      <FilterBar />
      {#if batchTargetIds().length}
        <div class="batch-bar">
          <span class="batch-label">{batchTargetIds().length} selected</span>
          <input class="batch-inp" type="text" placeholder="Tag…" title="Enter a tag to apply to the selected files" bind:value={batchTag} />
          <button type="button" class="bbtn" title="Apply this tag to the selected files" onclick={() => void applyBatchTag()}>Tag</button>
          <button type="button" class="bbtn" title="Mark selected files as favorites" onclick={() => void applyBatchFavorite(true)}>★</button>
          <button type="button" class="bbtn" title="Remove favorite from selected files" onclick={() => void applyBatchFavorite(false)}>☆</button>
          <span class="batch-rate">Rate:</span>
          {#each [1, 2, 3, 4, 5] as r}
            <button type="button" class="bbtn sm" title={`Rate selected files ${r} star${r > 1 ? "s" : ""}`} onclick={() => void applyBatchRating(r)}>{r}</button>
          {/each}
        </div>
      {/if}
      <div class="meta">
        {#if $searchStore.loading && !useFolderBrowse}
          <span>Loading…</span>
        {:else}
          <span>{displayed.length} shown · {displayedTotal} total</span>
          {#if useFolderBrowse && folderLoadingMore}
            <span class="meta-more"> · Loading more…</span>
          {/if}
        {/if}
      </div>
      <ResultsList
        assets={displayed}
        relevanceScores={useFolderBrowse ? null : $searchStore.relevanceScores}
        searchScoreMode={useFolderBrowse ? null : ($searchStore.query.searchMode ?? "lexical")}
        selectedId={selectedId}
        multiSelectedIds={selectedIds}
        onselect={selectAsset}
        onplay={(a) => playerStore.playAsset(a)}
        onDataChange={afterRowEdit}
        infiniteScroll={browseHasMore}
        loadingMore={$searchStore.loading || folderLoadingMore}
        onLoadMore={loadMore}
      />
    </main>
  </div>

  <PlayerBar />
  <ToastContainer />
  <SettingsPanel
    bind:open={settingsOpen}
    onStartScanAfterFolderAdded={startScanAfterFolderAdded}
    onRescan={rescan}
    onLibraryWiped={async () => {
      await refreshSidebar();
      selectedPath = null;
      browseAssets = [];
      folderTotal = null;
      selectedId = null;
      selectedIds = [];
      searchStore.setFolderScope(null);
      void searchStore.refresh();
    }}
  />
  <ProgressModal
    bind:open={scanModalOpen}
    progress={scanProgress}
    doneStats={scanDoneStats}
    embedProgress={embedProgress}
    embedDoneStats={embedDoneStats}
  />
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    min-height: 0;
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--spacing-lg);
    padding: var(--spacing-md) var(--spacing-lg);
    border-bottom: 1px solid var(--border);
    background: var(--bg-surface);
    flex-shrink: 0;
  }
  .search-wrap {
    flex: 1;
    min-width: 0;
  }
  .gear {
    flex-shrink: 0;
    width: 40px;
    height: 40px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    color: var(--text-primary);
    cursor: pointer;
    font-size: 18px;
  }
  .gear:hover {
    border-color: var(--accent);
  }
  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: var(--spacing-md) var(--spacing-lg);
    gap: var(--spacing-sm);
    min-height: 0;
  }
  .batch-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--spacing-sm);
    padding: var(--spacing-sm) var(--spacing-md);
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 8px;
    font-size: var(--font-size-sm);
  }
  .batch-label {
    color: var(--text-muted);
    margin-right: var(--spacing-sm);
  }
  .batch-inp {
    min-width: 120px;
    padding: 4px 8px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--bg-input);
    color: var(--text-primary);
  }
  .batch-rate {
    color: var(--text-muted);
    margin-left: var(--spacing-sm);
  }
  .bbtn {
    padding: 4px 10px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-secondary);
    cursor: pointer;
  }
  .bbtn.sm {
    padding: 2px 8px;
    font-size: 11px;
  }
  .meta {
    font-size: var(--font-size-sm);
    color: var(--text-muted);
  }
  .meta-more {
    opacity: 0.9;
  }
</style>
