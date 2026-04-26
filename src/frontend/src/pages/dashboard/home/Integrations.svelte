<script lang="ts">
	import { onMount } from 'svelte';
	import {
		integrationsStore,
		type Integration
	} from '../../../lib/stores/integrationsStore.svelte';

	onMount(() => {
		integrationsStore.fetchIntegrations();
	});

	function getIntegration(type: string): Integration | undefined {
		return integrationsStore.integrations.find((i) => i.type === type);
	}

	async function toggleIntegration(type: string) {
		await integrationsStore.createIntegration(type, 'enabled', {});
	}

	async function toggleIntegrationOnHome(type: string) {
		const integration = getIntegration(type);
		if (!integration) return;

		const newStatus = integration.status === 'enabled' ? 'disabled' : 'enabled';
		const newSettings = {
			...integration.settings,
			enable_test: !integration.settings.enable_test,
			_enable_test_hook: {
				kind: 'MinecraftEnableRcon'
			}
		};

		await integrationsStore.modifyIntegration(type, newStatus, newSettings);
	}
</script>

<div class="p-4">
	<h2 class="text-2xl font-bold mb-4">Integrations</h2>
	{#if integrationsStore.loading}
		<p>Loading...</p>
	{:else if integrationsStore.error}
		<p class="text-red-500">{integrationsStore.error}</p>
	{:else}
		<div class="space-y-4">
			{#each integrationsStore.integrations as integration}
				<div class="card bg-base-100 shadow-md p-4" data-integration={integration.type}>
					<h3 class="card-title">{integration.type} Integration</h3>
					<div class="card-body">
						<div class="flex gap-4">
							<button
								class={['btn', integration.status === 'enabled' ? 'btn-success' : 'btn-outline']}
								onclick={() => toggleIntegration(integration.type)}
							>
								Enable Integration
							</button>
							<button
								class={['btn', integration.status === 'enabled' ? 'btn-success' : 'btn-error']}
								onclick={() => toggleIntegrationOnHome(integration.type)}
							>
								Allow on Home Page
							</button>
						</div>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
