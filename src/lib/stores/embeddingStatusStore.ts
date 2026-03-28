import { writable } from "svelte/store";

/** Increment so UI (e.g. FilterBar) refetches embedding count after rebuild / wipe. */
export const embeddingIndexEpoch = writable(0);

export function bumpEmbeddingIndexEpoch() {
  embeddingIndexEpoch.update((n) => n + 1);
}
