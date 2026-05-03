<script lang="ts">
	// import { RouterView } from 'cross-router-svelte';
	// import { router } from './router';
	import { DashboardRoot } from './pages/dashboard/DashboardRoot.svelte';
	import ThemeProvider from './lib/theme/ThemeProvider.svelte';
</script>

<ThemeProvider>
	<!-- <RouterView {router} /> -->
	<DashboardRoot></DashboardRoot>
</ThemeProvider>
