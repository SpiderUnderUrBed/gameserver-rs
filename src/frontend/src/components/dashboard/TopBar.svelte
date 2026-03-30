<script lang="ts">
	import { serverConsole } from '../../lib/stores/serverConsoleStore.svelte';
	import Status from './Status.svelte';
	import { SlashIcon } from '@lucide/svelte';

	let commandInput = $state('');

	function sendCommand() {
		if (!commandInput.trim()) return;
		serverConsole.sendConsoleCommand(commandInput);
		commandInput = '';
	}
</script>

<div class="bg-base-100 flex flex-col rounded p-2 gap-2">
	<div class="flex flex-row gap-2 items-center overflow-x-auto">
		<Status
			status={serverConsole.statusIndicator}
			aria-label="Server Status"
			data-tip="Server Status"
			class="tooltip p-4"
		/>

		<button class="btn" commandfor="create-server-dialog" command="show-modal">Create Server</button
		>
		<button class="btn" commandfor="delete-server-dialog" command="show-modal">
			Delete current server
		</button>
		<button class="btn" onclick={() => serverConsole.startServer()}>Start Server</button>
		<button class="btn" onclick={() => serverConsole.stopServer()}>Stop Server</button>
		<button class="btn" commandfor="configure-server-dialog" command="show-modal">
			Configure Server
		</button>
		<button class="btn raw-toggle-button" onclick={() => serverConsole.toggleRaw()}>
			Raw Output: {serverConsole.rawOutputEnabled ? 'ON' : 'OFF'}
		</button>
		<button class="btn" commandfor="add-node-dialog" command="show-modal">Add Node</button>
		<button class="btn" onclick={() => serverConsole.toggleNodes()}>Toggle Nodes</button>
	</div>

	<label class="input w-full">
		<SlashIcon class="w-3 text-base-content/70" />
		<input
			class="grow"
			placeholder="Type command and Enter"
			bind:value={commandInput}
			onkeyup={(event) => event.key === 'Enter' && sendCommand()}
		/>
	</label>
</div>
