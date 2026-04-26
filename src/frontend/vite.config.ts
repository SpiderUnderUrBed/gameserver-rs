import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/postcss';

export default defineConfig({
  plugins: [svelte()],
  css: {
    postcss: {
      plugins: [tailwindcss()]
    }
  },
  server: {
    proxy: {
      '^/api/.*': {
        target: 'http://localhost:8083',
        ws: true
      }
    }
  },
  build: {
    outDir: "build/",
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              name(moduleId) {
                if (moduleId.includes('chart.js')) return 'chartjs';
                return null;
              }
            }
          ]
        }
      }
    }
  }
});