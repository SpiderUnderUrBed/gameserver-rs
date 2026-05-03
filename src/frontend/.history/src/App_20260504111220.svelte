<script lang="ts">
	// import { RouterView } from 'cross-router-svelte';
	// import { router } from './router';
	import { DashboardRootRoot } from './pages/dashboard/DashboardRoot.svelte';
	import ThemeProvider from './lib/theme/ThemeProvider.svelte';
</script>

<ThemeProvider>
	<!-- <RouterView {router} /> -->
	<DashboardRootRoot></DashboardRootRoot>
</ThemeProvider>
