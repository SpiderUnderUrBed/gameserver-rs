<script lang="ts">
	import { serverConsole } from '../../../lib/stores/serverConsoleStore.svelte';
	import ConsolePanel from '../../../components/dashboard/ConsolePanel.svelte';
	import TopBar from '../../../components/dashboard/TopBar.svelte';

	let serverName = $state('');
	let serverLocation = $state('');
	let serverProvider = $state('minecraft');
	let serverSandbox = $state(true);
	let nodeName = $state('');
	let nodeIp = $state('');
	let nodeType = $state('Custom');

	const submitCreateServer = async (event: SubmitEvent) => {
		if ((<HTMLButtonElement | null>event.submitter)?.value !== 'cancel') {
			await serverConsole.createDefaultServer(
				serverName,
				serverProvider,
				serverLocation,
				serverSandbox
			);
		}
		(<HTMLFormElement>event.target).reset();
	};

	const submitAddNode = async (event: SubmitEvent) => {
		if ((<HTMLButtonElement | null>event.submitter)?.value === 'cancel') return;
		await serverConsole.addNode(nodeName, nodeIp, nodeType);
		nodeName = '';
		nodeIp = '';
		nodeType = 'Custom';
	};
</script>

<TopBar />
{#if serverConsole.nodePanelVisible}
	<section class="alert alert-info flex flex-col gap-1 items-start">
		<h4 class="font-semibold">Nodes</h4>
		<ul class="list list-disc">
			{#each serverConsole.nodes as node}
				<li>
					<button class="btn btn-link" onclick={() => (serverConsole.selectedNode = node.nodename)}>
						{node.nodename}
					</button>
				</li>
			{:else}
				<p class="italic px-2">No nodes</p>
			{/each}
		</ul>
	</section>
{/if}

<ConsolePanel />

<dialog id="create-server-dialog" class="modal">
	<form onsubmit={submitCreateServer} method="dialog" class="modal-box">
		<h3 class="text-lg font-bold">Create Server</h3>

		<div class="p-4 fieldset">
			<label for="server_type" class="label">Server Type</label>
			<select id="server_type" bind:value={serverProvider} class="select">
				<option value="custom">Custom</option>
				<option value="minecraft">Vanilla Minecraft</option>
			</select>

			<label for="server_name" class="label">Server Name</label>
			<input
				type="text"
				class="input"
				id="server_name"
				placeholder="Server name"
				bind:value={serverName}
			/>

			<label for="server_location" class="label">Server Location</label>
			<input
				id="server_location"
				class="input"
				type="text"
				placeholder="Server location"
				bind:value={serverLocation}
			/>

			<label class="label mt-4">
				<input type="checkbox" class="checkbox" bind:checked={serverSandbox} /> Enable sandbox?
			</label>
		</div>

		<div class="modal-action">
			<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
				Cancel
			</button>
			<button class="btn btn-primary" type="submit">Server</button>
		</div>
	</form>
</dialog>

<dialog id="configure-server-dialog" class="modal">
	<form
		onsubmit={(event) => {
			if ((<HTMLButtonElement | null>event.submitter)?.value === 'cancel') return;
			serverConsole.addConsoleEntry({
				type: 'output',
				text: 'Configure server placeholder'
			});
		}}
		method="dialog"
		class="modal-box"
	>
		<h3 class="font-semibold text-lg">Configure Server</h3>

		<div class="p-4">
			<p>Selected server: {serverConsole.selectedNode ?? 'none'}</p>
		</div>

		<div class="modal-action">
			<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
				Cancel
			</button>
			<button class="btn btn-primary" type="submit">Confirm</button>
		</div>
	</form>
</dialog>

<dialog id="add-node-dialog" class="modal">
	<form onsubmit={submitAddNode} method="dialog" class="modal-box">
		<h3 class="font-semibold text-lg">Add Node</h3>

		<div class="p-4 fieldset">
			<label class="label" for="node_name">Node name</label>
			<input class="input" type="text" bind:value={nodeName} id="node_name" />

			<label class="label" for="node_ip">Node IP</label>
			<input class="input" type="text" bind:value={nodeIp} id="node_ip" />

			<label class="label" for="node_type"> Node type </label>
			<select bind:value={nodeType} class="select" id="node_type">
				<option value="Main">Initial gameserver</option>
				<option value="Custom">Custom</option>
			</select>
		</div>

		<div class="modal-action">
			<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
				Cancel
			</button>
			<button class="btn btn-primary" type="submit">Confirm</button>
		</div>
	</form>
</dialog>

<dialog id="delete-server-dialog" class="modal">
	<form
		onsubmit={(event) => {
			if ((<HTMLButtonElement | null>event.submitter)?.value === 'cancel') return;
			serverConsole.addConsoleEntry({
				type: 'output',
				text: 'Delete server request executed'
			});
		}}
		method="dialog"
		class="modal-box"
	>
		<h3 class="font-semibold text-lg">Delete Server</h3>

		<div class="p-4">
			<p>WARNING: This will remove all server files</p>
		</div>

		<div class="modal-action">
			<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
				Cancel
			</button>
			<button class="btn btn-primary" type="submit">I am sure</button>
		</div>
	</form>
</dialog>
