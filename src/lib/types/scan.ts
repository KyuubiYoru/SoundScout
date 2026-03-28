export type ScanPhase = "enumerating" | "extracting" | "indexing" | "complete";

export interface ScanProgress {
  scanned: number;
  total: number;
  currentFile: string;
  phase: ScanPhase;
}

export interface ScanStats {
  filesIndexed: number;
  filesSkipped: number;
  filesMissing: number;
  errors: number;
  durationSecs: number;
}

export interface EmbedRebuildProgress {
  processed: number;
  total: number;
  detail: string;
}

export interface EmbedRebuildComplete {
  written: number;
  durationSecs: number;
}
