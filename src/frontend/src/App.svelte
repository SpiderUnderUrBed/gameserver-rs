<script lang="ts">
	import { RouterView } from 'cross-router-svelte';
	import { router } from './router';
	import ThemeProvider from './lib/theme/ThemeProvider.svelte';
	import { Toaster, toast } from 'svelte-sonner';
	
</script>

<Toaster />
<ThemeProvider>
	<!-- <Notifications item={CustomNotification}> --> 
	<RouterView {router} />
	<!-- </Notifications> -->
</ThemeProvider>
