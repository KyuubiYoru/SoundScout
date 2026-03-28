import { writable, get } from "svelte/store";
import type { AppConfig, SearchQuery, SearchResults, SearchMode } from "$lib/types";
import * as ipc from "$lib/ipc";

const SEARCH_MODE_KEY = "soundscout_search_mode";

function isSearchMode(v: string): v is SearchMode {
  return v === "lexical" || v === "vector" || v === "both";
}

function configDefaultMode(c: AppConfig): SearchMode {
  const m = c.search.default_search_mode;
  if (m === "lexical" || m === "vector" || m === "both") return m;
  return "lexical";
}

function persistedSearchMode(configFallback: SearchMode): SearchMode {
  if (typeof localStorage === "undefined") return configFallback;
  const raw = localStorage.getItem(SEARCH_MODE_KEY);
  if (raw && isSearchMode(raw)) return raw;
  return configFallback;
}

function persistSearchMode(mode: SearchMode) {
  if (typeof localStorage !== "undefined") localStorage.setItem(SEARCH_MODE_KEY, mode);
}

const defaultQuery = (): SearchQuery => ({
  text: "",
  searchMode: "lexical",
  favoritesOnly: false,
  sortBy: "relevance",
  sortDir: "desc",
  offset: 0,
  limit: 50,
});

const state = writable<{
  query: SearchQuery;
  results: SearchResults["assets"];
  relevanceScores: number[] | null;
  total: number;
  loading: boolean;
}>({
  query: defaultQuery(),
  results: [],
  relevanceScores: null,
  total: 0,
  loading: false,
});

async function runSearch(append: boolean) {
  const q = get(state).query;
  state.update((s) => ({ ...s, loading: true }));
  try {
    const res: SearchResults = await ipc.search(q);
    const newScores = res.relevanceScores ?? null;
    state.update((s) => {
      let relevanceScores: number[] | null;
      if (append) {
        if (newScores != null && newScores.length === res.assets.length) {
          relevanceScores =
            s.relevanceScores != null ? [...s.relevanceScores, ...newScores] : [...newScores];
        } else {
          relevanceScores = s.relevanceScores;
        }
      } else {
        relevanceScores = newScores;
      }
      return {
        ...s,
        results: append ? [...s.results, ...res.assets] : res.assets,
        relevanceScores,
        total: res.total,
        loading: false,
      };
    });
  } catch {
    state.update((s) => ({ ...s, loading: false }));
  }
}

let debounceTimer: ReturnType<typeof setTimeout> | null = null;

export const searchStore = {
  subscribe: state.subscribe,
  /** After `settingsStore.load()`, applies `default_search_mode` and `localStorage` to `query.searchMode`. Vector/Both fall back to lexical if there are no embeddings. */
  async applyDefaultSearchModeFromConfig(cfg: AppConfig) {
    let mode = persistedSearchMode(configDefaultMode(cfg));
    try {
      const s = await ipc.getSemanticSearchStatus();
      if (Number(s.embeddingCount) === 0 && (mode === "vector" || mode === "both")) {
        mode = "lexical";
        persistSearchMode(mode);
      }
    } catch {
      /* keep mode */
    }
    state.update((s) => ({ ...s, query: { ...s.query, searchMode: mode } }));
  },
  search(text: string) {
    state.update((s) => ({ ...s, query: { ...s.query, text, offset: 0 } }));
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      debounceTimer = null;
      void runSearch(false);
    }, 250);
  },
  setFilter<K extends keyof SearchQuery>(key: K, value: SearchQuery[K]) {
    state.update((s) => ({ ...s, query: { ...s.query, [key]: value, offset: 0 } }));
    if (key === "searchMode" && value != null) {
      persistSearchMode(value as SearchMode);
    }
    void runSearch(false);
  },
  nextPage() {
    state.update((s) => ({
      ...s,
      query: { ...s.query, offset: s.query.offset + s.query.limit },
    }));
    void runSearch(true);
  },
  reset() {
    state.set({
      query: defaultQuery(),
      results: [],
      relevanceScores: null,
      total: 0,
      loading: false,
    });
    void runSearch(false);
  },
  refresh() {
    state.update((s) => ({ ...s, query: { ...s.query, offset: 0 } }));
    return runSearch(false);
  },
};
