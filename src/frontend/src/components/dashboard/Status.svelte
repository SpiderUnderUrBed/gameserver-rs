<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';

	const {
		status,
		...props
	}: HTMLAttributes<HTMLDivElement> & { status: 'up' | 'down' | 'unknown' } = $props();
</script>

<div {...props} class={['inline-grid *:[grid-area:1/1]', props.class]}>
	{#if status == 'up'}
		<div class="status status-lg status-success animate-bounce"></div>
	{:else if status === 'down'}
		<div class="status status-lg status-error animate-ping"></div>
		<div class="status status-lg status-error"></div>
	{:else}
		<div class="status status-lg"></div>
	{/if}
</div>
