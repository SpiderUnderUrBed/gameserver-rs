<script lang="ts">
	import { serverConsole } from '../../../lib/stores/serverConsoleStore.svelte';
	import ConsolePanel from '../../../components/dashboard/ConsolePanel.svelte';
	import TopBar from '../../../components/dashboard/TopBar.svelte';

	type NodeData = {
		nodename: string;
		ip: string;
		nodetype?: string;
		nodestatus?: { kind: string; data: unknown };
	};

	let serverName = $state('');
	let serverLocation = $state('');
	let serverProvider = $state('minecraft');
	let serverSandbox = $state(true);

	let nodes: NodeData[] = $state([]);
	
	let selectedNodeName = $state('');

	let nodeName = $state('');
	let nodeIp = $state('');
	let nodeType = $state('Custom');
	let switchNodeId = $state('');

	const fetchNodes = async () => {
		serverConsole.fetchNodes().then((node_list) => {
			nodes = node_list;
		})
	}

	const submitCreateServer = async (event: SubmitEvent) => {
		if ((<HTMLButtonElement | null>event.submitter)?.value !== 'cancel') {
			await serverConsole.createDefaultServer(
				serverName,
				serverProvider,
				'', // server type: missing
				serverLocation,
				serverSandbox
			);
		}
		(<HTMLFormElement>event.target).reset();
	};
	const deleteNode = async (event: SubmitEvent) => {
		if ((<HTMLButtonElement | null>event.submitter)?.value !== 'cancel') {
			//selectedNodeName
			await serverConsole.deleteNode(selectedNodeName, nodeIp, nodeType);
			nodeName = '';
			nodeIp = '';
			nodeType = 'Custom';
			fetchNodes();
		}
	}

	const submitAddNode = async (event: SubmitEvent) => {
		if ((<HTMLButtonElement | null>event.submitter)?.value === 'cancel') return;
		await serverConsole.addNode(nodeName, nodeIp, nodeType);
		nodeName = '';
		nodeIp = '';
		nodeType = 'Custom';
		fetchNodes();
	};
	fetchNodes();
</script>

<TopBar />

<ConsolePanel />
<dialog id="delete-node-dialog" class="modal">
	<form onsubmit={deleteNode} method="dialog">
		<div class="p-4 fieldset">
			<label for="selected_node">Select node</label>
			<select bind:value={selectedNodeName} id="selected_node" class="select">
				<option selected value={nodeName}>{nodeName}</option>
				{#each nodes as node}
					{#if (node.nodename != nodeName)}
						<option value={node.nodename}>{node.nodename}</option>
					{/if}
				{/each}	
			</select>
		</div>
		<div class="modal-action">
			<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
				Cancel
			</button>
			<button class="btn btn-primary" type="submit">Delete</button>
		</div>
	</form>
</dialog>
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
		onsubmit={async (event) => {
			if ((<HTMLButtonElement | null>event.submitter)?.value === 'cancel') return;
			serverConsole.addConsoleEntry({
				type: 'output',
				text: 'Delete server request executed'
			});	
			await serverConsole.deleteServer();
			(<HTMLFormElement>event.target).reset();
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


<dialog id="switch-node-dialog" class="modal">
	<form 
		onsubmit={(event) => {
			if ((<HTMLButtonElement | null>event.submitter)?.value === 'cancel') return;
			serverConsole.changeNode(serverName, switchNodeId)
		}}
		method="dialog"
		class="modal-box"
	>
		<h3 class="font-semibold text-lg">Switch server's node</h3>

		<div class="p-4 fieldset">
			<label class="label" for="node_name">Node name</label>

			{#if serverConsole.nodes}
				<select bind:value={switchNodeId} class="select" id="node_name">
					{#each serverConsole.nodes as node}
						<option value={node.nodename} selected={serverConsole.selectedNode === node.nodename}>{node.nodename}</option>
					{/each}
				</select>
			{:else}
				<p class="italic px-2">No nodes</p>
			{/if}
		</div>

		<div class="modal-action">
			<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
				Cancel
			</button>
			<button class="btn btn-primary" type="submit">Submit</button>
		</div>
	</form>
</dialog>