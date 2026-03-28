import type { Asset } from "./asset";

export type SortField = "relevance" | "name" | "duration" | "date";
export type SortDirection = "asc" | "desc";

/** Matches Rust `SearchMode` (camelCase in IPC). */
export type SearchMode = "lexical" | "vector" | "both";

export interface SearchQuery {
  text: string;
  searchMode?: SearchMode;
  extensions?: string[] | null;
  durationMin?: number | null;
  durationMax?: number | null;
  sampleRates?: number[] | null;
  channels?: number | null;
  favoritesOnly: boolean;
  tags?: string[] | null;
  publisher?: string | null;
  /** When set, limit search to this folder and subfolders (recursive). */
  folderRoot?: string | null;
  sortBy: SortField;
  sortDir: SortDirection;
  offset: number;
  limit: number;
}

export interface SearchResults {
  assets: Asset[];
  total: number;
  offset: number;
  /** Present for vector / hybrid search pages; same length as `assets` for that page. */
  relevanceScores?: number[] | null;
}

export interface FilterOptions {
  extensions: string[];
  sampleRates: number[];
  minDurationMs: number;
  maxDurationMs: number;
  publishers: string[];
}
