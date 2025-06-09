<script>
	import { onMount } from 'svelte';

	let addUserDialog;
	let deleteUserDialog;
	let createUsername = '';
	let deleteUsername = '';
	let password = '';
	let message = '';
	let users = [];

	const basePath = document
		.querySelector('meta[name="site-url"]')
		.content.replace(/\/$/, '');

	async function fetchUsers() {
		try {
			const response = await fetch(`${basePath}/api/users`);
			if (response.ok) {
				const json_data = await response.json();
				users = json_data.list.data;
				message = '';
			} else {
				console.error('Failed to get users from the server');
				message = 'Failed to get users from the server.';
			}
		} catch (error) {
			console.error('Error fetching users:', error);
			message = 'Error connecting to the server.';
		}
	}

	async function addUser(event) {
		event.preventDefault();

		const user = createUsername;
		const authcode = '0';

		try {
			const response = await fetch(`${basePath}/api/createuser`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ user, password, authcode })
			});

			if (!response.ok) {
				const error = await response.text();
				console.error('Server error:', error);
				alert('Failed to create user.');
			} else {
				alert('User created successfully!');
				fetchUsers();
				addUserDialog.close();
			}
		} catch (err) {
			console.error('Request failed:', err);
			alert('An error occurred while creating the user.');
		}
	}

	async function deleteUser(event) {
		event.preventDefault();

		const user = deleteUsername;
		const authcode = '0';

		try {
			const response = await fetch(`${basePath}/api/deleteuser`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ user, authcode })
			});

			if (!response.ok) {
				const error = await response.text();
				console.error('Server error:', error);
				alert('Failed to delete user.');
			} else {
				alert('User deleted successfully!');
				fetchUsers();
				deleteUserDialog.close();
			}
		} catch (err) {
			console.error('Request failed:', err);
			alert('An error occurred while deleting the user.');
		}
	}

	onMount(fetchUsers);
</script>

<link rel="stylesheet" href="/main.css" />

<div class="user-whole">
	<div class="user-center">
		<div class="user-background">
			<div class="user-operations">
				<button on:click={() => addUserDialog.showModal()}>Add User</button>
				<button on:click={() => deleteUserDialog.showModal()}>Delete User</button>
			</div>

			<div class="users">
				{#each users as user}
					<button class="users-element" on:click={() => alert(`User clicked: ${user.username}`)}>
						{user.username}
					</button>
				{/each}
			</div>

			<div id="message">{message}</div>
		</div>

		<dialog bind:this={deleteUserDialog} class="user-dialog">
			<form method="dialog" on:submit|preventDefault={deleteUser}>
				<h3>Delete User</h3>
				<label for="delete-username">Username:</label><br />
				<input type="text" id="delete-username" bind:value={deleteUsername} required /><br />

				<div class="dialog-buttons">
					<button type="submit">Delete User</button>
					<button type="button" on:click={() => deleteUserDialog.close()}>Cancel</button>
				</div>
			</form>
		</dialog>

		<dialog bind:this={addUserDialog} class="user-dialog">
			<form method="dialog" on:submit|preventDefault={addUser}>
				<h3>Add User</h3>
				<label for="create-username">Username:</label><br />
				<input type="text" id="create-username" bind:value={createUsername} required /><br />

				<label for="password">Password:</label><br />
				<input type="password" id="password" bind:value={password} required /><br />

				<div class="dialog-buttons">
					<button type="submit">Add User</button>
					<button type="button" on:click={() => addUserDialog.close()}>Cancel</button>
				</div>
			</form>
		</dialog>
	</div>
</div>
