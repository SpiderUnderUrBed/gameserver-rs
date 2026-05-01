<script lang="ts">
	import { type Settings, SettingsStore } from '../../../lib/stores/settingsStore.svelte';
	let settingsStore = new SettingsStore();
	let enabledNodesOnHomepage = $state<boolean | undefined> (false);
	let enabledStatsOnHomepage = $state<boolean | undefined>(false);

	
	(async () => {
		await settingsStore.refreshSettings();
		enabledNodesOnHomepage = settingsStore.currentSettings?.enable_nodes_on_home_page;
		enabledStatsOnHomepage = settingsStore.currentSettings?.enable_statistics_on_home_page;
	})();

	let trySettings = async () => {
		let newSettings: Settings = {
			enable_nodes_on_home_page: enabledNodesOnHomepage ?? false,
			enable_statistics_on_home_page: enabledStatsOnHomepage ?? false
		}
		settingsStore.changeSettings(newSettings)
	}
	let saveSettings = async () => {
		let newSettings: Settings = {
			enable_nodes_on_home_page: enabledNodesOnHomepage ?? false,
			enable_statistics_on_home_page: enabledStatsOnHomepage ?? false
		}
		settingsStore.changeSettings(newSettings)
		settingsStore.syncSettings()
	}

</script>

<div class="flex-1 p-4">
	<h2 class="text-2xl font-bold mb-4">Settings</h2>
	<div class="card bg-base-100 shadow-md p-4">
		<div>Enable nodes on homepage?</div>
		<div class="flex items-center w-32">
			<button class="btn btn-primary" class:btn-ghost={enabledNodesOnHomepage} onclick={() => enabledNodesOnHomepage = true}>Enable</button>
			<button class="btn btn-error" class:btn-ghost={!enabledNodesOnHomepage} onclick={() => enabledNodesOnHomepage = false}>disable</button>
		</div>
		<div>Enable statistics on homepage?</div>
		<div class="flex items-center w-32">
			<button class="btn btn-primary" class:btn-ghost={enabledStatsOnHomepage} onclick={() => enabledStatsOnHomepage = true}>Enable</button>
			<button class="btn btn-error" class:btn-ghost={!enabledStatsOnHomepage} onclick={() => enabledStatsOnHomepage = false}>disable</button>
		</div>
	</div>
	<div class="card flex flex-row items-center w-32 gap-2 bg-base-100 shadow-md p-4">
		<button class="btn btn-primary" onclick={trySettings}>Try</button>
		<button class="btn btn-primary" onclick={saveSettings}>Save</button>
	</div>
</div>
