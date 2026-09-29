<script lang="ts">
	import { navigate } from 'svelte5-router';
	import { toast } from 'svelte-sonner';
	import { auth } from '../lib/auth/auth.svelte';
	import type { Snippet } from 'svelte';

	let { children }: { children?: Snippet } = $props();

	$effect(() => {
		if (!auth.loggedIn) {
			toast.dismiss();
			navigate('/auth/login');
		}
	});
</script>

{#if auth.loggedIn}
	{@render children?.()}
{/if}
