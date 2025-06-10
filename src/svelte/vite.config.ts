import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';
import glob from 'tiny-glob';
import path from 'path';
import fs from 'fs';
import * as cheerio from 'cheerio';
import { viteStaticCopy } from 'vite-plugin-static-copy';

function inlineEverythingPlugin(): Plugin {
  return {
    name: 'inline-everything',
    apply: 'build',
    closeBundle: async () => {
      const buildDir = path.resolve(process.cwd(), 'build');
      const htmlFiles = await glob('**/*.html', { 
        cwd: buildDir, 
        absolute: true 
      });

      
      const assetFiles = await glob('**/_app/immutable/**/*.{css,js}', {
        cwd: buildDir,
        absolute: true
      });

      
      const assetMap = new Map();
      assetFiles.forEach(file => {
        assetMap.set(path.basename(file), file);
      });

      for (const htmlFile of htmlFiles) {
        let html = fs.readFileSync(htmlFile, 'utf8');
        const loaded_html = cheerio.load(html);

        
        loaded_html('link[rel="stylesheet"]').each((i, el) => {
          const href = loaded_html(el).attr('href');
          if (href) {
            const filename = path.basename(href);
            if (assetMap.has(filename)) {
              try {
                const cssContent = fs.readFileSync(assetMap.get(filename), 'utf8');
                loaded_html(el).replaceWith(`<style>${cssContent}</style>`);
              } catch (e) {
                console.warn(`Failed to inline CSS ${filename}:`, e.message);
              }
            }
          }
        });


        loaded_html('script[type="module"][src]').each((i, el) => {
          const src = loaded_html(el).attr('src');
          if (src) {
            const filename = path.basename(src);
            if (assetMap.has(filename)) {
              try {
                const jsContent = fs.readFileSync(assetMap.get(filename), 'utf8');
                loaded_html(el).replaceWith(`<script type="module">${jsContent}</script>`);
              } catch (e) {
                console.warn(`Failed to inline JS ${filename}:`, e.message);
              }
            }
          }
        });

        
        loaded_html('link[rel="modulepreload"]').remove();
        loaded_html('link[rel="preload"]').remove();
        loaded_html('script[data-sveltekit-hydrate]').remove();

        fs.writeFileSync(htmlFile, loaded_html.html());
      }

      
      try {
        fs.rmSync(path.join(buildDir, '_app'), { recursive: true });
      } catch (e) {
        console.warn('Could not clean up _app directory:', e.message);
      }
    }
  };
}

export default defineConfig({
  base: './',
  plugins: [
    sveltekit(),
    viteStaticCopy({
      targets: [
        {
          src: 'static/*',
          dest: ''
        }
      ]
    }),
    inlineEverythingPlugin()
  ]
});