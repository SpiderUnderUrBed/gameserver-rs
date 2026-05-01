<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';
	import { httpClient } from '../../lib/utils/http';
	import { message } from 'valibot';
	import { auth } from '../../lib/auth/auth.svelte';
	import { get, writable } from 'svelte/store';
	import { statusStore, statusType } from '../../lib/stores/serverConsoleStore.svelte';
	let changeStatusDialog: HTMLDialogElement;

	let selectedStatusType: 'node-status' | 'server-keyword' | 'server-process' | 'manual-click' = $state('manual-click');
	//let StatusType = $state('manual-click');

	let changeStatus = async () => {
		//console.log("Changing status");
		try {
			statusType.set(selectedStatusType);
			await httpClient.post(`/api/setsettings`, {
				json: {
					message: {
						status_type: selectedStatusType
					},
					type: "",
					authcode: ""
				}
			})

		} catch (e) {
			console.error(e);
		}
	}
	//let status = $state('unknown');
	// statusStore.subscribe((new_status) => {
	// 	console.log(new_status);
	// 	//status = new_status;
	// 	// if (selectedStatusType == "manual-click"){
	// 	// 	status = new_status;
	// 	// }
	// });
	const {
		...props
	}: HTMLAttributes<HTMLDivElement> = $props();
	//const statusStore = writable<'up' | 'down' | 'unknown'>('unknown');
</script>

<button onclick={() => changeStatusDialog.show()} class={['inline-grid *:[grid-area:1/1]', props.class]}>
	{#if $statusStore == 'up'}
		<div class="status status-lg status-success animate-bounce"></div>
	{:else if $statusStore === 'down'}
		<div class="status status-lg status-error animate-ping"></div>
		<div class="status status-lg status-error"></div>
	{:else}
		<div class="status status-lg"></div>
	{/if}
</button>

<dialog id="change-status" bind:this={changeStatusDialog} class="modal">
	<form onsubmit={changeStatus} method="dialog">
		<div class="modal-action">
			<div class="p-4 fieldset">
					<label for="selected_status">Selected Status:</label>
					<select id="selected_status" bind:value={selectedStatusType}>
						<option value="node-status">Node status</option>
						<option value="server-keyword">Server keyword</option>
						<option value="server-process">Server process</option>
						<option value="manual-click">Clicked manually</option>
					</select>
			</div>
		</div>
		<br>
		<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
			Cancel
		</button>
		<button class="btn btn-primary" type="submit">Save</button>
	</form>
</dialog>