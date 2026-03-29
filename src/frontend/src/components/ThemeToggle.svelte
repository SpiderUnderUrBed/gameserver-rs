<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import { MoonIcon, SunIcon } from '@lucide/svelte';
	import { useTheme } from '../lib/theme/theme.svelte';

	const theme = useTheme();

	let { class: className, ...props }: HTMLButtonAttributes = $props();
</script>

<button
	class={['btn btn-ghost btn-square', className]}
	onclick={() => theme.toggleTheme()}
	data-tip="Toggle Theme"
	{...props}
>
	{#if theme.current === 'dark'}
		<MoonIcon class="size-4" />
		<span class="sr-only">Toggle mode - dark</span>
	{:else}
		<SunIcon class="size-4" />
		<span class="sr-only">Toggle mode - light</span>
	{/if}
</button>
