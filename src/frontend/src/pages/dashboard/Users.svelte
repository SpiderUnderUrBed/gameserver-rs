<script lang="ts">
	import { onMount } from 'svelte';
	import { userStore, type User } from '../../lib/stores/usersStore.svelte';
	import { XIcon } from '@lucide/svelte';
	import { SvelteSet } from 'svelte/reactivity';

	let editDialog: HTMLDialogElement;
	let deleteDialog: HTMLDialogElement;

	let selectedUser = $state<string | null>(null);

	let newUsername = $state('');
	let newPassword = $state('');
	let perms = new SvelteSet<string>();

	let editUsername = $state('');
	let editPassword = $state('');

	let availablePerms = $state(['server', 'createusers']);

	onMount(() => {
		userStore.fetchUsers();
	});

	function openEditDialog(user: User) {
		selectedUser = user.username;
		editUsername = user.username;
		user.user_perms.forEach((val) => perms.add(val));
		editDialog.showModal();
	}

	function openDeleteDialog(username: string) {
		selectedUser = username;
		deleteDialog.showModal();
	}

	async function handleAdd(event: SubmitEvent) {
		try {
			if ((<HTMLButtonElement | null>event.submitter)?.value !== 'cancel') {
				await userStore.createUser(newUsername, newPassword, [...perms]);
			}
		} finally {
			(<HTMLFormElement>event.currentTarget).reset();
			perms.clear();
		}
	}

	async function handleEdit(event: SubmitEvent) {
		try {
			if ((<HTMLButtonElement | null>event.submitter)?.value !== 'cancel') {
				await userStore.editUser(editUsername, editPassword || undefined, [...perms]);
			}
		} finally {
			(<HTMLFormElement>event.currentTarget).reset();
			perms.clear();
		}
	}

	async function handleDelete(event: SubmitEvent) {
		if ((<HTMLButtonElement | null>event.submitter)?.value === 'cancel') return;
		if (selectedUser) {
			await userStore.deleteUser(selectedUser);
		}
	}
</script>

<div class="flex-1 p-4">
	<h1 class="text-2xl font-bold mb-4">Users</h1>
	<button class="btn btn-primary mb-4" command="show-modal" commandfor="add-user-dialog">
		Add User
	</button>
	{#if userStore.loading}
		<p>Loading...</p>
	{:else if userStore.error}
		<p class="text-red-500">{userStore.error}</p>
	{:else}
		<div class="grid gap-4">
			{#each userStore.users as user}
				<div class="card bg-base-100 shadow-md p-4">
					<h2 class="card-title">{user.username}</h2>
					<p>Permissions: {user.user_perms.join(', ') || 'None'}</p>
					<div class="card-actions justify-end">
						<button class="btn btn-secondary" onclick={() => openEditDialog(user)}>Edit</button>
						<button class="btn btn-error" onclick={() => openDeleteDialog(user.username)}>
							Delete
						</button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Add User Dialog -->
<dialog class="modal" id="add-user-dialog">
	<div class="modal-box">
		<h3 class="font-bold text-lg">Add User</h3>
		<form onsubmit={handleAdd} method="dialog" class="fieldset p-4">
			<label for="username" class="label">Username</label>
			<input id="username" type="text" class="input" bind:value={newUsername} required />

			<label for="password" class="label">Password</label>
			<input id="password" type="password" class="input" bind:value={newPassword} required />

			<div class="fieldset">
				<label for="add-perm-select" class="label">Permissions</label>
				<div class="join">
					<select class="select join-item" id="add-perm-select">
						<option disabled selected>Select permission</option>
						{#each availablePerms as perm}
							<option value={perm}>{perm}</option>
						{/each}
					</select>
					<button
						type="button"
						class="btn join-item"
						onclick={() => {
							const select = document.getElementById('add-perm-select') as HTMLSelectElement;
							const currentOpts = select.selectedOptions[0];
							if (currentOpts?.value && !currentOpts.disabled) {
								perms.add(currentOpts.value);
							}
						}}
					>
						Add
					</button>
				</div>
				<div class="flex flex-wrap gap-2">
					{#each perms as perm}
						<button
							type="button"
							class="badge badge-primary cursor-pointer group"
							onclick={() => {
								perms.delete(perm);
							}}
						>
							<span>{perm}</span>
							<XIcon class="w-3 btn btn-sm btn-circle btn-ghost h-auto" />
						</button>
					{/each}
				</div>
			</div>
			<div class="modal-action">
				<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
					Cancel
				</button>
				<button class="btn btn-primary" type="submit">Add</button>
			</div>
		</form>
	</div>
</dialog>

<!-- Edit User Dialog -->
<dialog class="modal" id="edit-user-dialog" bind:this={editDialog}>
	<div class="modal-box">
		<h3 class="font-bold text-lg">Edit User</h3>
		<form onsubmit={handleEdit} method="dialog">
			<div class="fieldset">
				<label for="username" class="label">Username</label>
				<input type="text" id="username" class="input" bind:value={editUsername} readonly />
			</div>
			<div class="fieldset">
				<label for="new_password" class="label">New Password (leave empty to keep current)</label>
				<input type="password" class="input" id="new_password" bind:value={editPassword} />
			</div>
			<div class="fieldset">
				<label for="edit-perm-select" class="label">Permissions</label>
				<div class="join">
					<select class="select join-item" id="edit-perm-select">
						<option disabled selected>Select permission</option>
						{#each availablePerms as perm}
							<option value={perm}>{perm}</option>
						{/each}
					</select>
					<button
						type="button"
						class="btn join-item"
						onclick={() => {
							const select = document.getElementById('edit-perm-select') as HTMLSelectElement;
							const currentOpts = select.selectedOptions[0];
							if (currentOpts?.value && !currentOpts.disabled) perms.add(currentOpts.value);
						}}
					>
						Add
					</button>
				</div>
				<div class="flex flex-wrap gap-2">
					{#each perms as perm}
						<button
							type="button"
							class="badge badge-primary cursor-pointer group"
							onclick={() => {
								perms.delete(perm);
							}}
						>
							<span>{perm}</span>
							<XIcon class="w-3 btn btn-sm btn-circle btn-ghost h-auto" />
						</button>
					{/each}
				</div>
			</div>
			<div class="modal-action">
				<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
					Cancel
				</button>
				<button type="submit" class="btn btn-primary">Save</button>
			</div>
		</form>
	</div>
</dialog>

<!-- Delete User Dialog -->
<dialog class="modal" id="delete-user-dialog" bind:this={deleteDialog}>
	<form onsubmit={handleDelete} method="dialog" class="modal-box">
		<h3 class="font-bold text-lg">Delete User</h3>
		<p>Are you sure you want to delete user "{selectedUser}"?</p>
		<div class="modal-action">
			<button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
				Cancel
			</button>
			<button type="submit" class="btn btn-error">Delete</button>
		</div>
	</form>
</dialog>
