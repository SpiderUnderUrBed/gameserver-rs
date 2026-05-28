<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { type Settings, type FileSystemDriver, type Filters, SettingsStore, type FilterModifyStructure } from '../../../lib/stores/settingsStore.svelte';
	let settingsStore = new SettingsStore();
	let enabledNodesOnHomepage = $state<boolean | undefined> (false);
	let enabledStatsOnHomepage = $state<boolean | undefined>(false);
	let consoleEntryOnTop = $state<boolean | undefined>(false);
	let rconUrl = $state<string | undefined>();
	let rconPassword = $state<string | undefined>();
	let currentGlobalFilter = $state<Filters | undefined>();
	let fileSystemDriver = $state<FileSystemDriver | undefined>();
	let forceSandbox = $state<boolean>();

	let lock = $state<boolean | undefined> (false);
	let currentClientFilters = $state<Filters[]>(
		[{
			kind: "none"
		}]
	);

	let global_filters: FilterModifyStructure[] = $state([
		{
			name: "alternating line",
			kind: "alternating_line"
		},
		{
			name: "none",
			kind: "none"
		}
	]);


	let client_filter: FilterModifyStructure[] = $state([
		{
			name: "none",
			kind: "none"
		},
		{
			name: "terminal",
			kind: "terminal"
		},
		{
			name: "duplicates",
			kind: "duplicates"
		}
	]);


	(async () => {
		await settingsStore.refreshSettings();
		enabledNodesOnHomepage = settingsStore.currentSettings?.enable_nodes_on_home_page;
		enabledStatsOnHomepage = settingsStore.currentSettings?.enable_statistics_on_home_page;
		consoleEntryOnTop = settingsStore.currentSettings?.console_entry_on_top;
		rconUrl = settingsStore.currentSettings?.rcon_url;
		rconPassword = settingsStore.currentSettings?.rcon_password;
		currentGlobalFilter = settingsStore.currentSettings?.filter;
		currentClientFilters = settingsStore.currentSettings?.client_filter ?? [currentClientFilters[0]];
		forceSandbox = settingsStore.currentSettings?.force_sandbox;
	})();

	let trySettings = async () => {
		let newSettings: Settings = {
			enable_nodes_on_home_page: enabledNodesOnHomepage ?? false,
			enable_statistics_on_home_page: enabledStatsOnHomepage ?? false,
			console_entry_on_top: consoleEntryOnTop ?? false,
			force_sandbox: forceSandbox ?? false,
			file_system_driver: fileSystemDriver ?? { kind: "none" },
			filter: currentGlobalFilter ?? { kind: "none" },
			rcon_url: rconUrl ?? '',
			rcon_password: rconPassword ?? '',
			lock: false,
			client_filter: currentClientFilters ?? {
				kind: 'none'
			}
		}
		settingsStore.changeSettings(newSettings)
	}
	let saveSettings = async () => {
		let newSettings: Settings = {
			enable_nodes_on_home_page: enabledNodesOnHomepage ?? false,
			enable_statistics_on_home_page: enabledStatsOnHomepage ?? false,
			console_entry_on_top: consoleEntryOnTop ?? false,
			force_sandbox: forceSandbox ?? false,
			file_system_driver: fileSystemDriver ?? { kind: "none" },
			filter: currentGlobalFilter ?? { kind: "none" },
			rcon_url: rconUrl ?? '',
			rcon_password: rconPassword ?? '',
			lock: false,
			client_filter: currentClientFilters ?? [{
				kind: 'none'
			}]
		}

		settingsStore.changeSettings(newSettings)
		settingsStore.syncSettings()
	}
	let makeFiltersGlobal = async () => {
		for (const filter of currentClientFilters){
			if (settingsStore.filterType(filter) == "server"){
				if (global_filters.findIndex(inner_filter => inner_filter.kind == filter.kind) == -1){
					let name = client_filter.filter(inner_filter => inner_filter.kind == filter.kind)[0].name;
					global_filters = [ ...global_filters, { name, kind: filter.kind } ]
				}
			}
		}
	}

</script>

