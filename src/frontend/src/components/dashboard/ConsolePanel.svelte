<script lang="ts">
    import { tick } from 'svelte';
    import { serverConsole, scrollHeight, newScrollHeightEvent } from '../../lib/stores/serverConsoleStore.svelte';
    
    let scrollContainer: HTMLDivElement;
    let scrollY = $state(0);
    let oldScrollHeight = $state(0);
    let wasNearBottom = $state(true);
    let entries: HTMLPreElement[] = [];

    function onScroll() {
        scrollY = scrollContainer.scrollTop;
        const distanceFromBottom = scrollContainer.scrollHeight - (scrollContainer.scrollTop + scrollContainer.clientHeight);
        wasNearBottom = distanceFromBottom <= 100;
    }

    newScrollHeightEvent.subscribe(([overrideHeight, newScrollHeight]) => {
        if (oldScrollHeight !== 0 && oldScrollHeight !== newScrollHeight && scrollContainer) {
            if (wasNearBottom || overrideHeight) {
                scrollContainer.scrollTop = scrollContainer.scrollHeight;
            }
        }
        oldScrollHeight = newScrollHeight;
    });

    // serverConsole.duplicateAmount.subscribe((amount) => {
    //     duplicateCount = amount;
    // });

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
    <div class="bg-neutral rounded-box relative flex flex-col min-h-0 flex-1 p-2 font-mono text-sm" id="consoleHistory">
        <div onscroll={onScroll} bind:this={scrollContainer} class="overflow-y-auto overflow-x-hidden flex-1 min-h-0">
            {#each serverConsole.consoleHistory as entry, i}
                <pre bind:this={entries[i]} class={[{ 'text-info': entry.type !== 'input' }, "text-wrap wrap-break-word m-0 p-0"]}><code>{entry.text}{entry.count > 0 ? ` (x${entry.count + 1})` : ''}</code></pre>
            {/each}
        </div>
    </div>
</main>