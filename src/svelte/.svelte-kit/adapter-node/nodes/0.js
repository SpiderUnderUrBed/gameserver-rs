

export const index = 0;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/fallbacks/layout.svelte.js')).default;
export const imports = ["_app/immutable/nodes/0.D54qfmAx.js","_app/immutable/chunks/Df-Lx6fs.js","_app/immutable/chunks/DvKonxjB.js"];
export const stylesheets = [];
export const fonts = [];
