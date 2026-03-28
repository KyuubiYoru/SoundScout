import type { SearchMode } from "./search";
export * from "./audio";
export * from "./asset";
export * from "./search";
export * from "./player";
export * from "./scan";

/** Mirrors `AppConfig` JSON from Tauri (`snake_case` keys). */
export interface AppConfig {
  general: { scan_roots: string[] };
  indexing: {
    parallel_workers: number;
    peak_resolution: number;
    skip_hidden_dirs: boolean;
    watch_scan_roots: boolean;
  };
  playback: {
    buffer_cache_count: number;
    auto_play_on_select: boolean;
    loop_playback: boolean;
  };
  search: {
    default_sort: string;
    results_per_page: number;
    /** @deprecated Ignored; use per-query search mode in the UI. */
    semantic_search?: boolean;
    /** Default mode when no `localStorage` choice exists (`lexical` \| `vector` \| `both`). */
    default_search_mode?: SearchMode;
  };
}

export interface SemanticSearchStatus {
  embeddingCount: number;
  assetCount: number;
  semanticEnabled: boolean;
  clapPipelineReady: boolean;
}
