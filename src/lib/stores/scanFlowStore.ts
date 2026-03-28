import { writable } from "svelte/store";

/**
 * When true, the next `scan:complete` event should chain into `rebuildTextEmbeddings`
 * (used after adding a scan folder from Settings).
 */
export const autoEmbedAfterNextScanComplete = writable(false);
