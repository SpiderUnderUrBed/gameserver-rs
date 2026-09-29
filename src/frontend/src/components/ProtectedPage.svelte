<script lang="ts">
	import { navigate } from 'svelte5-router';
	import { auth } from '../lib/auth/auth.svelte';
	import type { Snippet } from 'svelte';

	let { children }: { children?: Snippet } = $props();

	$effect(() => {
		if (!auth.loggedIn) {
			navigate('/auth/login');
		}
	});
</script>

{@render children?.()}
