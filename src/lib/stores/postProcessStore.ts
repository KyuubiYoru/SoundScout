import { writable } from "svelte/store";
import { defaultPostProcessConfig, type PostProcessConfig } from "$lib/types";

export const postProcessStore = writable<PostProcessConfig>(defaultPostProcessConfig);
