<script lang="ts">
	import { type Snippet } from 'svelte';
	import TopmostBar from '../../components/dashboard/TopmostBar.svelte';
	import { onMount } from 'svelte';
	import { serverConsole, type GetCurrentNodeResponse } from '../../lib/stores/serverConsoleStore.svelte';
	import { Toaster, toast } from 'svelte-sonner';
	import { httpClient } from '../../lib/utils/http';
	import { showNodeDialog, showServerDialog } from './home/dialogs';
	
	let selectNodeReminder = async () => {
		let found_node = false;
		try {
			const node = await httpClient.get<GetCurrentNodeResponse>(`/api/getcurrentnode`, {}).json();
			found_node = true;
		} catch (e) {
			found_node = false;
		}
		if (!found_node){
			toast('No current node was selected (required)', {
					action: {
						label: 'Select a node',
						onClick: () => showNodeDialog.set(true)
					}
				});	
		}
	}
	let selectServerReminder = async () => {
		let found_server = false;
		try {
			await serverConsole.fetchCurrentServer()
			if (serverConsole.selectedServer){
				found_server = true;
			}
		} catch (e) {
			found_server = false;
		}
		if (!found_server){
			toast('No current server was selected (required)', {
					action: {
						label: 'Select a server',
						onClick: () => showServerDialog.set(true)
					}
				});	
		}
	}

	onMount(() => {
		const metaTag = document.querySelector('meta[name="site-url"]');
		const basePath = metaTag?.getAttribute('content')?.replace(/\/$/, '') ?? '';
		serverConsole.init(basePath);
		selectServerReminder();
		selectNodeReminder();
	});

	const { outlet }: { outlet?: Snippet } = $props();
</script>

<div class="content-grid">
	<TopmostBar />

	{@render outlet?.()}
</div>


<style>
	.content-grid {
		padding: 0.8rem;
		display: flex;
		flex-direction: column;
		gap: 0.8rem;
	}
</style>

