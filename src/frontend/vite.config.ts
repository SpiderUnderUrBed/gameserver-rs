import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';
import glob from 'tiny-glob';
import path from 'path';
import fs from 'fs';
import * as cheerio from 'cheerio';
import { viteStaticCopy } from 'vite-plugin-static-copy';
import inlineEverythingPlugin from 'vite-plugin-inline-multipage';

function watchExtraDirs(dirs: string[]): Plugin {
  return {
    name: 'watch-extra-assets',
    async buildStart() {
      for (const dir of dirs) {
        const resolved = path.resolve(__dirname, dir);
        if (fs.existsSync(resolved)) {
          const files = fs.readdirSync(resolved);
          for (const file of files) {
            this.addWatchFile(path.join(resolved, file));
          }
        } else {
          console.warn(`[watch-extra-assets] Directory does not exist: ${resolved}`);
        }
      }
    }
  };
}

export default defineConfig({
  base: './',
  plugins: [
    sveltekit(),
    watchExtraDirs(
      ["extra-assets"]
    ),
    viteStaticCopy({
      targets: [
        {
          src: 'extra-assets/*',
          dest: ''
        }
      ]
    }),
    inlineEverythingPlugin()
  ]
});