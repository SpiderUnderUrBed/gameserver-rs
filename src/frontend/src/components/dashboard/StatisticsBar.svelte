<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import ChartWrapper from './ChartWrapper.svelte';
	import type { Chart, ChartConfiguration } from 'chart.js';
	import { StatisticsStore } from '../../lib/stores/statisticsStore.svelte';

	const statisticsStore = new StatisticsStore();

	let ramChart = $state<Chart | null>(null);
	let coreChart = $state<Chart | null>(null);

	const ramConfig = {
		type: 'line',
		data: {
			labels: [],
			datasets: [
				{
					label: 'Used Memory (GB)',
					data: [],
					borderWidth: 2,
					borderColor: 'rgba(75, 192, 192, 1)',
					backgroundColor: 'rgba(75, 192, 192, 0.2)',
					fill: true,
					tension: 0.3
				}
			]
		},
		options: {
			maintainAspectRatio: true,
			animation: {},
			responsive: true,
			scales: {
				x: {
					title: { display: true, text: 'Time' }
				},
				y: {
					beginAtZero: true,
					title: { display: true, text: 'GB' }
				}
			}
		}
	} satisfies ChartConfiguration;

	const coreConfig = {
		type: 'line',
		data: {
			labels: [],
			datasets: [
				{
					label: 'CPU Usage (Ghz)',
					data: [],
					borderWidth: 2,
					borderColor: 'rgba(75, 192, 192, 1)',
					backgroundColor: 'rgba(75, 192, 192, 0.2)',
					fill: true,
					tension: 0.3
				}			
			]
		},
		options: {
			maintainAspectRatio: true,
			animation: {},
			responsive: true,
			scales: {
				x: {
					title: { display: true, text: 'Time' }
				},
				y: {
					beginAtZero: true,
					title: { display: true, text: 'Ghz' }
				}
			}
		}
	} satisfies ChartConfiguration;

	onMount(() => {
		statisticsStore.init();
	});

	onDestroy(() => {
		statisticsStore.disconnect();
	});

	$effect(() => {
		if (!ramChart || statisticsStore.points.length === 0) return;

		const points = statisticsStore.points;
		ramChart.data.labels = points.map((p) => p.label);
		ramChart.data.datasets[0].data = points.map((p) => p.usedMemoryGB);

		const lastPoint = statisticsStore.points.at(-1);

		if (lastPoint?.totalMemoryGB != null) {
			ramChart.options.scales!.y!.max = lastPoint.totalMemoryGB;
		}
		ramChart.update('none');
	});

	$effect(() => {
		if (!coreChart || statisticsStore.points.length === 0) return;

		const points = statisticsStore.points;
		const average = points.map((p) => {
			const count = p.coreUsageHz.length;
			if (count === 0) return 0;

			const total = p.coreUsageHz.reduce(
				(acc, val) => (acc ?? 0) + (val ?? 0),
				0
			);

			return (total ?? 0) / count;
		});
		if (coreChart.data.datasets.length === 0) {
			coreChart.data.datasets.push({
				label: 'CPU Ghz (placeholder)',
				data: average,
				borderWidth: 2,
				borderColor: 'rgba(255, 99, 132, 1)',
				backgroundColor: 'rgba(255, 99, 132, 0.2)',
				fill: true,
				tension: 0.3
			});
		} else {
			coreChart.data.datasets[0].data = average;
		}
		coreChart.data.labels = points.map((p) => p.label);
		coreChart.update('none');
	});
</script>

<div
    class="grid grid-cols-1 md:grid-cols-2 items-center justify-center gap-5 w-full md:max-w-7xl mx-auto p-5"
>
    <div class="card bg-base-100 h-full">
        <div class="card-body">
            <h3 class="card-title">Memory</h3>
            <div class="flex-1 max-w-xl h-96 bg-base-200/40 rounded p-4">
                <ChartWrapper config={ramConfig} class="" onChartReady={(chart) => (ramChart = chart)} />
            </div>
        </div>
    </div>
    <div class="card bg-base-100 h-full">
        <div class="card-body">
            <h3 class="card-title">CPU (Average core usage)</h3>
            <div class="flex-1 max-w-xl h-96 bg-base-200/40 rounded p-4">
                <ChartWrapper config={coreConfig} onChartReady={(chart) => (coreChart = chart)} />
            </div>
        </div>
    </div>
</div>