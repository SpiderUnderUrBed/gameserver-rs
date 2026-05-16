<script lang="ts">
	import { serverConsole } from '../../lib/stores/serverConsoleStore.svelte';
	// import { showNodeDialog } from '../../pages/dashboard/home/dialogs';
	// import { serversStore } from '../../lib/stores/serversStore.svelte';
	import Status from './Status.svelte';
	// import StatusStore from './Status.svelte';


	
</script>

<div class="bg-base-100 flex flex-col rounded p-2 gap-2">
	<div class="flex flex-row gap-2 items-center overflow-x-auto">
		<Status
			//status={serverConsole.statusIndicator}
			aria-label="Server Status"
			data-tip="Server Status"
			class="tooltip p-4"
		/>

		<button class="btn" commandfor="create-server-dialog" command="show-modal">Create Server</button
		>
		<button class="btn" commandfor="delete-server-dialog" command="show-modal">
			Delete Server
		</button>
		<button class="btn" onclick={() => serverConsole.startServer()}>Start Server</button>
		<button class="btn" onclick={() => serverConsole.stopServer()}>Stop Server</button>
		<button class="btn" commandfor="configure-server-dialog" command="show-modal">
			Configure Server
		</button>
		<!-- TODO: consider if i want a raw output button -->
		<!-- <button class="btn raw-toggle-button" onclick={() => serverConsole.toggleRaw()}>
			Raw Output: {serverConsole.rawOutputEnabled ? 'ON' : 'OFF'}
		</button> -->
		<button class="btn" commandfor="add-node-dialog" command="show-modal">Add Node</button>
		<button class="btn"  commandfor="switch-node-dialog" command="show-modal">Switch node ({serverConsole.selectedNode ?? 'None'})</button>
		<button class="btn" commandfor="delete-node-dialog" command="show-modal">Delete node</button>
	</div>


</div>