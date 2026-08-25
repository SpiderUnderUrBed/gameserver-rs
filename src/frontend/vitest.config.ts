import { defineConfig } from 'vitest/config'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { playwright } from '@vitest/browser-playwright'
import process from 'node:process'

const execPath = process.env.PLAYWRIGHT_LAUNCH_OPTIONS_EXECUTABLE_PATH

export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      '^/api/.*': {
        target: 'http://localhost:8083',
        ws: true
      }
    }
  },
  test: {
    browser: {
      enabled: true,
      provider: playwright({
        launchOptions: {
          executablePath: execPath,
        },
      }),
      instances: [{ browser: 'chromium' }],
    },
  },
})