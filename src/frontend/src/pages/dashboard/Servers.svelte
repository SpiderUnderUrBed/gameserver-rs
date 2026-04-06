<script lang="ts">
    import { onMount } from 'svelte';
    import { serversStore, type Server } from '../../lib/stores/serversStore.svelte';

    let addDialog: HTMLDialogElement;
    let deleteDialog: HTMLDialogElement;
    let selectedServer = $state<string | null>(null);

    let newServername = $state('');
    let newProvider = $state('');
    let newProvidertype = $state('');
    let newLocation = $state('');
    let newSandbox = $state(false);
    let deleteServername = $state('');

    onMount(() => {
        serversStore.fetchServers();
    });

    function openDeleteDialog(servername: string) {
        selectedServer = servername;
        deleteServername = servername;
        deleteDialog.showModal();
    }

    async function handleAdd(event: SubmitEvent) {
        try {
            if ((<HTMLButtonElement | null>event.submitter)?.value !== 'cancel') {
                await serversStore.addServer(newServername, newProvider, newProvidertype, newLocation, newSandbox);
            }
        } finally {
            (<HTMLFormElement>event.target).reset();
            newSandbox = false;
        }
    }

    async function handleDelete(event: SubmitEvent) {
        if ((<HTMLButtonElement | null>event.submitter)?.value === 'cancel') return;
        if (selectedServer) {
            await serversStore.deleteServer(selectedServer);
        }
    }
</script>

<div class="flex-1 p-4">
    <h2 class="text-2xl font-bold mb-4">Servers</h2>
    <div class="mb-4 flex gap-4">
        <button class="btn btn-primary" onclick={() => addDialog.showModal()}>Add Server</button>
        <button class="btn btn-error" onclick={() => deleteDialog.showModal()}>Delete Server</button>
    </div>

    <!-- Data Column Header -->
    <div class="bg-base-200 p-4 rounded-lg mb-4 flex justify-between items-center">
        <div class="flex-1"></div>
        <h5 class="flex-3">Servername:</h5>
        <div class="flex flex-[1.3] justify-between">
            <h5>Roles:</h5>
            <h5>Extra settings:</h5>
        </div>
    </div>

    {#if serversStore.loading}
        <p>Loading...</p>
    {:else if serversStore.error}
        <p class="text-error">{serversStore.error}</p>
    {:else}
        <div class="overflow-x-auto">
            <table
                class="table table-zebra table-pin-rows table-pin-cols border border-base-content/5 bg-base-100 w-full"
            >
                <thead class="border-base-content/10">
                    <tr>
                        <th>#</th>
                        <th>Server Name</th>
                        <th>Node</th>
                        <th>Roles</th>
                        <th>Extra Settings</th>
                        <th class="text-end">Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {#each serversStore.servers as server, index}
                        <tr>
                            <th>{index + 1}</th>
                            <td>{server.servername}</td>
                            <td>-</td>
                            <td>None</td>
                            <td>—</td>
                            <td class="text-end">
                                <button
                                    class="btn btn-ghost btn-sm"
                                    onclick={() => openDeleteDialog(server.servername)}
                                >
                                    Delete
                                </button>
                            </td>
                        </tr>
                    {:else}
                        <tr>
                            <td colspan="5" class="text-center p-8 italic">No servers configured</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</div>

<!-- Add Server Dialog -->
<dialog class="modal" bind:this={addDialog}>
    <div class="modal-box">
        <h3 class="font-bold text-lg">Add Server</h3>
        <form onsubmit={handleAdd} method="dialog" class="fieldset p-4">
            <label for="servername" class="label">Server Name</label>
            <input id="servername" type="text" class="input w-full" bind:value={newServername} required />

            <label for="provider" class="label">Provider</label>
            <input id="provider" type="text" class="input w-full" bind:value={newProvider} />

            <label for="providertype" class="label">Provider Type</label>
            <input id="providertype" type="text" class="input w-full" bind:value={newProvidertype} />

            <label for="location" class="label">Location</label>
            <input id="location" type="text" class="input w-full" bind:value={newLocation} />

            <label class="label cursor-pointer justify-start gap-3 mt-2">
                <input type="checkbox" class="checkbox" bind:checked={newSandbox} />
                <span>Sandbox mode</span>
            </label>

            <div class="modal-action">
                <button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
                    Cancel
                </button>
                <button class="btn btn-primary" type="submit">Add</button>
            </div>
        </form>
    </div>
</dialog>

<!-- Delete Server Dialog -->
<dialog class="modal" bind:this={deleteDialog}>
    <form onsubmit={handleDelete} method="dialog" class="modal-box">
        <h3 class="font-bold text-lg">Delete Server</h3>
        <div class="fieldset p-4">
            <label for="delete-servername" class="label">Servername</label>
            <input type="text" class="input" bind:value={deleteServername} required />
        </div>
        <div class="modal-action">
            <button class="btn btn-ghost btn-error" type="submit" value="cancel" formnovalidate>
                Cancel
            </button>
            <button type="submit" class="btn btn-error">Delete</button>
        </div>
    </form>
</dialog>