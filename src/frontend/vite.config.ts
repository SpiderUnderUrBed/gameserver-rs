import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

// https://vite.dev/config/
export default defineConfig({
	plugins: [svelte(), tailwindcss()],
	server: {
		proxy: {
			'^/api/.*': {
				target: 'http://localhost:8083',
				ws: true
			}
		}
	},
	build: {
		rolldownOptions: {
			output: {
				codeSplitting: {
					groups: [
						{
							// Extract chartjs into its own bundle to avoid embedding it everywhere
							name(moduleId) {
								if (moduleId.includes('chart.js')) {
									return 'chartjs';
								}
								return null;
							}
						}
					]
				}
			}
		}
	}
});
