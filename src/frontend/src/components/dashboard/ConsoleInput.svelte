<script lang="ts">
	import { SlashIcon } from '@lucide/svelte';

    import { serverConsole } from '../../lib/stores/serverConsoleStore.svelte';

	let commandInput = $state('');

	function sendCommand() {
		if (!commandInput.trim()) return;
		serverConsole.sendConsoleCommand(commandInput);
		commandInput = '';
	}
</script>

<div>
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