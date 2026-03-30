<script lang="ts">
	import { type Snippet } from 'svelte';
	import TopmostBar from '../../components/dashboard/TopmostBar.svelte';
	import { onMount } from 'svelte';
	import { serverConsole } from '../../lib/stores/serverConsoleStore.svelte';

	onMount(() => {
		const metaTag = document.querySelector('meta[name="site-url"]');
		const basePath = metaTag?.getAttribute('content')?.replace(/\/$/, '') ?? '';
		serverConsole.init(basePath);
	});

	const { outlet }: { outlet?: Snippet } = $props();
</script>

<div class="content-grid">
	<TopmostBar />

	{@render outlet?.()}
</div>

<style>
	.content-grid {
		padding: 0.8rem;
		display: flex;
		flex-direction: column;
		gap: 0.8rem;
	}
</style>
