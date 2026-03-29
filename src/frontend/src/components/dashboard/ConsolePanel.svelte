<script lang="ts">
	import { serverConsole } from '../../lib/serverConsoleStore.svelte';
</script>

<main class="console-main w-full h-full flex flex-col">
	<div
		class="mockup-code relative flex flex-col min-h-0 flex-1 px-2 max-h-[80vh]"
		id="consoleHistory"
	>
		<div class="overflow-y-auto overflow-x-hidden flex-1">
			{#each serverConsole.consoleHistory as entry, idx}
				<pre class={{ 'text-info': entry.type !== 'input' }}><code>{entry.text}</code></pre>
			{/each}
		</div>
	</div>
</main>
