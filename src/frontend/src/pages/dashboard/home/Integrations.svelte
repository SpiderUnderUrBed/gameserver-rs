<script lang="ts">
	import { onMount } from 'svelte';
	import {
		integrationsStore,
		type Integration
	} from '../../../lib/stores/integrationsStore.svelte';

	onMount(() => {
		integrationsStore.fetchIntegrations();
		console.log(integrationsStore.integrations);
		for (const intergration of integrationsStore.integrations){
			default_intergrations = default_intergrations.filter((default_intergration) => default_intergration.type.kind !== intergration.type.kind);
		}
	});

	function getIntegration(type: string): Integration | undefined {
		return integrationsStore.integrations.find((i) => i.type.kind === type);

	}

	async function toggleIntegration(type: string) {
		const intergration = getIntegration(type);
		let type_wrapper = {
			kind: type
		};
		if (!intergration){
			await integrationsStore.createIntegration(type_wrapper, 'disabled', {});
			default_intergrations = default_intergrations.filter((default_intergration) => default_intergration.type.kind !== type);
		} else {
			await integrationsStore.deleteIntegration(type);
			if (!default_intergrations.includes(intergration)){
				default_intergrations.push({ type: intergration.type, status: "disabled", settings: intergration.settings});
			}
			// const newStatus = intergration.status === 'enabled' ? 'disabled' : 'enabled';
			// await integrationsStore.modifyIntegration(type_wrapper, newStatus, {})
		}
		
		console.log(default_intergrations)
	}

	async function toggleIntegrationOnHome(type: string) {
		const integration = getIntegration(type);
		if (!integration) return;

		const newSettings = {
			...integration.settings,
			enable_test: !integration.settings.enable_test,
			_enable_test_hook: {
				kind: 'MinecraftEnableRcon'
			}
		};
		let type_wrapper = {
			kind: type
		};
		const newStatus = integration.status === 'enabled' ? 'disabled' : 'enabled';
		await integrationsStore.modifyIntegration(type_wrapper, newStatus, newSettings);
		default_intergrations = default_intergrations.filter((default_intergration) => default_intergration.type.kind !== type);
	}

	// TODO: in the future dont hardcode intergrations here, have the backend fetch it from a db file or something 
	let default_intergrations: Integration[] = $state([
		{
			type: {
				kind: "minecraft"
			},
			status: 'disabled',
			settings: {}
		}
	])
	let local_intergrations: Integration[] = $state([]);

	$effect(() => {
		local_intergrations = integrationsStore.integrations
	})

</script>

<div class="p-4">
	<h2 class="text-2xl font-bold mb-4">Integrations</h2>
	{#if integrationsStore.loading}
		<p>Loading...</p>
	{:else if integrationsStore.error}
		<p class="text-red-500">{integrationsStore.error}</p>
	{:else}
		<h2>Loaded intergrations</h2>
		<div class="space-y-4">
			{#each local_intergrations as integration}
				<div class="card bg-base-100 shadow-md p-4" data-integration={integration.type.kind}>
					<h3 class="card-title">{integration.type.kind} Integration</h3>
					<div class="card-body">
						<div class="flex gap-4">
							<button
								class={['btn', integration.status === 'enabled' ? 'btn-success' : 'btn-outline']}
								onclick={() => toggleIntegration(integration.type.kind)}
							>
								Enable Integration
							</button>
							<button
								class={['btn', integration.status === 'enabled' ? 'btn-success' : 'btn-error']}
								onclick={() => toggleIntegrationOnHome(integration.type.kind)}
							>
								Allow on Home Page
							</button>
						</div>
					</div>
				</div>
			{/each}
		</div>
		<div class="divider"></div>
		<h2>
			Hardcoded intergrations
		</h2>
		<div>
			<div class="mt-4"></div>
			<div class="space-y-4">
				{@render defaultIntergrations()}
			</div>
		</div>
	{/if}
</div>

{#snippet defaultIntergrations()}
	{#each default_intergrations as integration}
		<div class="card bg-base-100 shadow-md p-4" data-integration={integration.type}>
			<h3 class="card-title">{integration.type.kind} Integration</h3>
			<div class="card-body">
				<div class="flex gap-4">
					<button
						class={['btn', integration.status === 'enabled' ? 'btn-success' : 'btn-outline']}
						onclick={() => toggleIntegration(integration.type.kind)}
					>
						Enable Integration
					</button>
					<button
						class={['btn', integration.status === 'enabled' ? 'btn-success' : 'btn-error']}
						onclick={() => toggleIntegrationOnHome(integration.type.kind)}
					>
						Allow on Home Page
					</button>
				</div>
			</div>
		</div>
	{/each}
{/snippet}