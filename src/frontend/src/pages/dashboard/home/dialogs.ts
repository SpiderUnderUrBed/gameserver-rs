import { writable } from "svelte/store";

export const showNodeDialog = writable(false);
export const showServerDialog = writable(false);
export const showNodeHealthDialog = writable(false);