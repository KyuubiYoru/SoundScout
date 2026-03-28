import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  Asset,
  FilterOptions,
  FolderNode,
  PostProcessConfig,
  SearchQuery,
  SearchResults,
  SemanticSearchStatus,
  Tag,
  TagWithCount,
} from "$lib/types";

export async function search(query: SearchQuery): Promise<SearchResults> {
  return invoke("search", { query });
}

export async function getFilterOptions(): Promise<FilterOptions> {
  return invoke("get_filter_options");
}

export async function browseFolder(folder: string, limit: number, offset: number): Promise<Asset[]> {
  return invoke("browse_folder", { folder, limit, offset });
}

export async function getFolderTree(): Promise<FolderNode[]> {
  return invoke("get_folder_tree");
}

/** Temp file path + metadata; load bytes via `fetch(convertFileSrc(path))` (avoids huge IPC). */
export interface AudioPcmFile {
  path: string;
  sampleRate: number;
  channels: number;
}

export async function getAudioPcmFile(assetId: number): Promise<AudioPcmFile> {
  return invoke("get_audio_pcm_file", { assetId });
}

/** Post-processed PCM (export pipeline) for in-app preview. */
export async function getProcessedPcmFile(
  assetId: number,
  isClip: boolean,
  startSec: number,
  endSec: number,
  postProcess: PostProcessConfig,
): Promise<AudioPcmFile> {
  return invoke("get_processed_pcm_file", {
    assetId,
    isClip,
    startSec,
    endSec,
    postProcess,
  });
}

/** Event: further PCM chunks after [`startPcmStream`]. */
export const EVT_PCM_STREAM_CHUNK = "pcm-stream-chunk";
/** Event: decode finished (no more chunk files). */
export const EVT_PCM_STREAM_FINISHED = "pcm-stream-finished";

export interface PcmStreamChunkPayload {
  streamId: number;
  sampleRate: number;
  channels: number;
  chunkIndex: number;
  path: string;
}

export interface PcmStreamFinishedPayload {
  streamId: number;
}

export interface PcmStreamStart {
  streamId: number;
  sampleRate: number;
  channels: number;
  durationSec: number;
  firstChunkPath: string;
  firstChunkIndex: number;
}

export async function startPcmStream(assetId: number): Promise<PcmStreamStart> {
  return invoke("start_pcm_stream", { assetId });
}

export async function cancelPcmStream(): Promise<void> {
  return invoke("cancel_pcm_stream");
}

/** Raw little-endian `f32` PCM (prefer [`getAudioPcmFile`] for large assets). */
export async function getAudioData(assetId: number): Promise<Uint8Array> {
  const raw = await invoke<ArrayBuffer | Uint8Array | number[]>("get_audio_data", { assetId });
  if (raw instanceof ArrayBuffer) return new Uint8Array(raw);
  if (raw instanceof Uint8Array) return raw;
  return new Uint8Array(raw);
}

export async function getPeaks(assetId: number): Promise<number[]> {
  return invoke("get_peaks", { assetId });
}

/** Save dialog; returns path if saved, `null` if cancelled. `isClip`: time range vs full file. */
export async function exportClipWav(
  assetId: number,
  isClip: boolean,
  startSec: number,
  endSec: number,
  postProcess: PostProcessConfig,
): Promise<string | null> {
  return invoke("export_clip_wav", { assetId, isClip, startSec, endSec, postProcess });
}

/** Temp WAV on disk + system clipboard file list (paste in Explorer/Finder; Linux varies). */
export async function copyClipWavToClipboard(
  assetId: number,
  isClip: boolean,
  startSec: number,
  endSec: number,
  postProcess: PostProcessConfig,
): Promise<void> {
  return invoke("copy_clip_wav_to_clipboard", { assetId, isClip, startSec, endSec, postProcess });
}

export async function startScan(): Promise<void> {
  return invoke("start_scan");
}

export async function cancelScan(): Promise<void> {
  return invoke("cancel_scan");
}

export const EVT_EMBED_PROGRESS = "embed:progress";
export const EVT_EMBED_COMPLETE = "embed:complete";

export async function toggleFavorite(assetId: number): Promise<boolean> {
  return invoke("toggle_favorite", { assetId });
}

export async function setRating(assetId: number, rating: number): Promise<void> {
  return invoke("set_rating", { assetId, rating });
}

export async function addTag(assetId: number, tagName: string): Promise<void> {
  return invoke("add_tag", { assetId, tagName });
}

export async function removeTag(assetId: number, tagId: number): Promise<void> {
  return invoke("remove_tag", { assetId, tagId });
}

export async function getTags(assetId: number): Promise<Tag[]> {
  return invoke("get_tags", { assetId });
}

export async function getAllTags(): Promise<TagWithCount[]> {
  return invoke("get_all_tags");
}

export async function getConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

export async function updateConfig(config: AppConfig): Promise<void> {
  return invoke("update_config", { config });
}

export async function pickDirectory(): Promise<string | null> {
  return invoke("pick_directory");
}

export const EVT_LIBRARY_FILES_CHANGED = "library-files-changed";

export async function bulkAddTag(assetIds: number[], tagName: string): Promise<void> {
  return invoke("bulk_add_tag", { assetIds, tagName });
}

export async function bulkSetFavorite(assetIds: number[], favorite: boolean): Promise<void> {
  return invoke("bulk_set_favorite", { assetIds, favorite });
}

export async function bulkSetRating(assetIds: number[], rating: number): Promise<void> {
  return invoke("bulk_set_rating", { assetIds, rating });
}

export async function getSemanticSearchStatus(): Promise<SemanticSearchStatus> {
  return invoke("get_semantic_search_status");
}

/** Build or refresh dense text embeddings (metadata + tags) for all assets. Returns number of rows written. */
export async function rebuildTextEmbeddings(): Promise<number> {
  return invoke("rebuild_text_embeddings");
}

export async function getSimilarAssets(assetId: number, limit: number): Promise<Asset[]> {
  return invoke("get_similar_assets", { assetId, limit });
}

export async function getAutoCategorySuggestions(assetId: number): Promise<string[]> {
  return invoke("get_auto_category_suggestions", { assetId });
}

export async function exportDatabase(): Promise<string | null> {
  return invoke("export_database");
}

/** Replaces DB and restarts the app on success. */
export async function importDatabase(): Promise<void> {
  return invoke("import_database");
}

/** Removes all indexed assets, tags, and embeddings from the library DB. Does not delete audio files or scan roots. Native confirm runs in the app; resolves to true if cleared, false if cancelled. */
export async function wipeLibraryDatabase(): Promise<boolean> {
  return invoke<boolean>("wipe_library_database");
}
