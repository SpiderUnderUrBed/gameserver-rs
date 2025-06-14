import staticAdapter from '@sveltejs/adapter-static';
import fs from 'fs/promises';
import path from 'path';

export default function (options = {}) {
  const baseAdapter = staticAdapter(options);

  return {
    ...baseAdapter,
    /**
     * @param {import("@sveltejs/kit").Builder} builder
     */
    async adapt(builder) {
      await baseAdapter.adapt(builder);
      
      const buildDir = 'build'; 
      const chunksDir = path.join(buildDir, '_app', 'immutable', 'chunks');
      
      const files = await fs.readdir(chunksDir);
      
      // Update all paths-*.js files
      await Promise.all(files.map(async file => {
        if (/^paths-\w+\.js$/.test(file)) {
          const filePath = path.join(chunksDir, file);
          const newContent = `
            export const base = (typeof window !== 'undefined' && window.__sveltekit_base) || '';
            export const assets = base;
          `;
          await fs.writeFile(filePath, newContent);
        }
      }));
    }
  };
};
