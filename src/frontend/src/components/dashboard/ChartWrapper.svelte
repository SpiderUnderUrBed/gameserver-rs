<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import {
		CategoryScale,
		Chart,
		LinearScale,
		LineController,
		LineElement,
		PointElement,
		Filler,
		Tooltip,
		type ChartConfiguration
	} from 'chart.js';

	interface ChartWrapperProps extends HTMLAttributes<HTMLCanvasElement> {
		config: ChartConfiguration; // Chart.js config object
		onChartReady?: (chart: Chart) => void;
	}

	let { config, onChartReady, ...rest }: ChartWrapperProps = $props();

	let canvas: HTMLCanvasElement;
	let chart: Chart;

	Chart.register(
		LineController,
		LineElement,
		PointElement,
		LinearScale,
		CategoryScale,
		Filler,
		Tooltip
	);

	onMount(() => {
		chart = new Chart(canvas, config);
		if (onChartReady) onChartReady(chart);
	});

	onDestroy(() => {
		if (chart) {
			chart.destroy();
		}
	});
</script>

<canvas bind:this={canvas} {...rest}></canvas>
