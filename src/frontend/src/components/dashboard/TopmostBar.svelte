<script lang="ts">
	import { Link } from 'cross-router-svelte';
	import { serverConsole } from '../../lib/serverConsoleStore.svelte';
</script>

<ul class="menu menu-horizontal rounded bg-base-100 w-full">
	<li><Link href="/filebrowser" activeClass="menu-active">Filebrowser</Link></li>
	<li><Link href="/statistics" activeClass="menu-active">Statistics</Link></li>
	<li><Link href="/workflows" activeClass="menu-active">Workflows</Link></li>
	<li><Link href="/intergrations" activeClass="menu-active">Integrations</Link></li>
	<li><Link href="/backups" activeClass="menu-active">Backups</Link></li>
	<li><Link href="/settings" activeClass="menu-active">Settings</Link></li>

	<li><button onclick={() => serverConsole.toggleManagementButtons()}>Manage buttons</button></li>
	{#if serverConsole.managementButtonsVisible}
		<li>
			<button onclick={() => serverConsole.loadTopmostButtons()}>Configure Button links</button>
		</li>
		<li>
			<button onclick={() => serverConsole.loadTopmostButtons()}>
				Temporarily enable defaults
			</button>
		</li>

		<li><button onclick={() => serverConsole.loadTopmostButtons()}>Restore defaults</button></li>
	{/if}
</ul>
