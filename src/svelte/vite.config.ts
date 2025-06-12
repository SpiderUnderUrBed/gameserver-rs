import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';
import glob from 'tiny-glob';
import path from 'path';
import fs from 'fs';
import * as cheerio from 'cheerio';
import { viteStaticCopy } from 'vite-plugin-static-copy';
import inlineEverythingPlugin from 'vite-plugin-inline-multipage';


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