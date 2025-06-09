// import adapter from '@sveltejs/adapter-static';
import customAdapter from './path-adaptor.js'; 
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
// svelte.config.js
// import adapter from '@sveltejs/adapter-static';
// import { vitePreprocess } from '@sveltejs/kit/vite';

export default {
  kit: {
    adapter: customAdapter(),
    paths: {
      base: '',       // Critical for relative paths
      assets: ''      // Ensures assets use relative paths
    }
  },
  preprocess: vitePreprocess()
};