<div class="flex-1 p-4">
	<h2 class="text-2xl font-bold mb-4">Settings</h2>
	<div class="card bg-base-100 shadow-md p-4">
	    <h6 class="text-1xl font-bold mb-4">Global</h6>
		<div>Enable nodes on homepage?</div>
		<div class="flex items-center w-32">
			<button class="btn btn-primary" class:btn-ghost={!enabledNodesOnHomepage} onclick={() => enabledNodesOnHomepage = true}>Enable</button>
			<button class="btn btn-error" class:btn-ghost={enabledNodesOnHomepage} onclick={() => enabledNodesOnHomepage = false}>disable</button>
		</div>
		<div>Enable statistics on homepage?</div>
		<div class="flex items-center w-32">
			<button class="btn btn-primary" class:btn-ghost={!enabledStatsOnHomepage} onclick={() => enabledStatsOnHomepage = true}>Enable</button>
			<button class="btn btn-error" class:btn-ghost={enabledStatsOnHomepage} onclick={() => enabledStatsOnHomepage = false}>disable</button>
		</div>
		<div>Enable console entry on:</div>
		<div class="flex items-center w-32">
			<button class="btn btn-primary" class:btn-ghost={!consoleEntryOnTop} onclick={() => consoleEntryOnTop = true}>Top</button>
			<button class="btn btn-error" class:btn-ghost={consoleEntryOnTop} onclick={() => consoleEntryOnTop = false}>Bottom</button>
		</div>
		<div>Force the sandbox:</div>
		<div class="flex items-center w-32">
			<button class="btn btn-primary" class:btn-ghost={!forceSandbox} onclick={() => forceSandbox = true}>Enable</button>
			<button class="btn btn-error" class:btn-ghost={forceSandbox} onclick={() => forceSandbox = false}>Disable</button>
		</div>
		<div>Rcon settings</div>
		<div>
		<div class="flex items-center w-96">
			<input bind:value={rconUrl} type="text" placeholder="RCON Url" class="input" />
			<input bind:value={rconPassword} type="text" placeholder="RCON password" class="input" />
		</div>
		</div>
		<div>Filters</div>
		<div>
			{#each global_filters as filter}
				<button onclick={() => currentGlobalFilter = { kind: filter.kind }} 
				class="btn"
				class:btn-primary={currentGlobalFilter?.kind === filter.kind}
				class:btn-ghost={currentGlobalFilter?.kind !== filter.kind}>{filter.name}</button>
			{/each}
		</div>
	</div>
	<div class="card bg-base-100 shadow-md p-4">
		<div>
			<h6 class="text-1xl font-bold mb-4">Session</h6>
			<div>Lock</div>
			<div class="flex items-center w-32">
				<button class="btn btn-primary" class:btn-ghost={!lock} onclick={() => lock = true}>Enable</button>
				<button class="btn btn-error" class:btn-ghost={lock} onclick={() => lock = false}>Disable</button>
			</div>
		</div>
	</div>
	<div class="card bg-base-100 shadow-md p-4">
		<div>
			<h6 class="text-1xl font-bold mb-4">User</h6>
			<div>Client filters</div>
			<div>
				{#each client_filter as filter}
					<button 
						onclick={() => {
							const index = currentClientFilters?.findIndex(f => f.kind === filter.kind) ?? -1;
							if (index === -1) {
								currentClientFilters = [...(currentClientFilters ?? []), { kind: filter.kind }];
							} else {
								currentClientFilters = currentClientFilters?.filter(f => f.kind !== filter.kind);
							}
						}} 
						class="btn"
						class:btn-primary={currentClientFilters?.some(f => f.kind === filter.kind)}
						class:btn-ghost={!currentClientFilters?.some(f => f.kind === filter.kind)}
					>{filter.name}</button>
				{/each}
				<br>
				<button class="w-20 h-5 text-sm rounded-md hover:bg-primary" onclick={makeFiltersGlobal}>To global</button>
			</div>
		</div>
	</div>
	<div class="card flex flex-row items-center w-45 gap-2 bg-base-100 shadow-md p-4">
		<button class="btn btn-primary" onclick={trySettings}>Try</button>
		<button class="btn btn-primary" onclick={saveSettings}>Save</button>
	</div>
</div>
