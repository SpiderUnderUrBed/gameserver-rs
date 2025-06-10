import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default {
  kit: {
    adapter: adapter(),
    output: {
      bundleStrategy: 'inline' 
    },
    prerender: {
      entries: ['*']
    },
    paths: {
    }
  },
  preprocess: vitePreprocess()
};
