<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';
	import { httpClient } from '../../lib/utils/http';
	import { message } from 'valibot';
	import { auth } from '../../lib/auth/auth.svelte';
	let changeStatusDialog: HTMLDialogElement;

	let selectedStatusType = $state('');

	let changeStatus = async () => {
		//console.log("Changing status");
		try {
			await httpClient.post(`/api/setsettings`, {
				json: {
					message: {
						//status_type: this.final_status
					},
					type: "",
					authcode: ""
				}
			})

		} catch (e) {
			console.error(e);
		}
	}

	const {
		status,
		...props
	}: HTMLAttributes<HTMLDivElement> & { status: 'up' | 'down' | 'unknown' } = $props();
</script>

<div role="button" tabindex="0" onclick={() => changeStatusDialog.show()} class={['inline-grid *:[grid-area:1/1] cursor-pointer', props.class]}>
	{#if status == 'up'}
		<div class="status status-lg status-success animate-bounce"></div>
	{:else if status === 'down'}
		<div class="status status-lg status-error animate-ping"></div>
		<div class="status status-lg status-error"></div>
	{:else}
		<div class="status status-lg"></div>
	{/if}
</div>

<dialog id="change-status" bind:this={changeStatusDialog} class="modal">
	<form onsubmit={changeStatus} method="dialog">
		<div class="modal-action">
			<div class="p-4 fieldset">
					<label for="selected_status">Selected Status:</label>
					<select id="selected_status" bind:value={selectedStatusType}>
						<option value="node-status">Node status</option>
						<option selected value="server-keyword">Server keyword</option>
						<option value="server-process">Server process</option>
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