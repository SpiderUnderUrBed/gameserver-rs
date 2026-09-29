<script lang="ts">
	import { type Snippet } from 'svelte';
	import TopmostBar from '../../components/dashboard/TopmostBar.svelte';
	import { onMount } from 'svelte';
	import { serverConsole, type GetCurrentNodeResponse } from '../../lib/stores/serverConsoleStore.svelte';
	import { toast } from 'svelte-sonner';
	import { httpClient } from '../../lib/utils/http';
	import { shouldNotifyOnce } from '../../lib/utils/sessionNotification';
	import { showNodeDialog, showServerDialog } from './home/dialogs';

	let { outlet }: { outlet?: Snippet } = $props();

	let selectNodeReminder = async () => {
		let found_node = false;
		try {
			await httpClient.get<GetCurrentNodeResponse>(`/api/getcurrentnode`, {}).json();
			found_node = true;
		} catch (e) {
			found_node = false;
		}
		if (!found_node && shouldNotifyOnce('session-warning:no-current-node')) {
			toast('No current node was selected (required)', {
				action: {
					label: 'Select a node',
					onClick: () => showNodeDialog.set(true)
				}
			});
		}
	};

	let selectServerReminder = async () => {
		let found_server = false;
		try {
			await serverConsole.fetchCurrentServer();
			if (serverConsole.selectedServer) {
				found_server = true;
			}
		} catch (e) {
			found_server = false;
		}
		if (!found_server && shouldNotifyOnce('session-warning:no-current-server')) {
			toast('No current server was selected (required)', {
				action: {
					label: 'Select a server',
					onClick: () => showServerDialog.set(true)
				}
			});
		}
	};

	onMount(async () => {
		if (window.location.pathname.startsWith('/auth')) {
			return;
		}

		const metaTag = document.querySelector('meta[name="site-url"]');
		const basePath = metaTag?.getAttribute('content')?.replace(/\/$/, '') ?? '';
		serverConsole.init(basePath);
		await Promise.allSettled([
			serverConsole.fetchCurrentServer(),
			serverConsole.getCurrentNode()
		]);
		await Promise.allSettled([
			selectServerReminder(),
			selectNodeReminder()
		]);
	});
</script>

<div class="content-grid">
	<TopmostBar />
	<div class="outlet-wrapper">
		{@render outlet?.()}
	</div>
</div>


<style>
.content-grid {
    padding: 0.8rem;
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
    height: 100vh;
    overflow: hidden;
}
.outlet-wrapper {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
}
</style>