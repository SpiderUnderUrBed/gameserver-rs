<script lang="ts">
	import { RouterView } from 'cross-router-svelte';
	import { router } from './router';
	import ThemeProvider from './lib/theme/ThemeProvider.svelte';
</script>

<ThemeProvider>
	<RouterView {router} />
</ThemeProvider>
