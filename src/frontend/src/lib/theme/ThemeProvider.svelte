<script lang="ts">
	import { setThemeContext } from './theme.svelte';
	let { children } = $props();

	// Initialize the theme state for this tree
	setThemeContext();
</script>

{@render children()}
