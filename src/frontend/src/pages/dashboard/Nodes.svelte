<script lang="ts">
	import { onMount } from 'svelte';
	import { nodesStore, type Node } from '../../lib/stores/nodesStore.svelte';

	let deleteDialog: HTMLDialogElement;

	let selectedNode = $state<string | null>(null);

	let newNodename = $state('');
	let newPassword = $state('');

	let deleteNodename = $state('');

	onMount(() => {
		nodesStore.fetchNodes();
	});

	function openDeleteDialog(nodename: string) {
		selectedNode = nodename;
		deleteNodename = nodename;
		deleteDialog.showModal();
	}

	async function handleAdd(event: SubmitEvent) {
		try {
			if ((<HTMLButtonElement | null>event.submitter)?.value !== 'cancel') {
				await nodesStore.addNode(newNodename, newPassword);
			}
		} finally {
			(<HTMLFormElement>event.target).reset();
		}
	}

	async function handleDelete(event: SubmitEvent) {
		if ((<HTMLButtonElement | null>event.submitter)?.value === 'cancel') return;
		if (selectedNode) {
			await nodesStore.deleteNode(selectedNode);
		}
	}
</script>

<div class="flex-1 p-4">
	<h2 class="text-2xl font-bold mb-4">Nodes</h2>
	<div class="mb-4 flex gap-4">
		<button class="btn btn-primary" command="show-modal" commandfor="add-node-dialog"
			>Add Node</button
		>
	</div>
	{#if nodesStore.loading}
		<p>Loading...</p>
	{:else if nodesStore.error}
		<p class="text-error">{nodesStore.error}</p>
	{:else}
		<div class="grid gap-4">
			{#each nodesStore.nodes as node}
				<div class="card bg-base-100 shadow-md p-4">
					<h2 class="card-title">{node.nodename}</h2>
					<div class="card-actions justify-end">
						<button class="btn btn-error" onclick={() => openDeleteDialog(node.nodename)}>
							Delete
						</button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Add Node Dialog -->
<dialog class="modal" id="add-node-dialog">
	<div class="modal-box">
		<h3 class="font-bold text-lg">Add Node</h3>
		<form onsubmit={handleAdd} method="dialog" class="fieldset p-4">
			<label for="nodename" class="label">Nodename</label>
			<input id="nodename" type="text" class="input" bind:value={newNodename} required />

			<label for="password" class="label">Password</label>
			<input id="password" type="password" class="input" bind:value={newPassword} required />

			<div class="modal-action">
				<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
					Cancel
				</button>
				<button class="btn btn-primary" type="submit">Add</button>
			</div>
		</form>
	</div>
</dialog>

<!-- Delete Node Dialog -->
<dialog class="modal" bind:this={deleteDialog} id="delete-node-dialog">
	<form onsubmit={handleDelete} method="dialog" class="modal-box">
		<h3 class="font-bold text-lg">Delete Node</h3>
		<p>Are you sure you want to delete node "{selectedNode}"?</p>
		<div class="modal-action">
			<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
				Cancel
			</button>
			<button type="submit" class="btn btn-error">Delete</button>
		</div>
	</form>
</dialog>
