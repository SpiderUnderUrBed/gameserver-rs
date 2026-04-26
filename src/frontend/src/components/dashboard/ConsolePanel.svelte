<script lang="ts">
	import { tick } from 'svelte';
	import { serverConsole, scrollHeight, newScrollHeightEvent } from '../../lib/stores/serverConsoleStore.svelte';

	let scrollContainer: HTMLDivElement;
	let scrollY = $state(0);
	let oldScrollHeight = $state(0);
	let wasNearBottom = $state(true);

	function onScroll() {
		scrollY = scrollContainer.scrollTop;
		
		const distanceFromBottom = scrollContainer.scrollHeight - (scrollContainer.scrollTop + scrollContainer.clientHeight);
		wasNearBottom = distanceFromBottom <= 100;
	}

	newScrollHeightEvent.subscribe(([overrideHeight, newScrollHeight]) => {
		if (oldScrollHeight !== 0 && oldScrollHeight !== newScrollHeight && scrollContainer) {
			
			if (wasNearBottom || overrideHeight) {
				scrollContainer.scrollTop = scrollContainer.scrollHeight;
			} else {
				const heightIncrease = newScrollHeight - oldScrollHeight;
				scrollContainer.scrollTop = scrollY + heightIncrease;
			}
		}
		oldScrollHeight = newScrollHeight;
	});

	$effect(() => {
		serverConsole.consoleHistory;

		(async () => {
			await tick(); 
			if (!scrollContainer) return;
			scrollHeight.set([scrollContainer.scrollHeight, scrollY]);
		})();
	});
</script>

<main class="console-main w-full h-full flex flex-col">
	<div
		class="mockup-code relative flex flex-col min-h-0 flex-1 px-2 max-h-[80vh]"
		id="consoleHistory"
	>
		<div onscroll={onScroll} bind:this={scrollContainer} class="overflow-y-auto overflow-x-hidden flex-1">
			{#each serverConsole.consoleHistory as entry, idx}
				<pre class={{ 'text-info': entry.type !== 'input' }}><code>{entry.text}</code></pre>
			{/each}
		</div>
	</div>
</main>
